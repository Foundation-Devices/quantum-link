use bytes::{Buf, BufMut};
use ql_codec::{Decode, Encode, Reader, Writer};

use crate::{Error, RpcError};

/// encoding and decoding owned rpc payloads
pub trait Codec: Encode + DecodeOwned {}

impl<T: Encode + DecodeOwned> Codec for T {}

/// decoding an owned rpc payload from any shared byte slice
pub trait DecodeOwned: for<'a> Decode<&'a [u8]> {}

impl<T: for<'a> Decode<&'a [u8]>> DecodeOwned for T {}

pub fn encode_value_part<T: Encode>(value: &T, out: &mut Vec<u8>) {
    let len = u64::try_from(value.encoded_len()).expect("rpc payload exceeds u64 length framing");
    out.put_u64_le(len);
    value.encode(&mut Writer::new(out));
}

pub fn encode_tagged_value_part<T: Encode>(tag: u8, value: &T, out: &mut Vec<u8>) {
    out.put_u8(tag);
    encode_value_part(value, out);
}

pub fn decode_exact<T, E>(bytes: &mut impl Buf) -> Result<T, RpcError<E>>
where
    T: DecodeOwned,
{
    let len = bytes.remaining();
    let contiguous;
    let input = if bytes.chunk().len() == len {
        bytes.chunk()
    } else {
        contiguous = bytes.copy_to_bytes(len);
        contiguous.as_ref()
    };
    let mut reader = Reader::new(input);
    let value = T::decode(&mut reader).map_err(RpcError::Codec)?;
    if !reader.is_empty() {
        return Err(Error::TrailingBytes.into());
    }
    bytes.advance(bytes.remaining());
    Ok(value)
}

#[cfg(test)]
mod tests {
    use bytes::Buf;
    use ql_codec::{Codec, Encode};

    use super::{decode_exact, encode_tagged_value_part};
    use crate::{Error, RpcError};

    #[test]
    fn typed_frames_preserve_payload_alignment_and_decode_across_chunks() {
        #[derive(Debug, PartialEq, Codec)]
        struct Payload {
            name: String,
            values: Vec<u64>,
        }

        let value = Payload {
            name: "hi".into(),
            values: vec![1, 42],
        };
        let payload = value.encode_vec();
        let mut frame = Vec::new();
        encode_tagged_value_part(1, &value, &mut frame);
        assert_eq!(frame[0], 1);
        assert_eq!(&frame[1..9], &(payload.len() as u64).to_le_bytes());
        assert_eq!(&frame[9..], payload);

        for split in 0..=payload.len() {
            let mut bytes = payload[..split].chain(&payload[split..]);
            assert_eq!(decode_exact::<Payload, ()>(&mut bytes).unwrap(), value);
            assert!(!bytes.has_remaining());
        }
    }

    #[test]
    fn malformed_payloads_and_trailing_bytes_are_rejected_across_chunks() {
        for (payload, expected) in [
            (
                &[3, b'h', b'i'][..],
                RpcError::Codec(ql_codec::Error::UnexpectedEof),
            ),
            (&[1, 255][..], RpcError::Codec(ql_codec::Error::InvalidUtf8)),
            (
                &[2, b'h', b'i', 0][..],
                RpcError::Protocol(Error::TrailingBytes),
            ),
        ] {
            for split in 0..=payload.len() {
                let mut bytes = payload[..split].chain(&payload[split..]);
                let error = decode_exact::<String, ()>(&mut bytes).unwrap_err();
                assert_eq!(error, expected);
            }
        }
    }
}
