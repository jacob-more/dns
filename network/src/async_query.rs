use std::fmt::Display;

use async_lib::once_watch;
use dns_lib::query::message::Message;
use futures::future::BoxFuture;
use pin_project::pin_project;

use crate::errors;


#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum QueryOpt {
    UdpTcp,
    Tcp,
    Quic,
    Tls,
    QuicTls,
    Https,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub(crate) enum QSendType {
    Initial,
    Retransmit,
}

impl Display for QSendType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Initial => write!(f, "Initial"),
            Self::Retransmit => write!(f, "Retransmit"),
        }
    }
}

#[pin_project(project = QInitQueryProj)]
pub(crate) enum QInitQuery {
    Fresh,
    WriteActiveQuery,
    Following(#[pin] once_watch::Receiver<Result<Message, errors::QueryError>>),
    Complete,
}

impl QInitQuery {
    #[inline]
    pub fn set_write_active_query(mut self: std::pin::Pin<&mut Self>) {
        self.set(QInitQuery::WriteActiveQuery);
    }

    #[inline]
    pub fn set_following(mut self: std::pin::Pin<&mut Self>, receiver: once_watch::Receiver<Result<Message, errors::QueryError>>) {
        self.set(QInitQuery::Following(receiver));
    }

    #[inline]
    pub fn set_complete(mut self: std::pin::Pin<&mut Self>) {
        self.set(QInitQuery::Complete);
    }
}

#[pin_project(project = QSendProj)]
pub(crate) enum QSend<'t, MetaT, ErrorT>
where
    MetaT: Copy
{
    Fresh(MetaT),
    SendQuery(MetaT, BoxFuture<'t, Result<(), ErrorT>>),
    Complete(MetaT),
}

impl<'t, MetaT, ErrorT> QSend<'t, MetaT, ErrorT>
where
    MetaT: Copy
{
    #[inline]
    pub fn meta(&self) -> MetaT {
        match self {
            Self::Fresh(meta) => *meta,
            Self::SendQuery(meta, _) => *meta,
            Self::Complete(meta) => *meta,
        }
    }

    #[inline]
    pub fn set_fresh(mut self: std::pin::Pin<&mut Self>, meta: MetaT) {
        self.set(Self::Fresh(meta));
    }

    #[inline]
    pub fn set_send_query(mut self: std::pin::Pin<&mut Self>, send: BoxFuture<'t, Result<(), ErrorT>>) {
        let meta = self.meta();

        self.set(Self::SendQuery(meta, send));
    }

    #[inline]
    pub fn set_complete(mut self: std::pin::Pin<&mut Self>) {
        let meta = self.meta();

        self.set(Self::Complete(meta));
    }
}
