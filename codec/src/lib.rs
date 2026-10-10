//! binary codec primitives shared by QuantumLink crates
//!
//! [`Encode`] and [`Decode`] define the wire encoding for protocol types
//! [`Decode::decode_bytes`] reads a value from the start of a byte slice, and [`Reader`] several
//! in a row. Their `_ref` forms borrow byte buffers and strings from the slice.
//!
//! `#[derive(Codec)]` (feature `derive`) implements both for structs and enums. By default
//! their encoding is backward and forward compatible: a struct, and the fields of an enum
//! variant, are preceded by their length. A decoder fills fields that a shorter encoding lacks
//! with [`Decode::missing`], and skips the ones it doesn't know. `#[codec(frozen)]` drops the
//! lengths for types that won't change. Enumerators are varints either way, so only those below
//! 128 take a single byte. Unless frozen, an enum decodes enumerators it doesn't know to its
//! `#[codec(unknown)]` variant, or fails without one. `#[codec(derive(...))]` derives traits on
//! the borrowed form, which encodes the same as the owned one. `#[codec(discriminants = Name)]`
//! on an enum also generates `Name`, its variants without their fields and numbered by
//! enumerator, and a `discriminant()` method returning it.
//!
//! Decoding recurses once per level of nesting, so a type decoded from untrusted input must not
//! be recursive, or a deep enough encoding overflows the stack.

mod buf_view;
mod codec;
mod error;
mod reader;
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

pub trait Decode: Sized {
    /// `Self` with byte buffers and strings borrowed from the encoding
    type Ref<'a>;

    fn decode_ref<'a>(reader: &mut Reader<'a>) -> Result<Self::Ref<'a>, Error>;

    fn from_ref(value: Self::Ref<'_>) -> Self;

    /// Decodes the owned form, by default through [`Decode::Ref`]
    ///
    /// A type whose borrowed form needs storage of its own, such as a list, should decode
    /// directly instead, or decoding it allocates twice.
    fn decode(reader: &mut Reader<'_>) -> Result<Self, Error> {
        Self::decode_ref(reader).map(Self::from_ref)
    }

    /// Value of a missing field, which an older encoder didn't write
    ///
    /// Standard library types use their `Default`, where they have one. A derived struct builds
    /// it from its fields' missing values, and a derived enum uses its unknown variant. `None`
    /// makes an encoding without the field fail to decode, so a field without a missing value
    /// can't be appended to an existing struct: it should be wrapped in an `Option`.
    fn missing<'a>() -> Option<Self::Ref<'a>> {
        None
    }

    /// Decodes `Self` from the start of `bytes`, ignoring whatever follows it
    fn decode_bytes(bytes: &[u8]) -> Result<Self, Error> {
        Self::decode(&mut Reader::new(bytes))
    }

    /// Like [`Decode::decode_bytes`], but borrows byte buffers and strings from `bytes`
    fn decode_bytes_ref(bytes: &[u8]) -> Result<Self::Ref<'_>, Error> {
        Self::decode_ref(&mut Reader::new(bytes))
    }
}

/// Helper for implementing [`Decode`] on a type that borrows nothing from the encoding
///
/// Such a type is its own borrowed form, so implementing this instead of [`Decode`] skips
/// spelling out `Ref = Self` and an identity `from_ref`. A blanket impl provides the rest.
///
/// This is only for implementers. Callers decode through [`Decode`] or [`Reader`], and generic
/// code bounds on [`Decode`], which every decodable type implements, whichever way it was
/// implemented.
pub trait DecodeValue: Sized {
    fn decode_value(reader: &mut Reader<'_>) -> Result<Self, Error>;

    /// See [`Decode::missing`]
    fn missing() -> Option<Self> {
        None
    }
}

impl<T: DecodeValue> Decode for T {
    type Ref<'a> = Self;

    fn decode_ref<'a>(reader: &mut Reader<'a>) -> Result<Self::Ref<'a>, Error> {
        <T as DecodeValue>::decode_value(reader)
    }

    fn from_ref(value: Self) -> Self {
        value
    }

    fn decode(reader: &mut Reader<'_>) -> Result<Self, Error> {
        <T as DecodeValue>::decode_value(reader)
    }

    fn missing<'a>() -> Option<Self::Ref<'a>> {
        <T as DecodeValue>::missing()
    }
}

/// Marks every encodable type except `u8`
///
/// A `Vec<u8>` borrows as `&[u8]`, because its encoded bytes already are the values. Any other
/// `Vec<T>` decodes item by item into a `Vec<T::Ref>`: the items are built while decoding, so
/// the list needs storage of its own, and only leaves such as `&[u8]` and `&str` borrow from
/// the encoding. Nested lists allocate at every level.
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

/// Implements the codec traits for a newtype over an integer, carried as a varint
#[macro_export]
macro_rules! varint_wrapper {
    ($name:ty, $inner:ty) => {
        impl $crate::Encode for $name {
            fn encoded_len(&self) -> usize {
                $crate::varint::encoded_len::<$inner>(self.0)
            }

            fn encode<W: $crate::BufMut + ?Sized>(&self, out: &mut W) {
                $crate::varint::encode::<$inner, W>(self.0, out);
            }
        }

        impl $crate::DecodeValue for $name {
            fn decode_value(reader: &mut $crate::Reader<'_>) -> Result<Self, $crate::Error> {
                Ok(Self(reader.decode_varint::<$inner>()?))
            }
        }

        impl $crate::Element for $name {}
    };
}
