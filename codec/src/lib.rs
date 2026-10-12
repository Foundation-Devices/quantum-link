//! binary codec primitives shared by QuantumLink crates
//!
//! [`Encode`] and [`Decode`] define the wire encoding for protocol types
//! [`Decode::decode_bytes`] reads a value from the start of a byte slice, and [`Reader`] several
//! in a row. their `_ref` forms borrow data from the encoding.
//!
//! `#[derive(Codec)]` (feature `derive`) implements both for structs and enums, generic ones
//! included. by default
//! their encoding is backward and forward compatible: a struct, and the fields of an enum
//! variant, are preceded by their length. A decoder fills fields that a shorter encoding lacks
//! with [`Decode::missing`], and skips the ones it doesn't know. `#[codec(frozen)]` drops the
//! lengths for types that won't change. Enumerators are varints either way, so only those below
//! 128 take a single byte. Unless frozen, an enum decodes enumerators it doesn't know to its
//! `#[codec(unknown)]` variant, or fails without one. `#[codec(derive(...))]` derives traits on
//! the borrowed form of a non-generic type, which encodes the same as the owned one.
//! traits such as `Clone` and `Copy` apply when the input container supports them.
//! `#[codec(discriminants = Name)]`
//! on an enum also generates `Name`, its variants without their fields and numbered by
//! enumerator, and a `discriminant()` method returning it.
//!
//! byte vectors and arrays borrow their bytes as [`Ref`]s, like strings. other vectors and
//! arrays decode their elements into `Vec<T::Ref>` and `[T::Ref; N]`.
//!
//! ```
//! # #[cfg(feature = "derive")]
//! # mod example {
//! use ql_codec::{Codec, Encode, Reader};
//!
//! #[derive(Codec)]
//! #[codec(frozen)]
//! struct Message {
//!     name: String,
//!     bytes: Vec<u8>,
//! }
//!
//! # fn example() -> Result<(), ql_codec::Error> {
//! let message = Message { name: "hello".into(), bytes: vec![1, 2] };
//! let mut bytes = message.encode_vec();
//! let shared: MessageRef<&[u8]> = Reader::new(bytes.as_slice()).decode_ref::<Message>()?;
//! assert_eq!(shared.name, "hello");
//!
//! let mut mutable: MessageRef<&mut [u8]> = Reader::new(bytes.as_mut_slice()).decode_ref::<Message>()?;
//! mutable.name.make_ascii_uppercase();
//! mutable.bytes[0] = 3;
//! # Ok(())
//! # }
//! # }
//! ```
//!
//! [`Reader`] accepts `&[u8]`, `&mut [u8]`, `Bytes` or `BytesMut`. its `take_n`, `take_array`,
//! `take_all`, and `take_len_prefixed` methods preserve that container type, so a mutable
//! reader lends disjoint mutable slices, and `Bytes` shares its buffer, without copying. owned
//! `decode` works with any input.
//! borrowed `decode_ref` selects shared or mutable views from the input. the derive generates
//! one `MessageRef<B>` for a type named `Message`, recursively preserving
//! mutability for every borrowed field, including strings and byte arrays.
//! scalars and storage for decoded collections remain owned. typed fields implement `Deref`,
//! and implement `DerefMut` when the input container does. input may be borrowed or owned.
//! `decode_bytes` and `decode_bytes_ref` also accept any of them. pass `as_slice()` or
//! `as_mut_slice()` when starting with a vector or array.
//!
//! Decoding recurses once per level of nesting, so a type decoded from untrusted input must not
//! be recursive, or a deep enough encoding overflows the stack.

mod buf_view;
mod codec;
mod error;
mod reader;
mod reference;
mod slice;
pub mod varint;

pub use buf_view::BufView;
pub use bytes::BufMut;
pub use codec::{
    decode_field, decode_field_ref, encode_bytes, encode_bytes_raw, encode_len_prefix,
    encoded_len_bytes, encoded_len_prefixed,
};
pub use error::Error;
#[cfg(feature = "derive")]
pub use ql_codec_macro::Codec;
pub use reader::Reader;
pub use reference::Ref;
pub use slice::ByteSlice;
pub use varint::Varint;

pub trait Encode {
    fn encoded_len(&self) -> usize;

    fn encode<W: bytes::BufMut + ?Sized>(&self, out: &mut W);

    fn encode_vec(&self) -> Vec<u8> {
        let len = self.encoded_len();
        let mut out = Vec::with_capacity(len);
        self.encode(&mut out);
        assert_eq!(out.len(), len);
        out
    }
}

/// owned and borrowed decoding from an input container
///
/// implement once over `B` to support shared and mutable input.
pub trait Decode<B: ByteSlice>: Sized {
    type Ref;

    /// decodes the owned form through the borrowed form by default
    ///
    /// collections should decode directly to avoid allocating storage twice.
    fn decode(reader: &mut Reader<B>) -> Result<Self, Error> {
        Self::decode_ref(reader).map(Self::from_ref)
    }

    fn decode_ref(reader: &mut Reader<B>) -> Result<Self::Ref, Error>;
    fn from_ref(value: Self::Ref) -> Self;

    /// decodes from the start of `bytes`, ignoring whatever follows
    fn decode_bytes(bytes: B) -> Result<Self, Error> {
        Self::decode(&mut Reader::new(bytes))
    }

    /// decodes the borrowed form, preserving the input container's mutability
    fn decode_bytes_ref(bytes: B) -> Result<Self::Ref, Error> {
        Self::decode_ref(&mut Reader::new(bytes))
    }

    /// the value of a field omitted by an older encoder
    fn missing() -> Option<Self> {
        Self::missing_ref().map(Self::from_ref)
    }

    /// borrowed default for a field omitted by an older encoder
    ///
    /// mutable input cannot borrow nonempty static defaults. wrap borrowed fixed arrays
    /// in `Option` to allow older encodings to omit them.
    fn missing_ref() -> Option<Self::Ref> {
        None
    }
}

/// implements owned and borrowed decoding for values that borrow nothing
pub trait DecodeValue: Sized {
    fn decode_value<B: ByteSlice>(reader: &mut Reader<B>) -> Result<Self, Error>;

    /// see [`Decode::missing`]
    fn missing() -> Option<Self> {
        None
    }
}

impl<B: ByteSlice, T: DecodeValue> Decode<B> for T {
    type Ref = Self;

    fn decode_ref(reader: &mut Reader<B>) -> Result<Self, Error> {
        <T as DecodeValue>::decode_value(reader)
    }

    fn from_ref(value: Self) -> Self {
        value
    }

    fn missing_ref() -> Option<Self> {
        <T as DecodeValue>::missing()
    }
}

/// Marks every encodable type except `u8`
///
/// A `Vec<u8>` borrows as a `Ref<B, [u8]>`, because its encoded bytes already are the values.
/// Any other `Vec<T>` decodes item by item into a `Vec<T::Ref>`: the items are built while
/// decoding, so the list needs storage of its own, and only leaves such as byte buffers and
/// strings borrow from the encoding. Nested lists allocate at every level.
///
/// These are two separate impls for `Vec`, and Rust can't express the `T != u8` that would keep
/// them apart, so every other type opts in here instead. The derive and `varint_wrapper!` do
/// that, hand-written impls need an `impl Element for T {}`. Without it, `Vec<T>` and `[T; N]`
/// don't implement [`Encode`] and [`Decode`], which shows up as an unsatisfied `T: Element`.
pub trait Element {}

impl<T: Encode + ?Sized> Encode for &T {
    fn encoded_len(&self) -> usize {
        (**self).encoded_len()
    }

    fn encode<W: bytes::BufMut + ?Sized>(&self, out: &mut W) {
        (**self).encode(out);
    }
}

impl<T: ?Sized> Element for &T {}

impl<T: Encode + ?Sized> Encode for &mut T {
    fn encoded_len(&self) -> usize {
        (**self).encoded_len()
    }

    fn encode<W: bytes::BufMut + ?Sized>(&self, out: &mut W) {
        (**self).encode(out);
    }
}

impl<T: ?Sized> Element for &mut T {}

/// Implements the codec traits for a newtype over an integer, carried as a varint
#[macro_export]
macro_rules! varint_wrapper {
    ($name:ty, $inner:ty) => {
        impl $crate::Encode for $name {
            #[inline]
            fn encoded_len(&self) -> usize {
                $crate::varint::encoded_len::<$inner>(self.0)
            }

            fn encode<W: $crate::BufMut + ?Sized>(&self, out: &mut W) {
                $crate::varint::encode::<$inner, W>(self.0, out);
            }
        }

        impl $crate::DecodeValue for $name {
            #[inline]
            fn decode_value<B: $crate::ByteSlice>(
                reader: &mut $crate::Reader<B>,
            ) -> Result<Self, $crate::Error> {
                Ok(Self(reader.decode_varint::<$inner>()?))
            }
        }

        impl $crate::Element for $name {}
    };
}
