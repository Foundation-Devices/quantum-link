use ql_common::ResetCode;
pub use ql_fsm::ResetOrigin;
use ql_fsm::{NoSessionError, OpenStreamError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QlStreamError {
    StreamReset {
        code: ResetCode,
        origin: ResetOrigin,
    },
    HeaderTooLarge,
    NoSession,
}

impl std::fmt::Display for QlStreamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StreamReset { code, origin } => write!(f, "stream reset {code:?} ({origin:?})"),
            Self::HeaderTooLarge => f.write_str("stream header exceeds record budget"),
            Self::NoSession => f.write_str("no session"),
        }
    }
}

impl std::error::Error for QlStreamError {}

impl From<NoSessionError> for QlStreamError {
    fn from(_: NoSessionError) -> Self {
        Self::NoSession
    }
}

impl From<OpenStreamError> for QlStreamError {
    fn from(error: OpenStreamError) -> Self {
        match error {
            OpenStreamError::HeaderTooLarge => Self::HeaderTooLarge,
            OpenStreamError::NoSession => Self::NoSession,
        }
    }
}
