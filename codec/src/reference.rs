use core::{
    borrow::{Borrow, BorrowMut},
    fmt,
    hash::{Hash, Hasher},
    marker::PhantomData,
    ops::{Deref, DerefMut},
};

use crate::{BufView, ByteSlice, Element, Encode, Error, Output, Writer};

/// a validated typed reference with the mutability of its byte container
///
/// supports strings and other layouts implementing Target. mutable access preserves
/// the length and validity of the encoding.
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
/// the type must have no padding and shared access must not modify its bytes. casts must
/// borrow the input bytes in the type's native layout. validation must check size, alignment
/// and contents so both casts are sound. typed writes must initialize every byte and preserve
/// validity. the flag must only be true if every bit pattern is valid.
/// callers of the casts must provide validated bytes. mutable casts additionally require
/// exclusive access.
pub unsafe trait Target {
    /// allows containers to skip element validation after checking layout
    const ANY_BIT_PATTERN: bool = false;

    /// checks the native size, alignment and contents
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

// safety: validation checks the layout and each element's contents when required
unsafe impl<T: Target> Target for [T] {
    const ANY_BIT_PATTERN: bool = T::ANY_BIT_PATTERN;

    fn validate(bytes: &[u8]) -> Result<(), Error> {
        if bytes.is_empty() {
            return Ok(());
        }
        if size_of::<T>() == 0 {
            return Err(Error::InvalidData);
        }
        if !bytes.as_ptr().cast::<T>().is_aligned() {
            return Err(Error::InvalidAlignment);
        }
        if bytes.len() % size_of::<T>() != 0 {
            return Err(Error::InvalidData);
        }
        if !T::ANY_BIT_PATTERN {
            for bytes in bytes.chunks_exact(size_of::<T>()) {
                T::validate(bytes)?;
            }
        }
        Ok(())
    }

    unsafe fn from_bytes(bytes: &[u8]) -> &Self {
        if bytes.is_empty() {
            return &[];
        }
        // safety: the caller validated length and alignment and each element's contents
        unsafe { core::slice::from_raw_parts(bytes.as_ptr().cast(), bytes.len() / size_of::<T>()) }
    }

    unsafe fn from_bytes_mut(bytes: &mut [u8]) -> &mut Self {
        if bytes.is_empty() {
            return &mut [];
        }
        // safety: the caller provides exclusive bytes with validated length and alignment
        unsafe {
            core::slice::from_raw_parts_mut(bytes.as_mut_ptr().cast(), bytes.len() / size_of::<T>())
        }
    }
}

// safety: see `[T]`, and validation additionally checks the fixed array length
unsafe impl<T: Target, const N: usize> Target for [T; N] {
    const ANY_BIT_PATTERN: bool = N == 0 || T::ANY_BIT_PATTERN;

    fn validate(bytes: &[u8]) -> Result<(), Error> {
        if bytes.len() != size_of::<Self>() {
            return Err(Error::InvalidData);
        }
        if N != 0 && size_of::<T>() == 0 && !T::ANY_BIT_PATTERN {
            return T::validate(bytes);
        }
        <[T] as Target>::validate(bytes)
    }

    unsafe fn from_bytes(bytes: &[u8]) -> &Self {
        let ptr = if bytes.is_empty() {
            core::ptr::NonNull::<Self>::dangling().as_ptr()
        } else {
            bytes.as_ptr().cast::<Self>()
        };
        // safety: validation ensures an aligned array layout and empty bytes mean zero size
        unsafe { &*ptr }
    }

    unsafe fn from_bytes_mut(bytes: &mut [u8]) -> &mut Self {
        let ptr = if bytes.is_empty() {
            core::ptr::NonNull::<Self>::dangling().as_ptr()
        } else {
            bytes.as_mut_ptr().cast::<Self>()
        };
        // safety: the array layout is validated and exclusively borrowed or has zero size
        unsafe { &mut *ptr }
    }
}

macro_rules! impl_target {
    ($($ty:ty),* $(,)?) => {
        $(
            // safety: these primitive layouts have no padding and accept every bit pattern
            unsafe impl Target for $ty {
                const ANY_BIT_PATTERN: bool = true;

                fn validate(bytes: &[u8]) -> Result<(), Error> {
                    <[Self; 1] as Target>::validate(bytes)
                }

                unsafe fn from_bytes(bytes: &[u8]) -> &Self {
                    // safety: the caller provides the validated primitive layout
                    unsafe { &*bytes.as_ptr().cast() }
                }

                unsafe fn from_bytes_mut(bytes: &mut [u8]) -> &mut Self {
                    // safety: the caller provides the validated primitive layout exclusively
                    unsafe { &mut *bytes.as_mut_ptr().cast() }
                }
            }
        )*
    };
}

impl_target!(
    u8,
    u16,
    u32,
    u64,
    u128,
    usize,
    i8,
    i16,
    i32,
    i64,
    i128,
    isize,
    f32,
    f64,
    ()
);
