use std::{collections::HashMap, future::Future, sync::{atomic::{AtomicU8, AtomicUsize, Ordering}, Arc, Mutex}, task::{Poll, Waker}};

/// A state in which the token has not yet been awoken. In this state, wakers
/// can be added to the `waker` map and they will be awoken if the state
/// changes to `STATE_AWAKE`.
const STATE_WAIT: u8 = 0;
/// A state in which the token has been awoken. No additional wakers should be
/// added to `wakers`.
const STATE_AWAKE: u8 = 1;

#[repr(u8)]
enum State {
    /// Equivalent to `STATE_WAIT`
    Wait = STATE_WAIT,
    /// Equivalent to `STATE_AWAKE`
    Awake = STATE_AWAKE,
}

impl From<u8> for State {
    #[inline]
    fn from(value: u8) -> Self {
        match value {
            STATE_WAIT => State::Wait,
            STATE_AWAKE => State::Awake,
            err_state => panic!("The awake token was in a state of neither being WAIT ({STATE_WAIT}) nor AWAKE ({STATE_AWAKE}). State was {err_state}"),
        }
    }
}

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
        self.next_id.fetch_add(1, Ordering::AcqRel)
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
            state: AtomicU8::new(State::Wait as u8),
            id_gen: IdGenerator::new(),
            wakers: Mutex::new(HashMap::new()),
        }
    }

    #[inline]
    pub fn awake(&self) {
        match State::from(self.state.swap(State::Awake as u8, Ordering::AcqRel)) {
            State::Wait => {
                let mut l_wakers = self.wakers.lock().unwrap();
                for (_waker_id, waker) in l_wakers.drain() {
                    waker.wake();
                }
                drop(l_wakers);
            },
            State::Awake => (),   // Already awake, cannot be awoken twice.
        };
    }

    #[inline]
    pub fn awoken(self: Arc<Self>) -> AwokenToken {
        match State::from(self.state.load(Ordering::Acquire)) {
            State::Wait => AwokenToken { state: AwokenState::Fresh { awake_token: self } },
            State::Awake => AwokenToken { state: AwokenState::Awoken },
        }
    }

    #[inline]
    pub fn try_awoken(&self) -> bool {
        match State::from(self.state.load(Ordering::Acquire)) {
            State::Wait => false,
            State::Awake => true,
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
            AwokenState::Fresh { awake_token } => match State::from(awake_token.state.load(Ordering::Acquire)) {
                State::Wait => {
                    // FIXME: this could cause problems if all possible IDs already exist. It will
                    //        get stuck in an infinite loop.
                    let mut id = awake_token.id_gen.next();
                    let mut l_waiters = awake_token.wakers.lock().unwrap();
                    while l_waiters.contains_key(&id) {
                        id = awake_token.id_gen.next();
                    }
                    l_waiters.insert(id, cx.waker().clone());
                    drop(l_waiters);

                    // Need to double check that the state was not switched
                    // while we were waiting for the lock.
                    match State::from(awake_token.state.load(Ordering::Acquire)) {
                        State::Wait => {
                            // State Change: Fresh --> Registered
                            self.state = AwokenState::Registered { awake_token: awake_token.clone(), waker_id: id };
                            Poll::Pending
                        },
                        State::Awake => {
                            // State Change: Fresh --> Awoken
                            self.state = AwokenState::Awoken;
                            Poll::Ready(())
                        },
                    }
                },
                State::Awake => {
                    // State Change: Registered --> Awoken
                    self.state = AwokenState::Awoken;
                    Poll::Ready(())
                },
            },
            AwokenState::Registered { awake_token, waker_id } => match State::from(awake_token.state.load(Ordering::Acquire)) {
                // There is a case where the future gets re-polled, but is still alive.
                // It can re-use the same id it was given before, since it will be overwriting
                // the previous entry.
                State::Wait => {
                    let mut l_waiters = awake_token.wakers.lock().unwrap();
                    match l_waiters.get_mut(waker_id) {
                        Some(waker) => {
                            waker.clone_from(cx.waker());
                            drop(l_waiters);

                            // State Unchanged: Registered --> Registered
                            Poll::Pending
                        },
                        None => {
                            // This case occurs if the state changed to Awake
                            // while we were trying to acquire the lock.
                            drop(l_waiters);

                            // State Unchanged: Registered --> Ready
                            self.state = AwokenState::Awoken;
                            Poll::Ready(())
                        },
                    }
                },
                State::Awake => {
                    // State Change: Registered --> Awoken
                    self.state = AwokenState::Awoken;
                    Poll::Ready(())
                },
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
