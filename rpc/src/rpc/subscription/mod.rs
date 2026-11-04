use super::Route;
use crate::Codec;

mod client;
mod server;

pub use self::{client::*, server::*};

/// rpc where one request opens a stream of typed events
///
/// event frames are length-delimited and the stream ends cleanly at eof
/// any partial trailing frame is reported as truncation on the client side
pub trait Subscription: Route {
    /// typed input that starts the subscription
    type Request: Codec;
    /// typed event yielded by the responder
    type Event: Codec;
}
