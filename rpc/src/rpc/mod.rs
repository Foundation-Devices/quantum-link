//! rpc protocol families built on top of one stream per call
//!
//! each trait in this module names one rpc shape and the typed values that
//! travel on that stream
//! route dispatch uses caller-defined route keys and the submodules provide the
//! matching client and server helpers for encoding, decoding, and handler glue

pub mod download;
pub mod duplex;
pub mod notification;
pub(crate) mod parts;
pub mod progress;
pub mod request;
pub mod subscription;
#[cfg(test)]
mod tests;
pub mod upload;
mod utils;

/// configures an rpc call
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RpcOptions {
    /// maximum encoded payload size excluding the rpc frame prefix
    ///
    /// applies separately to replies, events, progress values and multipart headers
    /// raw multipart body chunks are not limited
    /// defaults to 1 MiB
    pub max_value_bytes: usize,
}

impl Default for RpcOptions {
    fn default() -> Self {
        Self {
            max_value_bytes: 1024 * 1024,
        }
    }
}

pub trait RpcRouteKey: Sized + std::fmt::Debug + Ord + Clone + 'static {
    fn encoded_len(&self) -> usize;

    fn encode(&self, out: &mut Vec<u8>);

    fn decode(bytes: &[u8]) -> Option<Self>;
}

pub trait Route {
    type Key: RpcRouteKey;

    fn key() -> Self::Key;
}

use utils::*;

pub use self::{
    download::Download,
    duplex::Duplex,
    notification::Notification,
    parts::{MultipartPart, MultipartReader},
    progress::Progress,
    request::Request,
    subscription::Subscription,
    upload::Upload,
};
