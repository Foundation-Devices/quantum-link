//! RPC patterns built on QuantumLink streams
//!
//! each call uses one stream with a route key and typed payloads
//! the crate provides request, notification, subscription, upload, download,
//! progress, and duplex flows with client and server helpers

#![allow(clippy::type_complexity)]

mod chunk_queue;
mod codec;
mod error;
mod router;
mod rpc;
mod stream;

pub use chunk_queue::ChunkQueue;
pub use codec::RpcCodec;
pub use error::*;
pub use router::*;
pub use rpc::*;
pub use stream::*;
