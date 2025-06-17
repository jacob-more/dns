use std::{
    error::Error,
    fmt::Display,
    future::Future,
    hash::Hash,
    sync::{
        Arc, RwLock,
        atomic::{AtomicUsize, Ordering},
    },
    task::Poll,
};

use pin_project::{pin_project, pinned_drop};

use crate::shared_awake_token::{SharedAwakeToken, SharedAwokenToken};

pub fn channel<T: Clone>() -> (Sender<T>, Receiver<T>) {
    let awake_token = Arc::new(SharedAwakeToken::new(SendCell::new_fresh(1, 1)));
    let sender = Sender {
        awake_token: awake_token.clone(),
    };
    let receiver = Receiver {
        awoken_token: awake_token.awoken(),
    };

    (sender, receiver)
}

pub trait SameChannel<T> {
    fn same_channel(&self, other: T) -> bool;
}

pub trait OnceWatchSubscribe<T> {
    fn subscribe(self) -> Receiver<T>;
}

pub trait OnceWatchSend<T> {
    fn send(&self, value: T) -> Result<(), SendError>;
}

#[derive(Debug)]
struct SendCell<T> {
    senders: AtomicUsize,
    receivers: AtomicUsize,
    data: RwLock<SendCellData<T>>,
}

impl<T> SendCell<T> {
    pub fn new_fresh(senders: usize, receivers: usize) -> Self {
        Self {
            senders: AtomicUsize::new(senders),
            receivers: AtomicUsize::new(receivers),
            data: RwLock::new(SendCellData::Fresh),
        }
    }
}

#[derive(Debug)]
enum SendCellData<T> {
    Fresh,
    EmptyClosed,
    Closed(T),
}

impl<T> SendCellData<T> {
    pub fn is_fresh(&self) -> bool {
        match self {
            Self::Fresh => true,
            _ => false,
        }
    }

    pub fn try_close(&mut self) -> bool {
        if self.is_fresh() {
            *self = Self::EmptyClosed;
            return true;
        } else {
            return false;
        }
    }
}

#[derive(Debug)]
pub enum RecvError {
    Closed,
}

impl Display for RecvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::Closed => write!(f, "channel closed"),
        }
    }
}

impl Error for RecvError {}

#[derive(Debug)]
pub enum TryRecvError {
    Empty,
    Closed,
}

impl Display for TryRecvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::Empty => write!(f, "channel empty"),
            Self::Closed => write!(f, "channel closed"),
        }
    }
}

impl Error for TryRecvError {}

#[derive(Debug)]
pub enum SendError {
    Closed,
}

impl Display for SendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self {
            Self::Closed => write!(f, "channel closed"),
        }
    }
}

impl Error for SendError {}

#[derive(Debug, Clone)]
pub struct Sender<T> {
    awake_token: Arc<SharedAwakeToken<SendCell<T>>>,
}

impl<T> Sender<T> {
    fn shared(&self) -> &SendCell<T> {
        &self.awake_token.shared
    }

    pub fn new() -> Self {
        Self {
            awake_token: Arc::new(SharedAwakeToken::new(SendCell::new_fresh(1, 0))),
        }
    }

    pub fn close(&self) -> bool {
        let mut w_data = self.shared().data.write().unwrap();
        if w_data.try_close() {
            drop(w_data);
            self.awake_token.awake();
            return true;
        } else {
            drop(w_data);
            return false;
        }
    }

    pub fn sender_count(&self) -> usize {
        self.shared().senders.load(Ordering::Relaxed)
    }

    pub fn receiver_count(&self) -> usize {
        self.shared().receivers.load(Ordering::Relaxed)
    }
}

impl<T> OnceWatchSubscribe<T> for &Sender<T> {
    fn subscribe(self) -> Receiver<T> {
        self.shared().receivers.fetch_add(1, Ordering::Relaxed);
        Receiver {
            awoken_token: self.awake_token.clone().awoken(),
        }
    }
}

impl<T> OnceWatchSend<T> for Sender<T> {
    fn send(&self, value: T) -> Result<(), SendError> {
        let mut w_shared = self.shared().data.write().unwrap();
        if w_shared.is_fresh() {
            *w_shared = SendCellData::Closed(value);
            drop(w_shared);
            self.awake_token.awake();
            return Ok(());
        } else {
            drop(w_shared);
            return Err(SendError::Closed);
        }
    }
}

impl<T: Clone> OnceWatchSend<&T> for Sender<T> {
    fn send(&self, value: &T) -> Result<(), SendError> {
        let mut w_shared = self.shared().data.write().unwrap();
        if w_shared.is_fresh() {
            *w_shared = SendCellData::Closed(value.clone());
            drop(w_shared);
            self.awake_token.awake();
            return Ok(());
        } else {
            drop(w_shared);
            return Err(SendError::Closed);
        }
    }
}

impl<T> PartialEq for Sender<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.awake_token, &other.awake_token)
    }
}

impl<T> Eq for Sender<T> {}

impl<T> Hash for Sender<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.awake_token).hash(state);
    }
}

impl<T> SameChannel<&Sender<T>> for Sender<T> {
    fn same_channel(&self, other: &Sender<T>) -> bool {
        self == other
    }
}

impl<T> SameChannel<&Receiver<T>> for Sender<T> {
    fn same_channel(&self, other: &Receiver<T>) -> bool {
        Arc::ptr_eq(
            &self.awake_token,
            other.awoken_token.get_shared_awake_token(),
        )
    }
}

impl<T> Drop for Sender<T> {
    fn drop(&mut self) {
        self.shared().senders.fetch_sub(1, Ordering::Relaxed);
    }
}

#[pin_project(PinnedDrop)]
#[derive(Debug)]
pub struct Receiver<T> {
    #[pin]
    awoken_token: SharedAwokenToken<SendCell<T>>,
}

impl<T> Receiver<T> {
    fn shared(&self) -> &SendCell<T> {
        &self.awoken_token.get_shared_awake_token().shared
    }

    pub fn new() -> Self {
        Self {
            awoken_token: Arc::new(SharedAwakeToken::new(SendCell::new_fresh(0, 1))).awoken(),
        }
    }

    pub fn close(&self) -> bool {
        let mut w_data = self.shared().data.write().unwrap();
        if w_data.try_close() {
            drop(w_data);
            self.awoken_token.get_shared_awake_token().awake();
            return true;
        } else {
            drop(w_data);
            return false;
        }
    }

    pub fn get_sender(&self) -> Sender<T> {
        self.shared().senders.fetch_add(1, Ordering::Relaxed);
        Sender {
            awake_token: self.awoken_token.get_shared_awake_token().clone(),
        }
    }

    pub fn sender_count(&self) -> usize {
        self.shared().senders.load(Ordering::Relaxed)
    }

    pub fn receiver_count(&self) -> usize {
        self.shared().receivers.load(Ordering::Relaxed)
    }
}

impl<T: Clone> Receiver<T> {
    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        if self.awoken_token.get_shared_awake_token().try_awoken() {
            let r_send_cell = self.shared().data.read().unwrap();

            match &*r_send_cell {
                SendCellData::Fresh => Err(TryRecvError::Empty),
                SendCellData::EmptyClosed => Err(TryRecvError::Closed),
                SendCellData::Closed(value) => Ok(value.clone()),
            }
        } else {
            Err(TryRecvError::Empty)
        }
    }
}

impl<T> OnceWatchSubscribe<T> for &Receiver<T> {
    fn subscribe(self) -> Receiver<T> {
        self.shared().receivers.fetch_add(1, Ordering::Relaxed);
        self.clone()
    }
}

impl<T> Clone for Receiver<T> {
    fn clone(&self) -> Self {
        self.shared().receivers.fetch_add(1, Ordering::Relaxed);
        Self {
            awoken_token: self.awoken_token.get_shared_awake_token().clone().awoken(),
        }
    }
}

impl<T> PartialEq for Receiver<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(
            &self.awoken_token.get_shared_awake_token(),
            &other.awoken_token.get_shared_awake_token(),
        )
    }
}

impl<T> Eq for Receiver<T> {}

impl<T> Hash for Receiver<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.awoken_token.get_shared_awake_token()).hash(state);
    }
}

impl<T> SameChannel<&Sender<T>> for Receiver<T> {
    fn same_channel(&self, other: &Sender<T>) -> bool {
        Arc::ptr_eq(
            self.awoken_token.get_shared_awake_token(),
            &other.awake_token,
        )
    }
}

impl<T> SameChannel<&Receiver<T>> for Receiver<T> {
    fn same_channel(&self, other: &Receiver<T>) -> bool {
        self == other
    }
}

impl<T: Clone> Future for Receiver<T> {
    type Output = Result<T, RecvError>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        match self.as_mut().project().awoken_token.poll(cx) {
            Poll::Ready(()) => {
                let r_send_cell = self.shared().data.read().unwrap();

                match &*r_send_cell {
                    SendCellData::Fresh => panic!("No assignment has been made to the send cell"),
                    SendCellData::EmptyClosed => Poll::Ready(Err(RecvError::Closed)),
                    SendCellData::Closed(value) => Poll::Ready(Ok(value.clone())),
                }
            }
            Poll::Pending => Poll::Pending,
        }
    }
}

#[pinned_drop]
impl<T> PinnedDrop for Receiver<T> {
    fn drop(self: std::pin::Pin<&mut Self>) {
        self.shared().receivers.fetch_sub(1, Ordering::Relaxed);
    }
}
