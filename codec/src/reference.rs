use core::{
    borrow::{Borrow, BorrowMut},
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use bytemuck::{Pod, PodCastError};

use crate::{BufView, ByteSlice, Element, Encode, Error, Output, Writer};

/// a validated typed reference with the mutability of its byte container
///
/// supports strings and fixed width integer slices and arrays. mutable access preserves
/// the length and validity of the encoding. multibyte integers require little endian targets.
///
/// shared storage cannot be mutated through the typed reference
///
/// ```compile_fail
/// use ql_codec::Ref;
///
/// let mut text = Ref::<_, str>::new(&b"hello"[..]).unwrap();
/// text.make_ascii_uppercase();
/// ```
pub struct Ref<B, T: ?Sized> {
    bytes: B,
    target: PhantomData<T>,
}

impl<B: ByteSlice, T: Target + ?Sized> Ref<B, T> {
    pub fn new(bytes: B) -> Result<Self, Error> {
        T::validate(&bytes)?;
        Ok(Self {
            bytes,
            target: PhantomData,
        })
    }
}

impl<B, T: ?Sized> Ref<B, T> {
    pub fn into_bytes(self) -> B {
        self.bytes
    }
}

impl<B: ByteSlice, T: Target + ?Sized> Deref for Ref<B, T> {
    type Target = T;

    fn deref(&self) -> &T {
        // safety: construction validated these stable bytes and typed mutations preserve validity
        unsafe { T::from_bytes(&self.bytes) }
    }
}

impl<B: ByteSlice + DerefMut, T: Target + ?Sized> DerefMut for Ref<B, T> {
    fn deref_mut(&mut self) -> &mut T {
        // safety: mutable dereferencing provides the same validated bytes exclusively
        unsafe { T::from_bytes_mut(&mut self.bytes) }
    }
}

impl<B: ByteSlice + Clone, T: Target + ?Sized> Clone for Ref<B, T> {
    fn clone(&self) -> Self {
        Self::new(self.bytes.clone()).expect("cloned bytes retain a valid layout")
    }
}

impl<B: ByteSlice + Copy, T: Target + ?Sized> Copy for Ref<B, T> {}

impl<B: ByteSlice, T: Target + ?Sized> AsRef<T> for Ref<B, T> {
    fn as_ref(&self) -> &T {
        self
    }
}

impl<B: ByteSlice, T, const N: usize> AsRef<[T]> for Ref<B, [T; N]>
where
    [T; N]: Target,
{
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<B: ByteSlice + DerefMut, T: Target + ?Sized> AsMut<T> for Ref<B, T> {
    fn as_mut(&mut self) -> &mut T {
        self
    }
}

impl<B: ByteSlice + DerefMut, T, const N: usize> AsMut<[T]> for Ref<B, [T; N]>
where
    [T; N]: Target,
{
    fn as_mut(&mut self) -> &mut [T] {
        self.as_mut_slice()
    }
}

impl<B: ByteSlice, T: Target + ?Sized> Borrow<T> for Ref<B, T> {
    fn borrow(&self) -> &T {
        self
    }
}

impl<B: ByteSlice + DerefMut, T: Target + ?Sized> BorrowMut<T> for Ref<B, T> {
    fn borrow_mut(&mut self) -> &mut T {
        self
    }
}

impl<B: ByteSlice, T: Target + Encode + ?Sized> Encode for Ref<B, T> {
    fn encoded_len(&self) -> usize {
        (**self).encoded_len()
    }

    fn encode(&self, out: &mut Writer<impl Output>) {
        (**self).encode(out);
    }
}

impl<B, T: ?Sized> Element for Ref<B, T> {}

impl<B: ByteSlice, T: Target + BufView + ?Sized> BufView for Ref<B, T> {
    type Buf<'a>
        = T::Buf<'a>
    where
        Self: 'a;

    fn buf(&self) -> Self::Buf<'_> {
        (**self).buf()
    }
}

impl<B: ByteSlice, T: Target + fmt::Debug + ?Sized> fmt::Debug for Ref<B, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<B: ByteSlice, T: Target + fmt::Display + ?Sized> fmt::Display for Ref<B, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        (**self).fmt(f)
    }
}

impl<B: ByteSlice, C: ByteSlice, T: Target + PartialEq + ?Sized> PartialEq<Ref<C, T>>
    for Ref<B, T>
{
    fn eq(&self, other: &Ref<C, T>) -> bool {
        **self == **other
    }
}

impl<B: ByteSlice, T: Target + Eq + ?Sized> Eq for Ref<B, T> {}

impl<B: ByteSlice, T: Target + PartialEq + ?Sized> PartialEq<&T> for Ref<B, T> {
    fn eq(&self, other: &&T) -> bool {
        **self == **other
    }
}

impl<B: ByteSlice, T: Target + PartialOrd + ?Sized> PartialOrd for Ref<B, T> {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        (**self).partial_cmp(&**other)
    }
}

impl<B: ByteSlice, T: Target + Ord + ?Sized> Ord for Ref<B, T> {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        (**self).cmp(&**other)
    }
}

impl<B: ByteSlice, T: Target + Hash + ?Sized> Hash for Ref<B, T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        (**self).hash(state);
    }
}

/// layouts whose validated bytes remain valid after typed mutation
///
/// # Safety
///
/// successful validation must make both casts sound. typed writes must leave every byte
/// initialized and preserve the validity of later casts.
pub unsafe trait Target {
    fn validate(bytes: &[u8]) -> Result<(), Error>;
    unsafe fn from_bytes(bytes: &[u8]) -> &Self;
    unsafe fn from_bytes_mut(bytes: &mut [u8]) -> &mut Self;
}

// safety: safe string operations preserve utf8 and initialize every byte
unsafe impl Target for str {
    fn validate(bytes: &[u8]) -> Result<(), Error> {
        std::str::from_utf8(bytes)
            .map(|_| ())
            .map_err(|_| Error::InvalidUtf8)
    }

    unsafe fn from_bytes(bytes: &[u8]) -> &Self {
        // safety: the caller provides validated utf8
        unsafe { std::str::from_utf8_unchecked(bytes) }
    }

    unsafe fn from_bytes_mut(bytes: &mut [u8]) -> &mut Self {
        // safety: the caller provides exclusive validated utf8
        unsafe { std::str::from_utf8_unchecked_mut(bytes) }
    }
}

// safety: bytemuck checks the size and alignment of every cast
unsafe impl<T: Pod> Target for [T] {
    fn validate(bytes: &[u8]) -> Result<(), Error> {
        if bytes.is_empty() {
            return Ok(());
        }
        bytemuck::try_cast_slice::<u8, T>(bytes)
            .map(drop)
            .map_err(|error| match error {
                PodCastError::TargetAlignmentGreaterAndInputNotAligned => Error::InvalidAlignment,
                _ => Error::InvalidData,
            })
    }

    unsafe fn from_bytes(bytes: &[u8]) -> &Self {
        cast(bytes)
    }

    unsafe fn from_bytes_mut(bytes: &mut [u8]) -> &mut Self {
        cast_mut(bytes)
    }
}

// safety: see `[T]`, and validation additionally checks the fixed array length
unsafe impl<T: Pod, const N: usize> Target for [T; N] {
    fn validate(bytes: &[u8]) -> Result<(), Error> {
        if bytes.len() != size_of::<Self>() {
            return Err(Error::InvalidData);
        }
        <[T] as Target>::validate(bytes)
    }

    unsafe fn from_bytes(bytes: &[u8]) -> &Self {
        cast(bytes).try_into().expect("validated length")
    }

    unsafe fn from_bytes_mut(bytes: &mut [u8]) -> &mut Self {
        cast_mut(bytes).try_into().expect("validated length")
    }
}

/// `bytes` as `T`s, which they must hold exactly and aligned
fn cast<T: Pod>(bytes: &[u8]) -> &[T] {
    // an empty slice may sit at any address, but there's nothing to cast
    if bytes.is_empty() {
        return &[];
    }
    bytemuck::cast_slice(bytes)
}

/// see [`cast`]
fn cast_mut<T: Pod>(bytes: &mut [u8]) -> &mut [T] {
    if bytes.is_empty() {
        return &mut [];
    }
    bytemuck::cast_slice_mut(bytes)
}
