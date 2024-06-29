use std::{collections::HashMap, future::Future, sync::{atomic::{AtomicU8, AtomicUsize, Ordering}, Arc, Mutex}, task::{Poll, Waker}};

const STATE_WAIT: u8 = 0;
const STATE_AWAKE: u8 = 1;

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
pub struct AwakeToken {
    id_gen: IdGenerator,
    state: AtomicU8,
    wakers: Mutex<HashMap<usize, Waker>>,
}

impl AwakeToken {
    #[inline]
    pub fn new() -> Self {
        Self {
            state: AtomicU8::new(STATE_WAIT),
            id_gen: IdGenerator::new(),
            wakers: Mutex::new(HashMap::new()),
        }
    }

    #[inline]
    pub fn awake(&self) {
        match self.state.swap(STATE_AWAKE, Ordering::SeqCst) {
            STATE_WAIT => {
                // Note that we don't need to worry about any future tasks awaiting `AwokenToken`
                // fighting for the `wakers` lock because the state was set to AWAKE during the
                // atomic swap.
                let mut l_wakers = self.wakers.lock().unwrap();
                for (_waker_id, waker) in l_wakers.drain() {
                    waker.wake();
                }
                // This map is never going to be used again. The heap allocated memory can be freed.
                // However, need to wait until all the references to this `AwakeToken` have been
                // dropped before dropping the rest of the object.
                l_wakers.shrink_to(0);
                drop(l_wakers);
            },
            STATE_AWAKE => (),   // Already awake, cannot be awoken twice.
            err_state => panic!("The awake token was in a state of neither being WAIT ({STATE_WAIT}) nor AWAKE ({STATE_AWAKE}). State was {err_state}"),
        };
    }

    #[inline]
    pub fn awoken(self: Arc<Self>) -> AwokenToken {
        match self.state.load(Ordering::SeqCst) {
            STATE_WAIT => AwokenToken { state: AwokenState::Fresh { awake_token: self } },
            STATE_AWAKE => AwokenToken { state: AwokenState::Awoken },
            err_state => panic!("The awake token was in a state of neither being WAIT ({STATE_WAIT}) nor AWAKE ({STATE_AWAKE}). State was {err_state}"),
        }
    }
}

#[derive(Debug)]
enum AwokenState {
    Fresh { awake_token: Arc<AwakeToken> },
    Registered { awake_token: Arc<AwakeToken>, waker_id: usize },
    Awoken,
}

#[derive(Debug)]
pub struct AwokenToken {
    state: AwokenState
}

impl<'a> Future for AwokenToken {
    type Output = ();

    #[inline]
    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        match &self.state {
            AwokenState::Fresh { awake_token } => match awake_token.state.load(Ordering::SeqCst) {
                STATE_WAIT => {
                    // FIXME: this could cause problems if all possible IDs already exist. It will
                    //        get stuck in an infinite loop.
                    let mut id = awake_token.id_gen.next();
                    let mut l_waiters = awake_token.wakers.lock().unwrap();
                    while l_waiters.contains_key(&id) {
                        id = awake_token.id_gen.next();
                    }
                    l_waiters.insert(id, cx.waker().clone());
                    drop(l_waiters);

                    // State Change: Fresh --> Registered
                    self.state = AwokenState::Registered { awake_token: awake_token.clone(), waker_id: id };
                    Poll::Pending
                },
                STATE_AWAKE => {
                    // State Change: Registered --> Awoken
                    self.state = AwokenState::Awoken;
                    Poll::Ready(())
                },
                err_state => panic!("The awake token was in a state of neither being WAIT ({STATE_WAIT}) nor AWAKE ({STATE_AWAKE}). State was {err_state}"),
            },
            AwokenState::Registered { awake_token, waker_id } => match awake_token.state.load(Ordering::SeqCst) {
                // There is a weird case where the future gets re-polled, but is still alive.
                // This probably should not happen since the waker should only get woken if the
                // state was set to `STATE_AWAKE`. However, if it does happen for some reason, we
                // want to handle it gracefully by re-registering.
                // It can re-use the same id it was given before too, since it will be overwriting
                // the previous entry.
                STATE_WAIT => {
                    let mut l_waiters = awake_token.wakers.lock().unwrap();
                    match l_waiters.get_mut(waker_id) {
                        Some(waker) => waker.clone_from(cx.waker()),
                        None => { l_waiters.insert(*waker_id, cx.waker().clone()); },
                    }
                    drop(l_waiters);

                    // State Unchanged: Registered --> Registered
                    Poll::Pending
                },
                STATE_AWAKE => {
                    // State Change: Registered --> Awoken
                    self.state = AwokenState::Awoken;
                    Poll::Ready(())
                },
                err_state => panic!("The awake token was in a state of neither being WAIT ({STATE_WAIT}) nor AWAKE ({STATE_AWAKE}). State was {err_state}"),
            },
            AwokenState::Awoken => Poll::Ready(()),
        }
    }
}

impl Drop for AwokenToken {
    #[inline]
    fn drop(&mut self) {
        match &self.state {
            AwokenState::Fresh { awake_token: _ } => (),
            AwokenState::Registered { awake_token, waker_id } => {
                let mut l_waiters = awake_token.wakers.lock().unwrap();
                l_waiters.remove(waker_id);
                drop(l_waiters);
            },
            AwokenState::Awoken => (),
        };
    }
}
