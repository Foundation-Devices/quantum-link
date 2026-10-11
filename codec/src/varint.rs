use core::{fmt, ops::Deref};

use bytes::BufMut;

use crate::{ByteSlice, DecodeValue, Encode, Error, Reader};

/// An integer field carried as a varint
///
/// The plain integer codecs are fixed width, so a field only encodes as a
/// varint when it is wrapped here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Varint<T>(pub T);

pub trait Primitive: Copy {
    const MAX_ENCODED_LEN: usize;
    const LAST_PAYLOAD_MAX: u8;

    fn from_u8(value: u8) -> Self;
    fn low_7_bits(self) -> u8;
    fn shr_7(self) -> Self;
    fn needs_more(self) -> bool;
    fn add_payload(self, payload: u8, shift: usize) -> Self;
}

impl<T: Primitive> Varint<T> {
    pub const MAX_ENCODED_LEN: usize = T::MAX_ENCODED_LEN;
}

impl<T> Deref for Varint<T> {
    type Target = T;

    #[inline]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: fmt::Display> fmt::Display for Varint<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<T: Primitive> Encode for Varint<T> {
    fn encoded_len(&self) -> usize {
        encoded_len(self.0)
    }

    fn encode<W: BufMut + ?Sized>(&self, out: &mut W) {
        encode(self.0, out);
    }
}

impl<T: Primitive> DecodeValue for Varint<T> {
    fn decode_value<B: ByteSlice>(reader: &mut Reader<B>) -> Result<Self, Error> {
        self::decode(reader).map(Self)
    }

    fn missing() -> Option<Self> {
        Some(Self(T::from_u8(0)))
    }
}

pub fn encoded_len<T: Primitive>(mut value: T) -> usize {
    let mut len = 1;
    while value.needs_more() {
        value = value.shr_7();
        len += 1;
    }
    len
}

pub fn encode<T, W>(mut value: T, out: &mut W)
where
    T: Primitive,
    W: BufMut + ?Sized,
{
    while value.needs_more() {
        out.put_u8(value.low_7_bits() | 0x80);
        value = value.shr_7();
    }
    out.put_u8(value.low_7_bits());
}

#[inline]
pub fn decode<T, B>(reader: &mut Reader<B>) -> Result<T, Error>
where
    T: Primitive,
    B: ByteSlice,
{
    let first = reader.take_u8()?;
    if first < 0x80 {
        return Ok(T::from_u8(first));
    }

    let mut value = T::from_u8(first & 0x7f);
    for index in 1..T::MAX_ENCODED_LEN - 1 {
        let byte = reader.take_u8()?;
        value = value.add_payload(byte & 0x7f, index * 7);
        if byte < 0x80 {
            return Ok(value);
        }
    }

    let last = reader.take_u8()?;
    if last > T::LAST_PAYLOAD_MAX {
        return Err(Error::InvalidVarint);
    }
    Ok(value.add_payload(last, (T::MAX_ENCODED_LEN - 1) * 7))
}

macro_rules! impl_varint {
    ($($ty:ty),* $(,)?) => {
        $(
            impl Primitive for $ty {
                const MAX_ENCODED_LEN: usize = (size_of::<Self>() * 8).div_ceil(7);
                #[allow(clippy::cast_possible_truncation)]
                const LAST_PAYLOAD_MAX: u8 = (Self::MAX >> ((Self::MAX_ENCODED_LEN - 1) * 7)) as u8;

                #[inline]
                fn from_u8(value: u8) -> Self {
                    Self::from(value)
                }
                #[inline]
                #[allow(clippy::cast_possible_truncation)]
                fn low_7_bits(self) -> u8 {
                    (self & 0x7f) as u8
                }
                #[inline]
                fn shr_7(self) -> Self {
                    self >> 7
                }
                #[inline]
                fn needs_more(self) -> bool {
                    self >= 0x80
                }
                #[inline]
                fn add_payload(self, payload: u8, shift: usize) -> Self {
                    self | (Self::from(payload) << shift)
                }
            }
        )*
    };
}

impl_varint!(u8, u16, u32, u64, usize);

#[cfg(test)]
mod tests {
    use std::fmt::Debug;

    use super::*;

    fn assert_decodes<T>(value: T)
    where
        T: Primitive + Debug + PartialEq,
    {
        let mut out = Vec::new();
        encode(value, &mut out);
        assert_eq!(out.len(), encoded_len(value));
        let mut reader = Reader::new(out.as_slice());
        assert_eq!(decode(&mut reader), Ok(value));
        assert!(reader.is_empty());
    }

    fn assert_error<T>(bytes: &[u8], error: Error)
    where
        T: Primitive + Debug + PartialEq,
    {
        let mut reader = Reader::new(bytes);
        assert_eq!(decode::<T, _>(&mut reader), Err(error));
    }

    fn test_type<T>(max: T)
    where
        T: Primitive + Debug + PartialEq + TryFrom<u64>,
    {
        for value in [0, 1, 127].map(T::from_u8) {
            assert_decodes(value);
        }
        for shift in (7..size_of::<T>() * 8).step_by(7) {
            let boundary = 1u64 << shift;
            if let (Ok(before), Ok(after)) = (T::try_from(boundary - 1), T::try_from(boundary)) {
                assert_decodes(before);
                assert_decodes(after);
            }
        }
        assert_decodes(max);

        let mut encoded = Vec::new();
        encode(T::from_u8(127), &mut encoded);
        encoded.push(0x55);
        let mut reader = Reader::new(encoded.as_slice());
        assert_eq!(decode(&mut reader), Ok(T::from_u8(127)));
        assert_eq!(reader.take_all(), &[0x55]);

        assert_error::<T>(&[0x80], Error::UnexpectedEof);
        assert_error::<T>(&vec![0xff; T::MAX_ENCODED_LEN], Error::InvalidVarint);

        let bit_width = size_of::<T>() * 8;
        let mut overflow = vec![0x80; bit_width / 7];
        overflow.push(1 << (bit_width % 7));
        assert_error::<T>(&overflow, Error::InvalidVarint);
    }

    macro_rules! test_types {
        ($($ty:ident),* $(,)?) => {
            $(
                #[test]
                fn $ty() {
                    test_type::<$ty>(<$ty>::MAX);
                }
            )*
        };
    }

    test_types!(u8, u16, u32, u64, usize);
}
