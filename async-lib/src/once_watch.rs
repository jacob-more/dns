use std::{error::Error, fmt::Display, future::Future, hash::Hash, sync::{Arc, RwLock}, task::Poll};

use pin_project::pin_project;

use crate::shared_awake_token::{SharedAwakeToken, SharedAwokenToken};

pub fn channel<T: Clone>() -> (Sender<T>, Receiver<T>) {
    let sender = Sender::new();
    let receiver = (&sender).subscribe();

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
enum SendCell<T> {
    Fresh,
    EmptyClosed,
    Closed(T),
}

impl<T> SendCell<T> {
    pub fn is_fresh(&self) -> bool {
        match self {
            SendCell::Fresh => true,
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
    Closed
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
    Closed
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
    awake_token: Arc<SharedAwakeToken<RwLock<SendCell<T>>>>,
}

impl<T> Sender<T> {
    fn shared(&self) -> &RwLock<SendCell<T>> {
        &self.awake_token.shared
    }

    pub fn new() -> Self {
        Self {
            awake_token: Arc::new(SharedAwakeToken::new(RwLock::new(SendCell::Fresh))),
        }
    }

    pub fn close(&self) -> bool {
        if self.shared().write().unwrap().try_close() {
            self.awake_token.awake();
            return true;
        } else {
            return false;
        }
    }
}

impl<T> OnceWatchSubscribe<T> for Sender<T> {
    fn subscribe(self) -> Receiver<T> {
        Receiver {
            awoken_token: self.awake_token.awoken()
        }
    }
}

impl<T> OnceWatchSubscribe<T> for &Sender<T> {
    fn subscribe(self) -> Receiver<T> {
        Receiver {
            awoken_token: self.awake_token.clone().awoken()
        }
    }
}

impl<T> OnceWatchSend<T> for Sender<T> {
    fn send(&self, value: T) -> Result<(), SendError> {
        let mut w_shared = self.shared().write().unwrap();
        if w_shared.is_fresh() {
            *w_shared = SendCell::Closed(value);
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
        let mut w_shared = self.shared().write().unwrap();
        if w_shared.is_fresh() {
            *w_shared = SendCell::Closed(value.clone());
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
            other.awoken_token.get_shared_awake_token()
        )
    }
}

#[pin_project]
#[derive(Debug)]
pub struct Receiver<T> {
    #[pin]
    awoken_token: SharedAwokenToken<RwLock<SendCell<T>>>,
}

impl<T> Receiver<T> {
    fn shared(&self) -> &RwLock<SendCell<T>> {
        &self.awoken_token.get_shared_awake_token().shared
    }

    pub fn new() -> Self {
        Self {
            awoken_token: Arc::new(SharedAwakeToken::new(RwLock::new(SendCell::Fresh))).awoken()
        }
    }

    pub fn close(&self) -> bool {
        if self.shared().write().unwrap().try_close() {
            self.awoken_token.get_shared_awake_token().awake();
            return true;
        } else {
            return false;
        }
    }

    pub fn get_sender(&self) -> Sender<T> {
        Sender {
            awake_token: self.awoken_token.get_shared_awake_token().clone()
        }
    }
}

impl<T: Clone> Receiver<T> {
    pub fn try_recv(&self) -> Result<T, TryRecvError> {
        if self.awoken_token.get_shared_awake_token().try_awoken() {
            let r_send_cell = self.shared()
                .read()
                .unwrap();

            match &*r_send_cell {
                SendCell::Fresh => Err(TryRecvError::Empty),
                SendCell::EmptyClosed => Err(TryRecvError::Closed),
                SendCell::Closed(value) => Ok(value.clone()),
            }
        } else {
            Err(TryRecvError::Empty)
        }
    }
}

impl<T> OnceWatchSubscribe<T> for &Receiver<T> {
    fn subscribe(self) -> Receiver<T> {
        self.clone()
    }
}

impl<T> Clone for Receiver<T> {
    fn clone(&self) -> Self {
        Self {
            awoken_token: self.awoken_token.get_shared_awake_token()
                .clone()
                .awoken()
        }
    }
}

impl<T> PartialEq for Receiver<T> {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(
            &self.awoken_token.get_shared_awake_token(),
            &other.awoken_token.get_shared_awake_token()
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
            &other.awake_token
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

    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Self::Output> {
        match self.as_mut().project().awoken_token.poll(cx) {
            Poll::Ready(()) => {
                let r_send_cell = self.shared()
                    .read()
                    .unwrap();

                match &*r_send_cell {
                    SendCell::Fresh => panic!("No assignment has been made to the send cell"),
                    SendCell::EmptyClosed => Poll::Ready(Err(RecvError::Closed)),
                    SendCell::Closed(value) => Poll::Ready(Ok(value.clone())),
                }
            },
            Poll::Pending => Poll::Pending,
        }
    }
}
