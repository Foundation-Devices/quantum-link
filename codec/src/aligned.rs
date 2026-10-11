use core::ops::Deref;

use crate::{Decode, Encode, Error, Reader};

/// a numeric collection borrowed by casting aligned encoding bytes
///
/// fixed width integer vectors and arrays borrow as slices and array references without
/// allocating. the encoding is unchanged, so callers must align the actual payload address.
/// misaligned input returns [`Error::InvalidAlignment`]. multibyte integers require a little
/// endian target. owned decoding reads the integers normally.
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
            impl Decode for Aligned<Vec<$ty>> {
                type Ref<'a> = &'a [$ty];

                fn decode_ref<'a>(reader: &mut Reader<'a>) -> Result<Self::Ref<'a>, Error> {
                    let len = reader.decode_varint::<u32>()? as usize;
                    // safety: these fixed width integers accept every bit pattern
                    unsafe { decode_slice::<$ty>(reader, len) }
                }

                fn from_ref(value: &[$ty]) -> Self {
                    Self(value.to_vec())
                }

                fn decode(reader: &mut Reader<'_>) -> Result<Self, Error> {
                    Vec::<$ty>::decode(reader).map(Self)
                }

                fn missing<'a>() -> Option<Self::Ref<'a>> {
                    Some(&[])
                }
            }

            impl<const N: usize> Decode for Aligned<[$ty; N]> {
                type Ref<'a> = &'a [$ty; N];

                fn decode_ref<'a>(reader: &mut Reader<'a>) -> Result<Self::Ref<'a>, Error> {
                    // safety: these fixed width integers accept every bit pattern
                    let items = unsafe { decode_slice::<$ty>(reader, N)? };
                    Ok(items
                        .try_into()
                        .expect("slice of exactly N elements"))
                }

                fn from_ref(value: &[$ty; N]) -> Self {
                    Self(*value)
                }

                fn decode(reader: &mut Reader<'_>) -> Result<Self, Error> {
                    <[$ty; N]>::decode(reader).map(Self)
                }

                fn missing<'a>() -> Option<Self::Ref<'a>> {
                    Some(&const { [0; N] })
                }
            }
        )*
    };
}

impl_decode_aligned!(u8, u16, u32, u64, i8, i16, i32, i64);

// T must be a fixed width integer that accepts every bit pattern
unsafe fn decode_slice<'a, T>(reader: &mut Reader<'a>, len: usize) -> Result<&'a [T], Error> {
    if len == 0 {
        return Ok(&[]);
    }
    if size_of::<T>() > 1 && !cfg!(target_endian = "little") {
        return Err(Error::UnsupportedEndianness);
    }
    let byte_len = len
        .checked_mul(size_of::<T>())
        .ok_or(Error::LengthOverflow)?;
    let bytes = reader.take_n(byte_len)?;
    let ptr = bytes.as_ptr().cast::<T>();
    if !ptr.is_aligned() {
        return Err(Error::InvalidAlignment);
    }
    // safety: the slice covers len aligned elements and T accepts every bit pattern
    // the input remains borrowed for 'a
    Ok(unsafe { std::slice::from_raw_parts(ptr, len) })
}
