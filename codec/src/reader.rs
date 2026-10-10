use crate::{varint, Decode, Error};

#[derive(Clone)]
pub struct Reader<'a> {
    remaining: &'a [u8],
}

impl<'a> Reader<'a> {
    #[inline]
    pub fn new(bytes: &'a [u8]) -> Self {
        Self { remaining: bytes }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.remaining.is_empty()
    }

    #[inline]
    pub fn remaining_len(&self) -> usize {
        self.remaining.len()
    }

    #[inline]
    pub fn take_n(&mut self, len: usize) -> Result<&'a [u8], Error> {
        let (head, tail) = self
            .remaining
            .split_at_checked(len)
            .ok_or(Error::UnexpectedEof)?;
        self.remaining = tail;
        Ok(head)
    }

    pub fn take_array<const N: usize>(&mut self) -> Result<&'a [u8; N], Error> {
        let (head, tail) = self
            .remaining
            .split_first_chunk()
            .ok_or(Error::UnexpectedEof)?;
        self.remaining = tail;
        Ok(head)
    }

    #[inline]
    pub fn take_u8(&mut self) -> Result<u8, Error> {
        let (&byte, tail) = self.remaining.split_first().ok_or(Error::UnexpectedEof)?;
        self.remaining = tail;
        Ok(byte)
    }

    #[inline]
    pub fn take_all(&mut self) -> &'a [u8] {
        std::mem::take(&mut self.remaining)
    }

    #[inline]
    pub fn take_len_prefixed(&mut self) -> Result<&'a [u8], Error> {
        let len = self.decode_varint::<u32>()?;
        self.take_n(len as usize)
    }

    #[inline]
    pub fn decode<T: Decode>(&mut self) -> Result<T, Error> {
        T::decode(self)
    }

    #[inline]
    pub fn decode_ref<T: Decode>(&mut self) -> Result<T::Ref<'a>, Error> {
        T::decode_ref(self)
    }

    #[inline]
    pub fn decode_varint<T>(&mut self) -> Result<T, Error>
    where
        T: varint::Primitive,
    {
        varint::decode(self)
    }
}
