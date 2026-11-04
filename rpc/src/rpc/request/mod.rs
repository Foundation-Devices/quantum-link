use super::Route;
use crate::Codec;

mod client;
mod server;

pub use self::{client::*, server::*};

/// request-response rpc with exactly one typed value in each direction
///
/// the request is read to eof on the server side, so callers must finish the
/// request stream after encoding [`Self::Request`]
/// the response is also read to eof and rejects trailing bytes after
/// [`Self::Response`]
pub trait Request: Route {
    /// typed input sent by the caller
    type Request: Codec;
    /// typed output returned by the responder
    type Response: Codec;
}
