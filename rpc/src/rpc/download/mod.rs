use super::Route;
use crate::Codec;

mod client;
mod server;

pub use self::{client::*, server::*};

/// rpc where the responder returns metadata first and then zero or more byte parts
///
/// the typed portion of the response ends at [`Self::ResponseHeader`]
/// after the header is decoded, the rest of the stream is exposed as typed
/// part headers followed by raw byte chunks through [`crate::MultipartReader`]
pub trait Download: Route {
    /// typed input needed to start the download
    type Request: Codec;
    /// typed metadata available before parts arrive
    type ResponseHeader: Codec;
    /// typed metadata available before each byte part arrives
    type PartHeader: Codec;
}
