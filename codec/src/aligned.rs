use core::ops::Deref;

use crate::{ByteSlice, Decode, Encode, Error, Reader, Ref};

/// an integer collection decoded as a typed reference into aligned encoding bytes
///
/// the encoding is unchanged. the actual payload address must be aligned and multibyte
/// integers require a little endian target. owned decoding reads the integers normally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(transparent)]
pub struct Aligned<T>(pub T);

impl<T: Encode> Encode for Aligned<T> {
    fn encoded_len(&self) -> usize {
        self.0.encoded_len()
    }

    fn encode<W: bytes::BufMut + ?Sized>(&self, out: &mut W) {
        self.0.encode(out);
    }
}

impl<T> Deref for Aligned<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

macro_rules! impl_decode_aligned {
    ($($ty:ty),* $(,)?) => {
        $(
            impl <B: ByteSlice> Decode<B> for Aligned<Vec<$ty>> {
                type Ref = Ref<B, [$ty]>;

                fn decode(reader: &mut Reader<B>) -> Result<Self, Error> {
                    Vec::<$ty>::decode(reader).map(Self)
                }

                fn decode_ref(reader: &mut Reader<B>) -> Result<Self::Ref, Error> {
                    let len = reader.decode_varint::<u32>()? as usize;
                    let byte_len = len.checked_mul(size_of::<$ty>()).ok_or(Error::LengthOverflow)?;
                    Ref::new(reader.take_n(byte_len)?)
                }

                fn from_ref(value: Self::Ref) -> Self {
                    Self(value.to_vec())
                }

                fn missing_ref() -> Option<Self::Ref> {
                    Ref::new(B::empty()).ok()
                }
            }

            impl <B: ByteSlice, const N: usize> Decode<B> for Aligned<[$ty; N]> {
                type Ref = Ref<B, [$ty; N]>;

                fn decode(reader: &mut Reader<B>) -> Result<Self, Error> {
                    <[$ty; N]>::decode(reader).map(Self)
                }

                fn decode_ref(reader: &mut Reader<B>) -> Result<Self::Ref, Error> {
                    Ref::new(reader.take_n(size_of::<[$ty; N]>())?)
                }

                fn from_ref(value: Self::Ref) -> Self {
                    Self(*value)
                }

                fn missing() -> Option<Self> {
                    Some(Self([0; N]))
                }

                fn missing_ref() -> Option<Self::Ref> {
                    let zeros = &const { [0 as $ty; N] };
                    // safety: fixed width integers contain no padding and static zeros remain initialized
                    let bytes = unsafe { std::slice::from_raw_parts(zeros.as_ptr().cast(), size_of::<[$ty; N]>()) };
                    Ref::new(B::from_static(bytes)?).ok()
                }
            }
        )*
    };
}

impl_decode_aligned!(u8, u16, u32, u64, i8, i16, i32, i64);
