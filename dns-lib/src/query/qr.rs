use std::fmt::Display;

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum QR {
    Query,
    Response,
}

impl Display for QR {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QR::Query => write!(f, "Query"),
            QR::Response => write!(f, "Response"),
        }
    }
}

impl QR {
    #[inline]
    pub const fn is_query(&self) -> bool {
        match self {
            QR::Query => true,
            QR::Response => false,
        }
    }

    #[inline]
    pub const fn is_response(&self) -> bool {
        match self {
            QR::Query => false,
            QR::Response => true,
        }
    }
}
