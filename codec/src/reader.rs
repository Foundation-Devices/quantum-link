use crate::{varint, ByteSlice, Decode, Error, Ref};

#[derive(Clone)]
pub struct Reader<B> {
    remaining: B,
    /// Position of the end of `remaining`
    end: usize,
}

impl<B: ByteSlice> Reader<B> {
    /// Reads an encoding from its start
    #[inline]
    pub fn new(bytes: B) -> Self {
        let end = bytes.len();
        Self {
            remaining: bytes,
            end,
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.remaining.is_empty()
    }

    #[inline]
    pub fn remaining_len(&self) -> usize {
        self.remaining.len()
    }

    /// Bytes read since the encoding started
    #[inline]
    pub fn position(&self) -> usize {
        self.end - self.remaining.len()
    }

    #[inline]
    pub fn take_n(&mut self, len: usize) -> Result<B, Error> {
        if len > self.remaining.len() {
            return Err(Error::UnexpectedEof);
        }
        Ok(self.remaining.split_off_front(len))
    }

    #[inline]
    pub fn take_array<const N: usize>(&mut self) -> Result<Ref<B, [u8; N]>, Error> {
        Ref::new(self.take_n(N)?)
    }

    #[inline]
    pub fn take_u8(&mut self) -> Result<u8, Error> {
        self.remaining.take_u8().ok_or(Error::UnexpectedEof)
    }

    #[inline]
    pub fn take_all(&mut self) -> B {
        self.remaining.split_off_front(self.remaining.len())
    }

    #[inline]
    pub fn take_len_prefixed(&mut self) -> Result<B, Error> {
        let len = self.decode_varint::<u32>()?;
        self.take_n(len as usize)
    }

    /// Like [`Reader::take_len_prefixed`], but reads the bytes as part of this encoding, so
    /// integer lists in them stay aligned
    #[inline]
    pub fn take_len_prefixed_reader(&mut self) -> Result<Self, Error> {
        let remaining = self.take_len_prefixed()?;
        Ok(Self {
            remaining,
            end: self.position(),
        })
    }

    #[inline]
    pub fn decode<T: Decode<B>>(&mut self) -> Result<T, Error> {
        T::decode(self)
    }

    #[inline]
    pub fn decode_ref<T: Decode<B>>(&mut self) -> Result<T::Ref, Error> {
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
