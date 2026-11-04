use super::Route;
use crate::Codec;

mod client;
mod server;

pub use self::{client::*, server::*};

/// rpc where both sides exchange typed events on the same stream
///
/// The initiator opens the routed stream. After that, either side may send any
/// number of events of its directional event type until it finishes or closes
/// its write side.
pub trait Duplex: Route {
    /// typed event sent by the side that opened the stream
    type InitiatorEvent: Codec;
    /// typed event sent by the side handling the route
    type ResponderEvent: Codec;
}
