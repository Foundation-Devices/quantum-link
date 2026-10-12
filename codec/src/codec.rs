use std::{
    collections::BTreeMap,
    num::NonZeroU8,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use bytes::{Buf, Bytes};

use crate::{
    varint, BufView, ByteSlice, Decode, DecodeValue, Element, Encode, Error, Output, Reader, Ref,
    Writer,
};

macro_rules! impl_codec {
    (fixed_integer: $($ty:ty),* $(,)?) => {
        $(
            impl Encode for $ty {
                #[inline]
                fn encoded_len(&self) -> usize {
                    size_of::<Self>()
                }

                fn encode(&self, out: &mut Writer<impl Output>) {
                    out.put_slice(&self.to_le_bytes());
                }
            }

            impl DecodeValue for $ty {
                #[inline]
                fn decode_value<B: ByteSlice>(reader: &mut Reader<B>) -> Result<Self, Error> {
                    let bytes = reader.take_n(size_of::<Self>())?;
                    Ok(Self::from_le_bytes((&*bytes).try_into().unwrap()))
                }

                #[inline]
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

                fn encode(&self, out: &mut Writer<impl Output>) {
                    encode_bytes(self, out);
                }
            }

            impl <B: ByteSlice> Decode<B> for $ty {
                type Ref = Ref<B, [u8]>;

                #[inline]
                fn decode_ref(reader: &mut Reader<B>) -> Result<Self::Ref, Error> {
                    Ref::new(reader.take_len_prefixed()?)
                }

                #[inline]
                fn from_ref(value: Self::Ref) -> Self {
                    $from_ref(&*value)
                }

                #[inline]
                fn missing_ref() -> Option<Self::Ref> {
                    Ref::new(B::from_static(&[])?).ok()
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

                fn encode(&self, out: &mut Writer<impl Output>) {
                    $(self.$index.encode(out);)+
                }
            }

            impl <B: ByteSlice, $($name: Decode<B>),+> Decode<B> for ($($name,)+) {
                type Ref = ($($name::Ref,)+);

                fn decode(reader: &mut Reader<B>) -> Result<Self, Error> {
                    Ok(($($name::decode(reader)?,)+))
                }

                fn decode_ref(reader: &mut Reader<B>) -> Result<Self::Ref, Error> {
                    Ok(($($name::decode_ref(reader)?,)+))
                }

                fn from_ref(value: Self::Ref) -> Self {
                    ($($name::from_ref(value.$index),)+)
                }

                fn missing() -> Option<Self> {
                    Some(($($name::missing()?,)+))
                }

                fn missing_ref() -> Option<Self::Ref> {
                    Some(($($name::missing_ref()?,)+))
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
    Box<[u8]> => Box::from,
    Bytes => Bytes::copy_from_slice,
);
impl_codec!(tuple: (T0 0, T1 1), (T0 0, T1 1, T2 2));
// Not the fixed-width integers, so their lists can borrow as `Ref<B, [T]>`.
impl_codec!(
    element: usize, bool, NonZeroU8, (), Box<[u8]>, Bytes, String, Duration, SystemTime,
);

impl<const N: usize> Encode for Box<[u8; N]> {
    fn encoded_len(&self) -> usize {
        N
    }

    fn encode(&self, out: &mut Writer<impl Output>) {
        (**self).encode(out);
    }
}

impl<B: ByteSlice, const N: usize> Decode<B> for Box<[u8; N]> {
    type Ref = Ref<B, [u8; N]>;

    fn decode_ref(reader: &mut Reader<B>) -> Result<Self::Ref, Error> {
        <[u8; N]>::decode_ref(reader)
    }

    fn from_ref(value: Self::Ref) -> Self {
        // copy through a slice to avoid putting a large array on the stack
        Box::<[u8]>::from(value.as_slice())
            .try_into()
            .expect("slice of an N byte array")
    }

    fn missing() -> Option<Self> {
        Some(
            Box::<[u8]>::from(&const { [0; N] }[..])
                .try_into()
                .expect("slice of an N byte array"),
        )
    }

    fn missing_ref() -> Option<Self::Ref> {
        <[u8; N]>::missing_ref()
    }
}

impl<const N: usize> Element for Box<[u8; N]> {}

impl<T: Encode + Element, const N: usize> Encode for [T; N] {
    fn encoded_len(&self) -> usize {
        self.iter().map(Encode::encoded_len).sum()
    }

    fn encode(&self, out: &mut Writer<impl Output>) {
        for item in self {
            item.encode(out);
        }
    }
}

impl<B: ByteSlice, T: Decode<B> + Element, const N: usize> Decode<B> for [T; N] {
    type Ref = [T::Ref; N];

    fn decode(reader: &mut Reader<B>) -> Result<Self, Error> {
        decode_array(|| T::decode(reader))
    }

    fn decode_ref(reader: &mut Reader<B>) -> Result<Self::Ref, Error> {
        decode_array(|| T::decode_ref(reader))
    }

    fn from_ref(value: Self::Ref) -> Self {
        value.map(T::from_ref)
    }

    fn missing() -> Option<Self> {
        decode_array(|| T::missing().ok_or(())).ok()
    }

    fn missing_ref() -> Option<Self::Ref> {
        decode_array(|| T::missing_ref().ok_or(())).ok()
    }
}

impl<T: Element, const N: usize> Element for [T; N] {}

fn decode_array<T, E, const N: usize>(
    mut decode_item: impl FnMut() -> Result<T, E>,
) -> Result<[T; N], E> {
    let mut error = None;
    let items: [Option<T>; N] = std::array::from_fn(|_| {
        if error.is_some() {
            return None;
        }
        match decode_item() {
            Ok(item) => Some(item),
            Err(err) => {
                error = Some(err);
                None
            }
        }
    });
    match error {
        Some(error) => Err(error),
        None => Ok(items.map(|item| item.expect("decoded every array item"))),
    }
}

impl<T: Encode + Element> Encode for [T] {
    fn encoded_len(&self) -> usize {
        varint::encoded_len(len_prefix(self.len()))
            + self.iter().map(Encode::encoded_len).sum::<usize>()
    }

    fn encode(&self, out: &mut Writer<impl Output>) {
        varint::encode(len_prefix(self.len()), out);
        for item in self {
            item.encode(out);
        }
    }
}

impl<T: Encode + Element> Encode for Vec<T> {
    fn encoded_len(&self) -> usize {
        self.as_slice().encoded_len()
    }

    fn encode(&self, out: &mut Writer<impl Output>) {
        self.as_slice().encode(out);
    }
}

impl<B: ByteSlice, T: Decode<B> + Element> Decode<B> for Vec<T> {
    type Ref = Vec<T::Ref>;

    fn decode(reader: &mut Reader<B>) -> Result<Self, Error> {
        decode_list(reader, T::decode)
    }

    fn decode_ref(reader: &mut Reader<B>) -> Result<Self::Ref, Error> {
        decode_list(reader, T::decode_ref)
    }

    fn from_ref(value: Self::Ref) -> Self {
        value.into_iter().map(T::from_ref).collect()
    }

    fn missing_ref() -> Option<Self::Ref> {
        Some(Vec::new())
    }
}

impl<T: Element> Element for Vec<T> {}

fn decode_list<T, B: ByteSlice>(
    reader: &mut Reader<B>,
    decode_item: impl Fn(&mut Reader<B>) -> Result<T, Error>,
) -> Result<Vec<T>, Error> {
    let len = reader.decode_varint::<u32>()? as usize;
    // At most as much memory as the input, or a corrupt length could reserve gigabytes.
    let mut items = Vec::with_capacity(len.min(reader.remaining_len() / size_of::<T>().max(1)));
    for _ in 0..len {
        items.push(decode_item(reader)?);
    }
    Ok(items)
}

/// Encodes like a list of key and value pairs
impl<K: Encode, V: Encode> Encode for BTreeMap<K, V> {
    fn encoded_len(&self) -> usize {
        varint::encoded_len(len_prefix(self.len()))
            + self
                .iter()
                .map(|(key, value)| key.encoded_len() + value.encoded_len())
                .sum::<usize>()
    }

    fn encode(&self, out: &mut Writer<impl Output>) {
        varint::encode(len_prefix(self.len()), out);
        for (key, value) in self {
            key.encode(out);
            value.encode(out);
        }
    }
}

/// a key that occurs twice keeps its last value
impl<B, K, V> Decode<B> for BTreeMap<K, V>
where
    B: ByteSlice,
    K: Decode<B> + Ord,
    V: Decode<B>,
    K::Ref: Ord,
{
    type Ref = BTreeMap<K::Ref, V::Ref>;

    fn decode(reader: &mut Reader<B>) -> Result<Self, Error> {
        decode_map(reader, K::decode, V::decode)
    }

    fn decode_ref(reader: &mut Reader<B>) -> Result<Self::Ref, Error> {
        decode_map(reader, K::decode_ref, V::decode_ref)
    }

    fn from_ref(value: Self::Ref) -> Self {
        value
            .into_iter()
            .map(|(key, value)| (K::from_ref(key), V::from_ref(value)))
            .collect()
    }

    fn missing_ref() -> Option<Self::Ref> {
        Some(BTreeMap::new())
    }
}

impl<K, V> Element for BTreeMap<K, V> {}

fn decode_map<K: Ord, V, B: ByteSlice>(
    reader: &mut Reader<B>,
    decode_key: impl Fn(&mut Reader<B>) -> Result<K, Error>,
    decode_value: impl Fn(&mut Reader<B>) -> Result<V, Error>,
) -> Result<BTreeMap<K, V>, Error> {
    let len = reader.decode_varint::<u32>()? as usize;
    let mut entries = BTreeMap::new();
    for _ in 0..len {
        entries.insert(decode_key(reader)?, decode_value(reader)?);
    }
    Ok(entries)
}

impl Encode for str {
    #[inline]
    fn encoded_len(&self) -> usize {
        self.as_bytes().encoded_len()
    }

    fn encode(&self, out: &mut Writer<impl Output>) {
        self.as_bytes().encode(out);
    }
}

impl Encode for String {
    #[inline]
    fn encoded_len(&self) -> usize {
        self.as_str().encoded_len()
    }

    fn encode(&self, out: &mut Writer<impl Output>) {
        self.as_str().encode(out);
    }
}

impl<B: ByteSlice> Decode<B> for String {
    type Ref = Ref<B, str>;

    #[inline]
    fn decode_ref(reader: &mut Reader<B>) -> Result<Self::Ref, Error> {
        Ref::new(reader.take_len_prefixed()?)
    }

    #[inline]
    fn from_ref(value: Self::Ref) -> Self {
        value.to_string()
    }

    #[inline]
    fn missing_ref() -> Option<Self::Ref> {
        Ref::new(B::from_static(&[])?).ok()
    }
}

impl Encode for NonZeroU8 {
    #[inline]
    fn encoded_len(&self) -> usize {
        size_of::<u8>()
    }

    fn encode(&self, out: &mut Writer<impl Output>) {
        out.put_u8(self.get());
    }
}

impl DecodeValue for NonZeroU8 {
    #[inline]
    fn decode_value<B: ByteSlice>(reader: &mut Reader<B>) -> Result<Self, Error> {
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

    fn encode(&self, out: &mut Writer<impl Output>) {
        varint::encode(*self as u64, out);
    }
}

impl DecodeValue for usize {
    #[inline]
    fn decode_value<B: ByteSlice>(reader: &mut Reader<B>) -> Result<Self, Error> {
        Self::try_from(reader.decode_varint::<u64>()?).map_err(|_| Error::InvalidRange)
    }

    #[inline]
    fn missing() -> Option<Self> {
        Some(0)
    }
}

impl Encode for bool {
    #[inline]
    fn encoded_len(&self) -> usize {
        size_of::<u8>()
    }

    fn encode(&self, out: &mut Writer<impl Output>) {
        out.put_u8(u8::from(*self));
    }
}

impl DecodeValue for bool {
    #[inline]
    fn decode_value<B: ByteSlice>(reader: &mut Reader<B>) -> Result<Self, Error> {
        match reader.take_u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(Error::InvalidDiscriminant),
        }
    }

    #[inline]
    fn missing() -> Option<Self> {
        Some(false)
    }
}

impl Encode for () {
    #[inline]
    fn encoded_len(&self) -> usize {
        0
    }

    fn encode(&self, _out: &mut Writer<impl Output>) {}
}

impl DecodeValue for () {
    #[inline]
    fn decode_value<B: ByteSlice>(_reader: &mut Reader<B>) -> Result<Self, Error> {
        Ok(())
    }

    #[inline]
    fn missing() -> Option<Self> {
        Some(())
    }
}

impl<T: Encode> Encode for Option<T> {
    fn encoded_len(&self) -> usize {
        1 + self.as_ref().map_or(0, Encode::encoded_len)
    }

    fn encode(&self, out: &mut Writer<impl Output>) {
        match self {
            None => out.put_u8(0),
            Some(inner) => {
                out.put_u8(1);
                inner.encode(out);
            }
        }
    }
}

impl<B: ByteSlice, T: Decode<B>> Decode<B> for Option<T> {
    type Ref = Option<T::Ref>;

    fn decode(reader: &mut Reader<B>) -> Result<Self, Error> {
        decode_option(reader, T::decode)
    }

    fn decode_ref(reader: &mut Reader<B>) -> Result<Self::Ref, Error> {
        decode_option(reader, T::decode_ref)
    }

    fn from_ref(value: Self::Ref) -> Self {
        value.map(T::from_ref)
    }

    fn missing_ref() -> Option<Self::Ref> {
        Some(None)
    }
}

impl<T> Element for Option<T> {}

fn decode_option<T, B: ByteSlice>(
    reader: &mut Reader<B>,
    decode_value: impl Fn(&mut Reader<B>) -> Result<T, Error>,
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

    fn encode(&self, out: &mut Writer<impl Output>) {
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

impl<B: ByteSlice, T: Decode<B>, E: Decode<B>> Decode<B> for Result<T, E> {
    type Ref = Result<T::Ref, E::Ref>;

    fn decode(reader: &mut Reader<B>) -> Result<Self, Error> {
        decode_result(reader, T::decode, E::decode)
    }

    fn decode_ref(reader: &mut Reader<B>) -> Result<Self::Ref, Error> {
        decode_result(reader, T::decode_ref, E::decode_ref)
    }

    fn from_ref(value: Self::Ref) -> Self {
        value.map(T::from_ref).map_err(E::from_ref)
    }
}

impl<T, E> Element for Result<T, E> {}

fn decode_result<T, E, B: ByteSlice>(
    reader: &mut Reader<B>,
    decode_value: impl Fn(&mut Reader<B>) -> Result<T, Error>,
    decode_error: impl Fn(&mut Reader<B>) -> Result<E, Error>,
) -> Result<Result<T, E>, Error> {
    match reader.take_u8()? {
        0 => Ok(Ok(decode_value(reader)?)),
        1 => Ok(Err(decode_error(reader)?)),
        _ => Err(Error::InvalidDiscriminant),
    }
}

impl Encode for Duration {
    #[inline]
    fn encoded_len(&self) -> usize {
        size_of::<u64>() + size_of::<u32>()
    }

    fn encode(&self, out: &mut Writer<impl Output>) {
        self.as_secs().encode(out);
        self.subsec_nanos().encode(out);
    }
}

impl DecodeValue for Duration {
    #[inline]
    fn decode_value<B: ByteSlice>(reader: &mut Reader<B>) -> Result<Self, Error> {
        let secs: u64 = reader.decode()?;
        let nanos: u32 = reader.decode()?;
        if nanos >= 1_000_000_000 {
            return Err(Error::InvalidRange);
        }
        Ok(Self::new(secs, nanos))
    }

    #[inline]
    fn missing() -> Option<Self> {
        Some(Self::ZERO)
    }
}

/// Encodes as the [`Duration`] since the Unix epoch, which earlier times clamp to.
impl Encode for SystemTime {
    #[inline]
    fn encoded_len(&self) -> usize {
        Duration::ZERO.encoded_len()
    }

    fn encode(&self, out: &mut Writer<impl Output>) {
        self.duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .encode(out);
    }
}

impl DecodeValue for SystemTime {
    #[inline]
    fn decode_value<B: ByteSlice>(reader: &mut Reader<B>) -> Result<Self, Error> {
        UNIX_EPOCH
            .checked_add(reader.decode()?)
            .ok_or(Error::InvalidRange)
    }
}

/// Length prefixes are `u32`, so a 64-bit host cannot emit one a 32-bit peer would reject: a
/// `usize` varint is capped at ten bytes on one target and five on the other.
#[track_caller]
#[inline]
pub(crate) fn len_prefix(len: usize) -> u32 {
    u32::try_from(len).expect("length prefix above u32::MAX")
}

/// Encoded length of `len` bytes behind their length prefix
#[inline]
pub fn encoded_len_prefixed(len: usize) -> usize {
    varint::encoded_len(len_prefix(len)) + len
}

/// Writes the length prefix for `len` bytes
pub fn encode_len_prefix(len: usize, out: &mut Writer<impl Output>) {
    varint::encode(len_prefix(len), out);
}

pub fn encoded_len_bytes<B: BufView + ?Sized>(bytes: &B) -> usize {
    encoded_len_prefixed(bytes.buf().remaining())
}

pub fn encode_bytes<B: BufView + ?Sized>(bytes: &B, out: &mut Writer<impl Output>) {
    encode_len_prefix(bytes.buf().remaining(), out);
    encode_bytes_raw(bytes, out);
}

/// Writes the bytes without a length prefix, for a field that runs to the end of the message.
pub fn encode_bytes_raw<B: BufView + ?Sized>(bytes: &B, out: &mut Writer<impl Output>) {
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
pub fn decode_field<T, B>(fields: &mut Reader<B>) -> Result<T, Error>
where
    T: Decode<B>,
    B: ByteSlice,
{
    if fields.is_empty() {
        T::missing().ok_or(Error::UnexpectedEof)
    } else {
        T::decode(fields)
    }
}

/// Like [`decode_field`], but decodes the borrowed form
pub fn decode_field_ref<T, B>(fields: &mut Reader<B>) -> Result<T::Ref, Error>
where
    T: Decode<B>,
    B: ByteSlice,
{
    if fields.is_empty() {
        T::missing_ref().ok_or(Error::UnexpectedEof)
    } else {
        T::decode_ref(fields)
    }
}
