//! application messages carried over QuantumLink RPC
//!
//! feature modules define requests, responses, events, and payloads for one area
//! such as onboarding, keys, firmware, or system operations
//! these types sit above the protocol streams and RPC machinery
//!
//! payloads use `ql-codec`. append fields with safe defaults and preserve wire and route ids.
//! use `Unknown` only where callers can safely ignore unfamiliar variants.

#[macro_use]
mod macros;

pub mod app_store;
pub mod backup;
pub mod bitcoin;
pub mod codec;
pub mod debug;
pub mod firmware;
pub mod fx;
pub mod key;
pub mod onboarding;
pub mod scv;
pub mod system;
pub mod time;

pub use ql_codec::Error;

pub use self::{
    app_store::*, backup::*, bitcoin::*, codec::Empty, debug::*, firmware::*, fx::*, key::*,
    onboarding::*, scv::*, system::*, time::*,
};

// naming scheme
// Request:
// - Request{Thing}
// - {Thing}Params
// - {Thing}Response
// Subscription:
// - Subscribe{Thing}
// - {Thing}Params
// - {Thing}Event
// Download:
// - Download{Thing}
// - Download{Thing}Params
// - Download{Thing}Header
// - Download{Thing}PartHeader
// Upload:
// - Upload{Thing}
// - Upload{Thing}Params
// - Upload{Thing}PartHeader
// - Upload{Thing}Response
// Duplex:
// - {Action}{Thing}
// - {Action}{Thing}InitiatorEvent
// - {Action}{Thing}ResponderEvent
