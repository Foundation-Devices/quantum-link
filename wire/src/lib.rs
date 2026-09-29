//! QuantumLink wire records and authenticated handshakes
//!
//! defines peer identities, routing and session headers, encrypted frames,
//! and the [`IK`](IkPattern::Ik), [`KK`](IkPattern::Kk), and [`XX`](XxHandshake) handshakes
//! [`QlCrypto`] supplies the key establishment and encryption operations

#![allow(clippy::too_many_arguments)]

mod crypto;
mod encrypted;
mod encrypted_message;
mod error;
mod handshake;
mod header;
mod identity;
mod pq;
mod qid;
mod record;
#[cfg(any(feature = "software-crypto", test))]
mod testing;

pub use crypto::*;
pub use encrypted::*;
pub use encrypted_message::*;
pub use error::*;
pub use handshake::*;
pub use header::*;
pub use identity::*;
pub use pq::*;
pub use qid::*;
pub use record::*;
#[cfg(any(feature = "software-crypto", test))]
pub use testing::*;

pub const QL_WIRE_VERSION: u8 = 2;
pub const ENCRYPTED_MESSAGE_AUTH_SIZE: usize = 16;

#[cfg(test)]
mod tests;
