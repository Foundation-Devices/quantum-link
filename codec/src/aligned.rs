use bytemuck::Pod;

use crate::{
    codec::len_prefix, varint, ByteSlice, Decode, Element, Encode, Error, Output, Reader, Ref,
    Writer,
};

#[cfg(target_endian = "big")]
compile_error!("Integer lists are lent as encoded, little-endian");

/// Zeros needed at `position` to align an integer of `size` bytes
#[inline]
fn padding(position: usize, size: usize) -> usize {
    position.next_multiple_of(size) - position
}

/// Encoded length of `len` integers of type `T`, without a length prefix
///
/// Up to `size_of::<T>() - 1` zeros precede them to align them, and the rest of those follow
/// them, so the length doesn't depend on where they're encoded.
#[inline]
fn encoded_len_ints<T: Pod>(len: usize) -> usize {
    size_of::<T>() - 1 + len * size_of::<T>()
}

fn encode_ints<T: Pod>(items: &[T], out: &mut Writer<impl Output>) {
    let padding = padding(out.position(), size_of::<T>());
    out.put_zeros(padding);
    out.put_slice(bytemuck::cast_slice(items));
    out.put_zeros(size_of::<T>() - 1 - padding);
}

/// Takes the bytes of `len` integers of type `T`, skipping the zeros around them
fn take_ints<T: Pod, B: ByteSlice>(reader: &mut Reader<B>, len: usize) -> Result<B, Error> {
    let size = size_of::<T>();
    let padding = padding(reader.position(), size);
    reader.take_n(padding)?;
    let bytes = reader.take_n(len.checked_mul(size).ok_or(Error::LengthOverflow)?)?;
    reader.take_n(size - 1 - padding)?;
    Ok(bytes)
}

fn take_list<T: Pod, B: ByteSlice>(reader: &mut Reader<B>) -> Result<B, Error> {
    let len = reader.decode_varint::<u32>()?;
    take_ints::<T, B>(reader, len as usize)
}

/// Copies integers out of `bytes`, which needn't be aligned
fn copy_ints<T: Pod>(bytes: &[u8]) -> Vec<T> {
    let mut items = vec![T::zeroed(); bytes.len() / size_of::<T>()];
    bytemuck::cast_slice_mut(&mut items).copy_from_slice(bytes);
    items
}

/// See [`copy_ints`]
fn copy_int_array<T: Pod, const N: usize>(bytes: &[u8]) -> [T; N] {
    let mut items = [T::zeroed(); N];
    bytemuck::cast_slice_mut(&mut items).copy_from_slice(bytes);
    items
}

/// Implements the codec traits for lists and arrays of a fixed-width integer
///
/// These are separate impls for each integer, because generic ones would overlap with the ones
/// for an [`Element`].
macro_rules! impl_int {
    ($($ty:ty),* $(,)?) => {
        $(
            impl Encode for [$ty] {
                #[inline]
                fn encoded_len(&self) -> usize {
                    varint::encoded_len(len_prefix(self.len()))
                        + encoded_len_ints::<$ty>(self.len())
                }

                fn encode(&self, out: &mut Writer<impl Output>) {
                    varint::encode(len_prefix(self.len()), out);
                    encode_ints(self, out);
                }
            }

            impl Encode for Vec<$ty> {
                #[inline]
                fn encoded_len(&self) -> usize {
                    self.as_slice().encoded_len()
                }

                fn encode(&self, out: &mut Writer<impl Output>) {
                    self.as_slice().encode(out);
                }
            }

            impl<B: ByteSlice> Decode<B> for Vec<$ty> {
                type Ref = Ref<B, [$ty]>;

                #[inline]
                fn decode(reader: &mut Reader<B>) -> Result<Self, Error> {
                    Ok(copy_ints(&take_list::<$ty, B>(reader)?))
                }

                #[inline]
                fn decode_ref(reader: &mut Reader<B>) -> Result<Self::Ref, Error> {
                    Ref::new(take_list::<$ty, B>(reader)?)
                }

                #[inline]
                fn from_ref(value: Self::Ref) -> Self {
                    value.to_vec()
                }

                #[inline]
                fn missing_ref() -> Option<Self::Ref> {
                    Ref::new(B::empty()).ok()
                }
            }

            impl Element for Vec<$ty> {}

            impl<const N: usize> Encode for [$ty; N] {
                #[inline]
                fn encoded_len(&self) -> usize {
                    encoded_len_ints::<$ty>(N)
                }

                fn encode(&self, out: &mut Writer<impl Output>) {
                    encode_ints(self, out);
                }
            }

            impl<B: ByteSlice, const N: usize> Decode<B> for [$ty; N] {
                type Ref = Ref<B, [$ty; N]>;

                #[inline]
                fn decode(reader: &mut Reader<B>) -> Result<Self, Error> {
                    Ok(copy_int_array(&take_ints::<$ty, B>(reader, N)?))
                }

                #[inline]
                fn decode_ref(reader: &mut Reader<B>) -> Result<Self::Ref, Error> {
                    Ref::new(take_ints::<$ty, B>(reader, N)?)
                }

                #[inline]
                fn from_ref(value: Self::Ref) -> Self {
                    *value
                }

                #[inline]
                fn missing() -> Option<Self> {
                    Some([0; N])
                }

                #[inline]
                fn missing_ref() -> Option<Self::Ref> {
                    let zeros = bytemuck::cast_slice::<$ty, u8>(&const { [0; N] });
                    Ref::new(B::from_static(zeros)?).ok()
                }
            }

            impl<const N: usize> Element for [$ty; N] {}
        )*
    };
}

impl_int!(u8, u16, u32, u64, i8, i16, i32, i64);
