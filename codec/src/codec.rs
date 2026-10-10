use std::{
    num::NonZeroU8,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use bytes::{Buf, BufMut, Bytes};

use crate::{varint, BufView, Decode, DecodeValue, Element, Encode, Error, Reader};

macro_rules! impl_codec {
    (fixed_integer: $($ty:ty),* $(,)?) => {
        $(
            impl Encode for $ty {
                #[inline]
                fn encoded_len(&self) -> usize {
                    size_of::<Self>()
                }

                fn encode<W: BufMut + ?Sized>(&self, out: &mut W) {
                    out.put_slice(&self.to_le_bytes());
                }
            }

            impl DecodeValue for $ty {
                #[inline]
                fn decode_value(reader: &mut Reader<'_>) -> Result<Self, Error> {
                    Ok(Self::from_le_bytes(*reader.take_array()?))
                }

                fn missing() -> Option<Self> {
                    Some(0)
                }
            }
        )*
    };
    (byte_buffer: $($ty:ty => $from_ref:expr),* $(,)?) => {
        $(
            impl Encode for $ty {
                #[inline]
                fn encoded_len(&self) -> usize {
                    encoded_len_bytes(self)
                }

                fn encode<W: BufMut + ?Sized>(&self, out: &mut W) {
                    encode_bytes(self, out);
                }
            }

            impl Decode for $ty {
                type Ref<'a> = &'a [u8];

                #[inline]
                fn decode_ref<'a>(reader: &mut Reader<'a>) -> Result<Self::Ref<'a>, Error> {
                    reader.take_len_prefixed()
                }

                #[inline]
                fn from_ref(value: &[u8]) -> Self {
                    $from_ref(value)
                }

                fn missing<'a>() -> Option<Self::Ref<'a>> {
                    Some(&[])
                }
            }
        )*
    };
    (tuple: $(($($name:ident $index:tt),+)),* $(,)?) => {
        $(
            impl<$($name: Encode),+> Encode for ($($name,)+) {
                fn encoded_len(&self) -> usize {
                    0 $(+ self.$index.encoded_len())+
                }

                fn encode<W: BufMut + ?Sized>(&self, out: &mut W) {
                    $(self.$index.encode(out);)+
                }
            }

            impl<$($name: Decode),+> Decode for ($($name,)+) {
                type Ref<'a> = ($($name::Ref<'a>,)+);

                fn decode_ref<'a>(reader: &mut Reader<'a>) -> Result<Self::Ref<'a>, Error> {
                    Ok(($($name::decode_ref(reader)?,)+))
                }

                fn from_ref(value: Self::Ref<'_>) -> Self {
                    ($($name::from_ref(value.$index),)+)
                }

                fn decode(reader: &mut Reader<'_>) -> Result<Self, Error> {
                    Ok(($($name::decode(reader)?,)+))
                }

                fn missing<'a>() -> Option<Self::Ref<'a>> {
                    Some(($($name::missing()?,)+))
                }
            }

            impl<$($name),+> Element for ($($name,)+) {}
        )*
    };
    (element: $($ty:ty),* $(,)?) => {
        $(impl Element for $ty {})*
    };
}

impl_codec!(fixed_integer: u8, u16, u32, u64, i8, i16, i32, i64);
impl_codec!(
    byte_buffer:
    Vec<u8> => <[u8]>::to_vec,
    Box<[u8]> => Box::from,
    Bytes => Bytes::copy_from_slice,
);
impl_codec!(tuple: (A 0, B 1), (A 0, B 1, C 2));
// Everything but `u8`, so `Vec<u8>` can borrow as `&[u8]`.
impl_codec!(
    element: u16, u32, u64, i8, i16, i32, i64, usize, bool, NonZeroU8, (),
    Vec<u8>, Box<[u8]>, Bytes, String, Duration, SystemTime,
);

impl<const N: usize> Encode for [u8; N] {
    fn encoded_len(&self) -> usize {
        N
    }

    fn encode<W: BufMut + ?Sized>(&self, out: &mut W) {
        out.put_slice(self);
    }
}

impl<const N: usize> Decode for [u8; N] {
    type Ref<'a> = &'a [u8; N];

    fn decode_ref<'a>(reader: &mut Reader<'a>) -> Result<Self::Ref<'a>, Error> {
        reader.take_array()
    }

    fn from_ref(value: &[u8; N]) -> Self {
        *value
    }

    fn missing<'a>() -> Option<Self::Ref<'a>> {
        Some(&const { [0; N] })
    }
}

impl<const N: usize> Element for [u8; N] {}

impl<const N: usize> Encode for Box<[u8; N]> {
    fn encoded_len(&self) -> usize {
        N
    }

    fn encode<W: BufMut + ?Sized>(&self, out: &mut W) {
        out.put_slice(self.as_ref());
    }
}

impl<const N: usize> Decode for Box<[u8; N]> {
    type Ref<'a> = &'a [u8; N];

    fn decode_ref<'a>(reader: &mut Reader<'a>) -> Result<Self::Ref<'a>, Error> {
        reader.take_array()
    }

    fn from_ref(value: &[u8; N]) -> Self {
        // Through a slice, because a large array would be copied through the stack.
        Box::<[u8]>::from(value.as_slice())
            .try_into()
            .expect("slice of an N byte array")
    }

    fn missing<'a>() -> Option<Self::Ref<'a>> {
        Some(&const { [0; N] })
    }
}

impl<const N: usize> Element for Box<[u8; N]> {}

impl<T: Encode + Element, const N: usize> Encode for [T; N] {
    fn encoded_len(&self) -> usize {
        self.iter().map(Encode::encoded_len).sum()
    }

    fn encode<W: BufMut + ?Sized>(&self, out: &mut W) {
        for item in self {
            item.encode(out);
        }
    }
}

impl<T: Decode + Element, const N: usize> Decode for [T; N] {
    type Ref<'a> = [T::Ref<'a>; N];

    fn decode_ref<'a>(reader: &mut Reader<'a>) -> Result<Self::Ref<'a>, Error> {
        decode_array(reader, T::decode_ref)
    }

    fn from_ref(value: Self::Ref<'_>) -> Self {
        value.map(T::from_ref)
    }

    fn decode(reader: &mut Reader<'_>) -> Result<Self, Error> {
        decode_array(reader, T::decode)
    }

    fn missing<'a>() -> Option<Self::Ref<'a>> {
        let items = (0..N).map(|_| T::missing()).collect::<Option<Vec<_>>>()?;
        Some(into_array(items))
    }
}

impl<T: Element, const N: usize> Element for [T; N] {}

fn decode_array<'a, T, const N: usize>(
    reader: &mut Reader<'a>,
    decode_item: impl Fn(&mut Reader<'a>) -> Result<T, Error>,
) -> Result<[T; N], Error> {
    let items = (0..N)
        .map(|_| decode_item(reader))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(into_array(items))
}

fn into_array<T, const N: usize>(items: Vec<T>) -> [T; N] {
    let Ok(array) = items.try_into() else {
        unreachable!("collected exactly N items");
    };
    array
}

impl Encode for [u8] {
    #[inline]
    fn encoded_len(&self) -> usize {
        encoded_len_bytes(self)
    }

    fn encode<W: BufMut + ?Sized>(&self, out: &mut W) {
        encode_bytes(self, out);
    }
}

impl<T: Encode + Element> Encode for Vec<T> {
    fn encoded_len(&self) -> usize {
        varint::encoded_len(len_prefix(self.len()))
            + self.iter().map(Encode::encoded_len).sum::<usize>()
    }

    fn encode<W: BufMut + ?Sized>(&self, out: &mut W) {
        varint::encode(len_prefix(self.len()), out);
        for item in self {
            item.encode(out);
        }
    }
}

impl<T: Decode + Element> Decode for Vec<T> {
    type Ref<'a> = Vec<T::Ref<'a>>;

    fn decode_ref<'a>(reader: &mut Reader<'a>) -> Result<Self::Ref<'a>, Error> {
        decode_list(reader, T::decode_ref)
    }

    fn from_ref(value: Self::Ref<'_>) -> Self {
        value.into_iter().map(T::from_ref).collect()
    }

    fn decode(reader: &mut Reader<'_>) -> Result<Self, Error> {
        decode_list(reader, T::decode)
    }

    fn missing<'a>() -> Option<Self::Ref<'a>> {
        Some(Vec::new())
    }
}

impl<T: Element> Element for Vec<T> {}

fn decode_list<'a, T>(
    reader: &mut Reader<'a>,
    decode_item: impl Fn(&mut Reader<'a>) -> Result<T, Error>,
) -> Result<Vec<T>, Error> {
    let len = reader.decode_varint::<u32>()? as usize;
    // At most as much memory as the input, or a corrupt length could reserve gigabytes.
    let mut items = Vec::with_capacity(len.min(reader.remaining_len() / size_of::<T>().max(1)));
    for _ in 0..len {
        items.push(decode_item(reader)?);
    }
    Ok(items)
}

impl Encode for str {
    #[inline]
    fn encoded_len(&self) -> usize {
        self.as_bytes().encoded_len()
    }

    fn encode<W: BufMut + ?Sized>(&self, out: &mut W) {
        self.as_bytes().encode(out);
    }
}

impl Encode for String {
    #[inline]
    fn encoded_len(&self) -> usize {
        self.as_str().encoded_len()
    }

    fn encode<W: BufMut + ?Sized>(&self, out: &mut W) {
        self.as_str().encode(out);
    }
}

impl Decode for String {
    type Ref<'a> = &'a str;

    #[inline]
    fn decode_ref<'a>(reader: &mut Reader<'a>) -> Result<Self::Ref<'a>, Error> {
        std::str::from_utf8(reader.take_len_prefixed()?).map_err(|_| Error::InvalidUtf8)
    }

    #[inline]
    fn from_ref(value: &str) -> Self {
        value.to_owned()
    }

    fn missing<'a>() -> Option<Self::Ref<'a>> {
        Some("")
    }
}

impl Encode for NonZeroU8 {
    #[inline]
    fn encoded_len(&self) -> usize {
        size_of::<u8>()
    }

    fn encode<W: BufMut + ?Sized>(&self, out: &mut W) {
        out.put_u8(self.get());
    }
}

impl DecodeValue for NonZeroU8 {
    #[inline]
    fn decode_value(reader: &mut Reader<'_>) -> Result<Self, Error> {
        Self::new(reader.take_u8()?).ok_or(Error::InvalidData)
    }
}

/// `usize` is a varint, because its width differs between targets
///
/// A target too narrow for the value fails to decode it with [`Error::InvalidRange`].
impl Encode for usize {
    #[inline]
    fn encoded_len(&self) -> usize {
        varint::encoded_len(*self as u64)
    }

    fn encode<W: BufMut + ?Sized>(&self, out: &mut W) {
        varint::encode(*self as u64, out);
    }
}

impl DecodeValue for usize {
    #[inline]
    fn decode_value(reader: &mut Reader<'_>) -> Result<Self, Error> {
        Self::try_from(reader.decode_varint::<u64>()?).map_err(|_| Error::InvalidRange)
    }

    fn missing() -> Option<Self> {
        Some(0)
    }
}

impl Encode for bool {
    #[inline]
    fn encoded_len(&self) -> usize {
        size_of::<u8>()
    }

    fn encode<W: BufMut + ?Sized>(&self, out: &mut W) {
        out.put_u8(u8::from(*self));
    }
}

impl DecodeValue for bool {
    #[inline]
    fn decode_value(reader: &mut Reader<'_>) -> Result<Self, Error> {
        match reader.take_u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::InvalidDiscriminant),
        }
    }

    fn missing() -> Option<Self> {
        Some(false)
    }
}

impl Encode for () {
    fn encoded_len(&self) -> usize {
        0
    }

    fn encode<W: BufMut + ?Sized>(&self, _out: &mut W) {}
}

impl DecodeValue for () {
    fn decode_value(_reader: &mut Reader<'_>) -> Result<Self, Error> {
        Ok(())
    }

    fn missing() -> Option<Self> {
        Some(())
    }
}

impl<T: Encode> Encode for Option<T> {
    fn encoded_len(&self) -> usize {
        1 + self.as_ref().map_or(0, Encode::encoded_len)
    }

    fn encode<W: BufMut + ?Sized>(&self, out: &mut W) {
        match self {
            None => out.put_u8(0),
            Some(inner) => {
                out.put_u8(1);
                inner.encode(out);
            }
        }
    }
}

impl<T: Decode> Decode for Option<T> {
    type Ref<'a> = Option<T::Ref<'a>>;

    fn decode_ref<'a>(reader: &mut Reader<'a>) -> Result<Self::Ref<'a>, Error> {
        decode_option(reader, T::decode_ref)
    }

    fn from_ref(value: Self::Ref<'_>) -> Self {
        value.map(T::from_ref)
    }

    fn decode(reader: &mut Reader<'_>) -> Result<Self, Error> {
        decode_option(reader, T::decode)
    }

    fn missing<'a>() -> Option<Self::Ref<'a>> {
        Some(None)
    }
}

impl<T> Element for Option<T> {}

fn decode_option<'a, T>(
    reader: &mut Reader<'a>,
    decode_value: impl Fn(&mut Reader<'a>) -> Result<T, Error>,
) -> Result<Option<T>, Error> {
    match reader.take_u8()? {
        0 => Ok(None),
        1 => Ok(Some(decode_value(reader)?)),
        _ => Err(Error::InvalidDiscriminant),
    }
}

impl<T: Encode, E: Encode> Encode for Result<T, E> {
    fn encoded_len(&self) -> usize {
        1 + match self {
            Ok(value) => value.encoded_len(),
            Err(error) => error.encoded_len(),
        }
    }

    fn encode<W: BufMut + ?Sized>(&self, out: &mut W) {
        match self {
            Ok(value) => {
                out.put_u8(0);
                value.encode(out);
            }
            Err(error) => {
                out.put_u8(1);
                error.encode(out);
            }
        }
    }
}

impl<T: Decode, E: Decode> Decode for Result<T, E> {
    type Ref<'a> = Result<T::Ref<'a>, E::Ref<'a>>;

    fn decode_ref<'a>(reader: &mut Reader<'a>) -> Result<Self::Ref<'a>, Error> {
        decode_result(reader, T::decode_ref, E::decode_ref)
    }

    fn from_ref(value: Self::Ref<'_>) -> Self {
        value.map(T::from_ref).map_err(E::from_ref)
    }

    fn decode(reader: &mut Reader<'_>) -> Result<Self, Error> {
        decode_result(reader, T::decode, E::decode)
    }
}

impl<T, E> Element for Result<T, E> {}

fn decode_result<'a, T, E>(
    reader: &mut Reader<'a>,
    decode_value: impl Fn(&mut Reader<'a>) -> Result<T, Error>,
    decode_error: impl Fn(&mut Reader<'a>) -> Result<E, Error>,
) -> Result<Result<T, E>, Error> {
    match reader.take_u8()? {
        0 => Ok(Ok(decode_value(reader)?)),
        1 => Ok(Err(decode_error(reader)?)),
        _ => Err(Error::InvalidDiscriminant),
    }
}

impl Encode for Duration {
    fn encoded_len(&self) -> usize {
        size_of::<u64>() + size_of::<u32>()
    }

    fn encode<W: BufMut + ?Sized>(&self, out: &mut W) {
        self.as_secs().encode(out);
        self.subsec_nanos().encode(out);
    }
}

impl DecodeValue for Duration {
    fn decode_value(reader: &mut Reader<'_>) -> Result<Self, Error> {
        let secs: u64 = reader.decode()?;
        let nanos: u32 = reader.decode()?;
        if nanos >= 1_000_000_000 {
            return Err(Error::InvalidRange);
        }
        Ok(Self::new(secs, nanos))
    }

    fn missing() -> Option<Self> {
        Some(Self::ZERO)
    }
}

/// Encodes as the [`Duration`] since the Unix epoch, which earlier times clamp to.
impl Encode for SystemTime {
    fn encoded_len(&self) -> usize {
        Duration::ZERO.encoded_len()
    }

    fn encode<W: BufMut + ?Sized>(&self, out: &mut W) {
        self.duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .encode(out);
    }
}

impl DecodeValue for SystemTime {
    fn decode_value(reader: &mut Reader<'_>) -> Result<Self, Error> {
        UNIX_EPOCH
            .checked_add(reader.decode()?)
            .ok_or(Error::InvalidRange)
    }
}

/// Length prefixes are `u32`, so a 64-bit host cannot emit one a 32-bit peer would reject: a
/// `usize` varint is capped at ten bytes on one target and five on the other.
#[track_caller]
#[inline]
fn len_prefix(len: usize) -> u32 {
    u32::try_from(len).expect("length prefix above u32::MAX")
}

/// Encoded length of `len` bytes behind their length prefix
#[inline]
pub fn encoded_len_prefixed(len: usize) -> usize {
    varint::encoded_len(len_prefix(len)) + len
}

/// Writes the length prefix for `len` bytes
pub fn encode_len_prefix<W: BufMut + ?Sized>(len: usize, out: &mut W) {
    varint::encode(len_prefix(len), out);
}

pub fn encoded_len_bytes<B: BufView + ?Sized>(bytes: &B) -> usize {
    encoded_len_prefixed(bytes.buf().remaining())
}

pub fn encode_bytes<B, W>(bytes: &B, out: &mut W)
where
    B: BufView + ?Sized,
    W: BufMut + ?Sized,
{
    encode_len_prefix(bytes.buf().remaining(), out);
    encode_bytes_raw(bytes, out);
}

/// Writes the bytes without a length prefix, for a field that runs to the end of the message.
pub fn encode_bytes_raw<B, W>(bytes: &B, out: &mut W)
where
    B: BufView + ?Sized,
    W: BufMut + ?Sized,
{
    // `BufMut::put` needs a sized writer, so feed it a chunk at a time.
    let mut bytes = bytes.buf();
    while bytes.has_remaining() {
        let chunk = bytes.chunk();
        out.put_slice(chunk);
        bytes.advance(chunk.len());
    }
}

/// Decodes the next field of a struct that isn't frozen, or its missing value once the encoding
/// ran out
///
/// A field cut short is still an error.
pub fn decode_field<T: Decode>(fields: &mut Reader<'_>) -> Result<T, Error> {
    if fields.is_empty() {
        T::missing().map(T::from_ref).ok_or(Error::UnexpectedEof)
    } else {
        T::decode(fields)
    }
}

/// Like [`decode_field`], but borrows byte buffers and strings from `fields`
pub fn decode_field_ref<'a, T: Decode>(fields: &mut Reader<'a>) -> Result<T::Ref<'a>, Error> {
    if fields.is_empty() {
        T::missing().ok_or(Error::UnexpectedEof)
    } else {
        T::decode_ref(fields)
    }
}
