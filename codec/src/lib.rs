//! binary codec primitives shared by QuantumLink crates
//!
//! [`Encode`] and [`Decode`] define the wire encoding for protocol types
//! [`Decode::decode_bytes`] reads a value from the start of a byte container, and [`Reader`]
//! several in a row. Their `_ref` forms borrow from the encoding instead.
//!
//! `#[derive(Codec)]` (feature `derive`) implements both for structs and enums, generic ones
//! included. By default their encoding is backward and forward compatible: a struct, and the
//! fields of an enum variant, are preceded by their length. A decoder fills fields that a shorter
//! encoding lacks with [`Decode::missing`], and skips the ones it doesn't know.
//! `#[codec(frozen)]` drops the lengths for types that won't change. Enumerators are varints
//! either way, so only those below 128 take a single byte. Unless frozen, an enum decodes
//! enumerators it doesn't know to its `#[codec(unknown)]` variant, or fails without one. The
//! derive generates `<Name>Ref<B>` as the borrowed form, which encodes the same as the owned one,
//! and `#[codec(derive(...))]` derives traits on it for non-generic types.
//! `#[codec(discriminants = Name)]` on an enum also generates `Name`, its variants without their
//! fields and numbered by enumerator, and a `discriminant()` method returning it.
//!
//! Borrowed forms decode from `&[u8]`, `&mut [u8]`, `Bytes` or `BytesMut`, without copying.
//! Strings, byte buffers, and vectors and arrays of fixed-width integers are lent as [`Ref`]s,
//! which dereference to the value, mutably to change the encoding in place if the container is
//! mutable. Other vectors and arrays decode their elements into `Vec<T::Ref>` and `[T::Ref; N]`.
//!
//! The integers are laid out like in memory: little-endian, starting a multiple of
//! `size_of::<T>()` from the start of the encoding, with `size_of::<T>() - 1` zeros around them.
//! [`Writer`] and [`Reader`] count from where they start, so an encoding must be decoded from its
//! start. To borrow the integers, it must also start aligned for `T` in memory, or borrowed
//! decoding fails with [`Error::InvalidAlignment`]. The owned form decodes either way.
//!
//! ```
//! # #[cfg(feature = "derive")]
//! # mod example {
//! use ql_codec::{Codec, Decode, Encode};
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
//! let shared: MessageRef<&[u8]> = Message::decode_bytes_ref(bytes.as_slice())?;
//! assert_eq!(shared.name, "hello");
//!
//! let mut mutable: MessageRef<&mut [u8]> = Message::decode_bytes_ref(bytes.as_mut_slice())?;
//! mutable.name.make_ascii_uppercase();
//! mutable.bytes[0] = 3;
//! # Ok(())
//! # }
//! # }
//! ```
//!
//! Decoding recurses once per level of nesting, so a type decoded from untrusted input must not
//! be recursive, or a deep enough encoding overflows the stack.

mod aligned;
mod buf_view;
mod codec;
mod error;
mod reader;
mod reference;
mod slice;
pub mod varint;
mod writer;

pub use buf_view::BufView;
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
pub use writer::{Output, Writer};

pub trait Encode {
    fn encoded_len(&self) -> usize;

    fn encode(&self, out: &mut Writer<impl Output>);

    fn encode_vec(&self) -> Vec<u8> {
        let len = self.encoded_len();
        let mut out = Writer::new(Vec::with_capacity(len));
        self.encode(&mut out);
        assert_eq!(out.position(), len);
        out.into_inner()
    }
}

/// owned and borrowed decoding from an input container
///
/// implement once over `B` to support shared and mutable input.
pub trait Decode<B: ByteSlice>: Sized {
    type Ref;

    /// decodes the owned form, through the borrowed form by default
    ///
    /// a type holding other values should decode them directly, or decoding allocates twice and
    /// misaligned integer lists fail to borrow.
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
    /// a mutable container can't lend a non-empty array, so an array appended to a struct should
    /// be an `Option`.
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

/// Marks the types whose lists and arrays decode item by item
///
/// That's every encodable type except the fixed-width integers, whose lists and arrays are lent
/// as a whole, as `Ref<B, [T]>`. The two kinds need separate `Vec<T>` and `[T; N]` impls, and
/// Rust only keeps those apart if the item-by-item ones require a marker the integers lack.
///
/// The derive and `varint_wrapper!` implement it. A hand-written codec impl needs an
/// `impl Element for T {}`, or `Vec<T>` and `[T; N]` fail with an unsatisfied `T: Element`.
pub trait Element {}

impl<T: Encode + ?Sized> Encode for &T {
    fn encoded_len(&self) -> usize {
        (**self).encoded_len()
    }

    fn encode(&self, out: &mut Writer<impl Output>) {
        (**self).encode(out);
    }
}

impl<T: ?Sized> Element for &T {}

impl<T: Encode + ?Sized> Encode for &mut T {
    fn encoded_len(&self) -> usize {
        (**self).encoded_len()
    }

    fn encode(&self, out: &mut Writer<impl Output>) {
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

            fn encode(&self, out: &mut $crate::Writer<impl $crate::Output>) {
                $crate::varint::encode::<$inner>(self.0, out);
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
