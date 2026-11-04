use super::Route;
use crate::Codec;

mod client;
pub(crate) mod codec;
mod server;

pub use self::{client::*, server::*};

/// rpc where the responder streams progress values before a final response
///
/// the request is eof-delimited
/// response frames are tagged so the client can distinguish
/// [`Self::Progress`] items from the final [`Self::Response`]
/// reaching eof before the final response is an error
pub trait Progress: Route {
    /// typed input sent by the caller
    type Request: Codec;
    /// typed progress item emitted before completion
    type Progress: Codec;
    /// typed terminal response that completes the call
    type Response: Codec;
}
