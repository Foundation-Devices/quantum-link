use ql_codec::{BufMut, BufView, Decode, Encode, Reader};
use ql_common::QID;

use crate::{
    encrypted_message::EncryptedMessage,
    handshake::{Ik1, Ik2, Xx1, Xx2, Xx3, Xx4},
    Error, RouteHeader, SessionHeader, QL_WIRE_VERSION,
};

pub fn encode_record<W, T>(out: &mut W, header: RecordHeader, body: &T)
where
    W: bytes::BufMut + ?Sized,
    T: Encode + ?Sized,
{
    header.encode(out);
    body.encode(out);
}

pub fn encode_record_vec<T: Encode + ?Sized>(header: RecordHeader, body: &T) -> Vec<u8> {
    let mut out = Vec::with_capacity(header.encoded_len() + body.encoded_len());
    encode_record(&mut out, header, body);
    out
}

pub fn decode_record<T: Decode>(bytes: &[u8]) -> Result<(RecordHeader, T::Ref<'_>), Error> {
    let mut reader = Reader::new(bytes);
    Ok((reader.decode()?, reader.decode_ref::<T>()?))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ql_codec::Codec)]
#[codec(frozen)]
pub struct RecordHeader {
    pub version: u8,
    pub route: RouteHeader,
    pub record_type: RecordType,
}

impl RecordHeader {
    pub const WIRE_SIZE: usize = size_of::<u8>() + QID::SIZE * 2 + size_of::<u8>();

    pub fn new(route: RouteHeader, record_type: RecordType) -> Self {
        Self {
            version: QL_WIRE_VERSION,
            route,
            record_type,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ql_codec::Codec)]
#[codec(frozen)]
pub enum RecordType {
    Handshake = 1,
    Session = 2,
}

#[derive(Debug, Clone, PartialEq, Eq, ql_codec::Codec)]
#[codec(frozen, discriminants = HandshakeKind)]
#[repr(u8)]
pub enum QlHandshakeRecord {
    Ik1(Ik1) = 1,
    Ik2(Ik2) = 2,
    Kk1(Ik1) = 3,
    Kk2(Ik2) = 4,
    Xx1(Xx1) = 5,
    Xx2(Xx2) = 6,
    Xx3(Xx3) = 7,
    Xx4(Xx4) = 8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QlSessionRecord<B> {
    pub header: SessionHeader,
    pub payload: EncryptedMessage<B>,
}

impl<B: AsRef<[u8]>> QlSessionRecord<B> {
    pub fn into_owned(self) -> QlSessionRecord<Vec<u8>> {
        QlSessionRecord {
            header: self.header,
            payload: self.payload.into_owned(),
        }
    }
}

impl<B: BufView> Encode for QlSessionRecord<B> {
    fn encoded_len(&self) -> usize {
        self.header.encoded_len() + self.payload.encoded_len()
    }

    fn encode<W: BufMut + ?Sized>(&self, out: &mut W) {
        self.header.encode(out);
        self.payload.encode(out);
    }
}

impl Decode for QlSessionRecord<Vec<u8>> {
    type Ref<'a> = QlSessionRecord<&'a [u8]>;

    fn decode_ref<'a>(reader: &mut Reader<'a>) -> Result<Self::Ref<'a>, ql_codec::Error> {
        Ok(QlSessionRecord {
            header: reader.decode()?,
            payload: reader.decode_ref::<EncryptedMessage<Vec<u8>>>()?,
        })
    }

    fn from_ref(value: Self::Ref<'_>) -> Self {
        value.into_owned()
    }
}
