use std::{collections::HashMap, future::Future, sync::{atomic::{AtomicU8, AtomicUsize, Ordering}, Arc, Mutex}, task::{Poll, Waker}};

const STATE_ALIVE: u8 = 0;
const STATE_CANCELED: u8 = 1;

#[derive(Debug)]
struct IdGenerator {
    next_id: AtomicUsize,
}

impl IdGenerator {
    #[inline]
    fn new() -> Self {
        Self { next_id: AtomicUsize::new(0) }
    }

    #[inline]
    fn next(&self) -> usize {
        self.next_id.fetch_add(1, Ordering::SeqCst)
    }
}

#[derive(Debug)]
pub struct Cancel {
    id_gen: IdGenerator,
    state: AtomicU8,
    wakers: Mutex<HashMap<usize, Waker>>,
}

impl Cancel {
    #[inline]
    pub fn new() -> Self {
        Self {
            state: AtomicU8::new(STATE_ALIVE),
            id_gen: IdGenerator::new(),
            wakers: Mutex::new(HashMap::new()),
        }
    }

    #[inline]
    pub fn cancel(&self) {
        match self.state.swap(STATE_CANCELED, Ordering::SeqCst) {
            STATE_ALIVE => {
                // Note that we don't need to worry about any future tasks awaiting `Cancelled`
                // fighting for the `wakers` lock because the state was set to CANCELLED during the
                // atomic swap.
                let mut l_wakers = self.wakers.lock().unwrap();
                for (_waker_id, waker) in l_wakers.drain() {
                    waker.wake();
                }
                // This map is never going to be used again. The heap allocated memory can be freed.
                // However, need to wait until all the references to this `Cancel` have been dropped
                // before dropping the rest of the object.
                l_wakers.shrink_to(0);
                drop(l_wakers);
            },
            STATE_CANCELED => (),   // Already cancelled, cannot be cancelled twice.
            err_state => panic!("The cancel token was in a state of neither being alive nor canceled. State was {err_state}"),
        };
    }

    #[inline]
    pub fn canceled(self: Arc<Self>) -> Canceled {
        match self.state.load(Ordering::SeqCst) {
            STATE_ALIVE => Canceled { state: CanceledState::Fresh { cancel: self } },
            STATE_CANCELED => Canceled { state: CanceledState::Canceled },
            err_state => panic!("The cancel token was in a state of neither being alive nor canceled. State was {err_state}"),
        }
    }
}

#[derive(Debug)]
enum CanceledState {
    Fresh { cancel: Arc<Cancel> },
    Registered { cancel: Arc<Cancel>, waker_id: usize },
    Canceled,
}

#[derive(Debug)]
pub struct Canceled {
    state: CanceledState
}

impl<'a> Future for Canceled {
    type Output = ();

    #[inline]
    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        match &self.state {
            CanceledState::Fresh { cancel } => match cancel.state.load(Ordering::SeqCst) {
                STATE_ALIVE => {
                    // FIXME: this could cause problems if all possible IDs already exist. It will
                    //        get stuck in an infinite loop.
                    let mut id = cancel.id_gen.next();
                    let mut l_waiters = cancel.wakers.lock().unwrap();
                    while l_waiters.contains_key(&id) {
                        id = cancel.id_gen.next();
                    }
                    l_waiters.insert(id, cx.waker().clone());
                    drop(l_waiters);

                    // State Change: Fresh --> Registered
                    self.state = CanceledState::Registered { cancel: cancel.clone(), waker_id: id };
                    Poll::Pending
                },
                STATE_CANCELED => {
                    // State Change: Registered --> Canceled
                    self.state = CanceledState::Canceled;
                    Poll::Ready(())
                },
                err_state => panic!("The cancel token was in a state of neither being alive nor canceled. State was {err_state}"),
            },
            CanceledState::Registered { cancel, waker_id } => match cancel.state.load(Ordering::SeqCst) {
                // There is a weird case where the future gets re-polled, but is still alive.
                // This probably should not happen since the waker should only get woken if the
                // state was set to `STATE_CANCELED`. However, if it does happen for some reason, we
                // want to handle it gracefully by re-registering.
                // It can re-use the same id it was given before too, since it will be overwriting
                // the previous entry.
                STATE_ALIVE => {
                    let mut l_waiters = cancel.wakers.lock().unwrap();
                    l_waiters.insert(*waker_id, cx.waker().clone());
                    drop(l_waiters);

                    // State Unchanged: Registered --> Registered
                    Poll::Pending
                },
                STATE_CANCELED => {
                    // State Change: Registered --> Canceled
                    self.state = CanceledState::Canceled;
                    Poll::Ready(())
                },
                err_state => panic!("The cancel token was in a state of neither being alive nor canceled. State was {err_state}"),
            },
            CanceledState::Canceled => Poll::Ready(()),
        }
    }
}

impl Drop for Canceled {
    #[inline]
    fn drop(&mut self) {
        match &self.state {
            CanceledState::Fresh { cancel: _ } => (),
            CanceledState::Registered { cancel, waker_id } => {
                let mut l_waiters = cancel.wakers.lock().unwrap();
                l_waiters.remove(&waker_id);
                drop(l_waiters);
            },
            CanceledState::Canceled => (),
        };
    }
}
