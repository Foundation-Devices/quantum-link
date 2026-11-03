use bytes::{Buf, BufMut};
use ql_codec::{Decode, Encode, Output, Reader, Writer};

use crate::Error;

rpc! {
    #[derive(Copy, Default)]
    pub struct Empty {}
}

pub fn encode<T: Encode, B: BufMut + ?Sized>(value: &T, out: &mut B) {
    value.encode(&mut Writer::new(BufMutOutput(out)));
}

pub fn decode<T, B: Buf>(bytes: &mut B) -> Result<T, Error>
where
    T: for<'a> Decode<&'a [u8]>,
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
    let value = T::decode(&mut reader)?;
    if !reader.is_empty() {
        return Err(Error::InvalidData);
    }
    bytes.advance(bytes.remaining());
    Ok(value)
}

struct BufMutOutput<'a, B: ?Sized>(&'a mut B);

impl<B: BufMut + ?Sized> Output for BufMutOutput<'_, B> {
    fn put_slice(&mut self, bytes: &[u8]) {
        self.0.put_slice(bytes);
    }
}
