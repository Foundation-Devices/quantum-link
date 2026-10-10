//! chunking for QuantumLink records over BLE
//!
//! [`Chunker`] splits a record into fixed size packets with a sequence and index
//! [`Dechunker`] accepts packets out of order and yields the record once complete

pub use chunk::*;
pub use dechunk::*;

mod chunk;
mod dechunk;
#[cfg(test)]
mod tests;

pub const MAX_RECORD_SIZE: usize = 128 * 1024;
// ble firmware mtu with dle enabled
// https://github.com/Foundation-Devices/prime-ble-firmware/blob/543345473e874d7b45c47b386168ff24b9b7f060/consts/src/lib.rs#L10
pub const APP_MTU: usize = 244;
pub const HEADER_SIZE: usize = size_of::<u16>() * 2 + size_of::<u32>();
pub const CHUNK_DATA_SIZE: usize = APP_MTU - HEADER_SIZE;

// v1 required its final header byte to be zero, so this tags v2 without growing the header
const V2_TAG: u8 = 0xb2;
const V2_TAG_BITS: u32 = (V2_TAG as u32) << 24;
const RECORD_LEN_MASK: u32 = 0x00ff_ffff;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    V1,
    V2,
}

pub fn packet_version(data: &[u8]) -> Option<Version> {
    match *data.get(HEADER_SIZE - 1)? {
        0 => Some(Version::V1),
        V2_TAG => Some(Version::V2),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ql_codec::Codec)]
#[codec(frozen)]
struct Header {
    sequence: u16,
    index: u16,
    tagged_record_len: u32,
}
