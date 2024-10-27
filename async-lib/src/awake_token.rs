use std::{future::Future, hash::Hash, sync::Arc, task::Poll};

use pin_project::pin_project;

use crate::shared_awake_token::{SharedAwakeToken, SharedAwokenToken};

pub trait SameAwakeToken<T> {
    fn same_awake_token(&self, other: T) -> bool;
}

#[derive(Debug)]
pub struct AwakeToken { shared: Arc<SharedAwakeToken<()>> }

impl AwakeToken {
    #[inline]
    pub fn new() -> Self {
        Self {
            shared: Arc::new(SharedAwakeToken::new(()))
        }
    }

    #[inline]
    pub fn awake(&self) {
        self.shared.awake()
    }

    #[inline]
    pub fn awoken(&self) -> AwokenToken {
        AwokenToken { shared: self.shared.clone().awoken() }
    }

    #[inline]
    pub fn try_awoken(&self) -> bool {
        self.shared.try_awoken()
    }
}

impl Clone for AwakeToken {
    fn clone(&self) -> Self {
        Self {
            shared: self.shared.clone()
        }
    }
}

impl PartialEq for AwakeToken {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.shared, &other.shared)
    }
}

impl Eq for AwakeToken {}

impl Hash for AwakeToken {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        Arc::as_ptr(&self.shared).hash(state);
    }
}

impl SameAwakeToken<&AwakeToken> for AwakeToken {
    fn same_awake_token(&self, other: &AwakeToken) -> bool {
        self == other
    }
}

impl SameAwakeToken<&AwokenToken> for AwakeToken {
    fn same_awake_token(&self, other: &AwokenToken) -> bool {
        Arc::ptr_eq(
            &self.shared,
            &other.shared.get_shared_awake_token()
        )
    }
}

#[pin_project]
#[derive(Debug)]
pub struct AwokenToken {
    #[pin]
    shared: SharedAwokenToken<()>
}

impl AwokenToken {
    #[inline]
    pub fn awoken(&self) -> AwokenToken {
        AwokenToken {
            shared: self.shared.get_shared_awake_token()
                .clone()
                .awoken()
        }
    }

    #[inline]
    pub fn try_awoken(&self) -> bool {
        self.shared.get_shared_awake_token().try_awoken()
    }

    #[inline]
    pub fn get_awake_token(&self) -> AwakeToken {
        AwakeToken { shared: self.shared.get_shared_awake_token().clone() }
    }

    #[inline]
    pub fn awake(&self) {
        self.shared.get_shared_awake_token().awake()
    }
}

impl<'a> Future for AwokenToken {
    type Output = ();

    #[inline]
    fn poll(mut self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> Poll<Self::Output> {
        self.as_mut().project().shared.poll(cx)
    }
}

impl SameAwakeToken<&AwakeToken> for AwokenToken {
    fn same_awake_token(&self, other: &AwakeToken) -> bool {
        Arc::ptr_eq(
            &self.shared.get_shared_awake_token(),
            &other.shared
        )
    }
}

impl SameAwakeToken<&AwokenToken> for AwokenToken {
    fn same_awake_token(&self, other: &AwokenToken) -> bool {
        Arc::ptr_eq(
            &self.shared.get_shared_awake_token(),
            &other.shared.get_shared_awake_token()
        )
    }
}
