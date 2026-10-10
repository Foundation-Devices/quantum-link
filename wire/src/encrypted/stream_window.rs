use ql_codec::Varint;

use super::StreamId;

/// advertises the highest byte offset the peer may send on a stream.
#[derive(Debug, Clone, PartialEq, Eq, ql_codec::Codec)]
#[codec(frozen)]
pub struct StreamWindow {
    pub stream_id: StreamId,
    pub maximum_offset: Varint<u64>,
}
