#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, ql_codec::Codec)]
#[codec(frozen)]
#[repr(transparent)]
pub struct HandshakeId(pub u32);
