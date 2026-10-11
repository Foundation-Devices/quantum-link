use core::{borrow::Borrow, mem, ops::Deref};

use bytes::{Buf, Bytes, BytesMut};

use crate::{Decode, Error};

/// A byte slice owner used by the codec reader
///
/// Borrowed forms lend byte buffers from it, shared from `&[u8]` and `Bytes`, mutably from
/// `&mut [u8]` and `BytesMut`. Strings and fixed-size arrays are lent alike from slices, but
/// copied out of `Bytes` and `BytesMut`, which can't lend a `&str`.
pub trait ByteSlice: Deref<Target = [u8]> + AsRef<[u8]> + Sized {
    /// `T`'s borrowed form, lent from this slice
    ///
    /// The derive names field types through this, because `#[derive]` only bounds a field type
    /// that starts with a type parameter.
    type Of<T: Decode>;
    type Str: Deref<Target = str>;
    type Array<const N: usize>: Borrow<[u8; N]>;

    /// `Of<T>` is `T::Ref<Self>`, which code generic over the slice can't see, so this converts
    fn of<T: Decode>(value: T::Ref<Self>) -> Self::Of<T>;

    /// See [`ByteSlice::of`]
    fn ref_of<T: Decode>(value: Self::Of<T>) -> T::Ref<Self>;

    /// splits `self[..mid]` off the front, leaving `self[mid..]` behind
    ///
    /// # Panics
    ///
    /// Panics if `mid` exceeds the slice length.
    fn split_off_front(&mut self, mid: usize) -> Self;

    fn take_u8(&mut self) -> Option<u8>;

    fn into_str(self) -> Result<Self::Str, Error>;

    /// Should only be called on exactly `N` bytes
    fn into_array<const N: usize>(self) -> Self::Array<N>;

    fn empty() -> Self;

    fn empty_str() -> Self::Str;

    /// `N` zero bytes, unless they'd have to be lent mutably without a buffer to lend them from
    fn zeros<const N: usize>() -> Option<Self::Array<N>>;
}

impl<'a> ByteSlice for &'a [u8] {
    type Of<T: Decode> = T::Ref<Self>;
    type Str = &'a str;
    type Array<const N: usize> = &'a [u8; N];

    #[inline]
    fn of<T: Decode>(value: T::Ref<Self>) -> Self::Of<T> {
        value
    }

    #[inline]
    fn ref_of<T: Decode>(value: Self::Of<T>) -> T::Ref<Self> {
        value
    }

    #[inline]
    fn split_off_front(&mut self, mid: usize) -> Self {
        let (head, tail) = self.split_at(mid);
        *self = tail;
        head
    }

    #[inline]
    fn take_u8(&mut self) -> Option<u8> {
        let (&byte, remaining) = self.split_first()?;
        *self = remaining;
        Some(byte)
    }

    #[inline]
    fn into_str(self) -> Result<Self::Str, Error> {
        core::str::from_utf8(self).map_err(|_| Error::InvalidUtf8)
    }

    #[inline]
    fn into_array<const N: usize>(self) -> Self::Array<N> {
        self.try_into().expect("exactly N bytes")
    }

    #[inline]
    fn empty() -> Self {
        &[]
    }

    #[inline]
    fn empty_str() -> Self::Str {
        ""
    }

    #[inline]
    fn zeros<const N: usize>() -> Option<Self::Array<N>> {
        Some(&const { [0; N] })
    }
}

impl<'a> ByteSlice for &'a mut [u8] {
    type Of<T: Decode> = T::Ref<Self>;
    type Str = &'a mut str;
    type Array<const N: usize> = &'a mut [u8; N];

    #[inline]
    fn of<T: Decode>(value: T::Ref<Self>) -> Self::Of<T> {
        value
    }

    #[inline]
    fn ref_of<T: Decode>(value: Self::Of<T>) -> T::Ref<Self> {
        value
    }

    #[inline]
    fn split_off_front(&mut self, mid: usize) -> Self {
        let (head, tail) = mem::take(self).split_at_mut(mid);
        *self = tail;
        head
    }

    #[inline]
    fn take_u8(&mut self) -> Option<u8> {
        let (byte, remaining) = mem::take(self).split_first_mut()?;
        let byte = *byte;
        *self = remaining;
        Some(byte)
    }

    #[inline]
    fn into_str(self) -> Result<Self::Str, Error> {
        core::str::from_utf8_mut(self).map_err(|_| Error::InvalidUtf8)
    }

    #[inline]
    fn into_array<const N: usize>(self) -> Self::Array<N> {
        self.try_into().expect("exactly N bytes")
    }

    #[inline]
    fn empty() -> Self {
        Default::default()
    }

    #[inline]
    fn empty_str() -> Self::Str {
        Default::default()
    }

    #[inline]
    fn zeros<const N: usize>() -> Option<Self::Array<N>> {
        None
    }
}

/// Implements [`ByteSlice`] for a `bytes` buffer, which splits without copying
macro_rules! impl_bytes_buffer {
    ($($ty:ty),* $(,)?) => {
        $(
            impl ByteSlice for $ty {
                type Of<T: Decode> = T::Ref<Self>;
                type Str = String;
                type Array<const N: usize> = [u8; N];

                #[inline]
                fn of<T: Decode>(value: T::Ref<Self>) -> Self::Of<T> {
                    value
                }

                #[inline]
                fn ref_of<T: Decode>(value: Self::Of<T>) -> T::Ref<Self> {
                    value
                }

                #[inline]
                fn split_off_front(&mut self, mid: usize) -> Self {
                    self.split_to(mid)
                }

                #[inline]
                fn take_u8(&mut self) -> Option<u8> {
                    Buf::try_get_u8(self).ok()
                }

                #[inline]
                fn into_str(self) -> Result<Self::Str, Error> {
                    core::str::from_utf8(&self)
                        .map(str::to_owned)
                        .map_err(|_| Error::InvalidUtf8)
                }

                #[inline]
                fn into_array<const N: usize>(self) -> Self::Array<N> {
                    (*self).try_into().expect("exactly N bytes")
                }

                #[inline]
                fn empty() -> Self {
                    Self::new()
                }

                #[inline]
                fn empty_str() -> Self::Str {
                    String::new()
                }

                #[inline]
                fn zeros<const N: usize>() -> Option<Self::Array<N>> {
                    Some([0; N])
                }
            }
        )*
    };
}

impl_bytes_buffer!(Bytes, BytesMut);
