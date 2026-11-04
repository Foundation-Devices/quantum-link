use ql_common::ResetCode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Error {
    Truncated,
    LengthOverflow,
    UnexpectedFrameKind(u8),
    TrailingBytes,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Truncated => f.write_str("truncated rpc payload"),
            Self::LengthOverflow => f.write_str("rpc payload length overflow"),
            Self::UnexpectedFrameKind(kind) => write!(f, "unexpected rpc frame kind {kind}"),
            Self::TrailingBytes => f.write_str("trailing rpc bytes"),
        }
    }
}

impl std::error::Error for Error {}

impl Error {
    pub const fn reset_code(self) -> ResetCode {
        match self {
            Self::LengthOverflow => ResetCode::LIMIT,
            Self::Truncated | Self::UnexpectedFrameKind(_) | Self::TrailingBytes => {
                ResetCode::PROTOCOL
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpcError<T> {
    Protocol(Error),
    Codec(ql_codec::Error),
    Transport(T),
}

impl<T> RpcError<T> {
    pub const fn reset_code(&self) -> Option<ResetCode> {
        match self {
            Self::Protocol(error) => Some(error.reset_code()),
            Self::Codec(_) => Some(ResetCode::CODEC),
            Self::Transport(_) => None,
        }
    }
}

impl<T> std::fmt::Display for RpcError<T>
where
    T: std::fmt::Display,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Protocol(error) => write!(f, "{error}"),
            Self::Codec(error) => write!(f, "{error}"),
            Self::Transport(error) => write!(f, "{error}"),
        }
    }
}

impl<T> std::error::Error for RpcError<T>
where
    T: std::error::Error + 'static,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Protocol(error) => Some(error),
            Self::Codec(error) => Some(error),
            Self::Transport(error) => Some(error),
        }
    }
}

impl<T> From<Error> for RpcError<T> {
    fn from(error: Error) -> Self {
        Self::Protocol(error)
    }
}
