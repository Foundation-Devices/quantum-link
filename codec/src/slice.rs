use core::{mem, ops::Deref};

use bytes::{Buf, Bytes, BytesMut};

/// bytes that retain their container type when split
///
/// # Safety
///
/// dereferencing must return the same address and length until the container is split.
/// mutable dereferencing must refer to those same bytes. moving the container must preserve
/// the allocation and shared access must not modify its bytes.
pub unsafe trait ByteSlice: Deref<Target = [u8]> + Sized {
    /// splits off the first `mid` bytes and retains the rest
    ///
    /// panics if `mid` exceeds the slice length
    fn split_off_front(&mut self, mid: usize) -> Self;

    /// backing storage for a missing borrowed field, which a mutable container only has if empty
    fn from_static(bytes: &'static [u8]) -> Option<Self>;

    /// takes the first byte, if there is one
    fn take_u8(&mut self) -> Option<u8> {
        (!self.is_empty()).then(|| self.split_off_front(1)[0])
    }
}

// safety: slice references retain their allocation and mutable slices are exclusive
unsafe impl ByteSlice for &[u8] {
    #[inline]
    fn split_off_front(&mut self, mid: usize) -> Self {
        let (head, tail) = self.split_at(mid);
        *self = tail;
        head
    }

    fn from_static(bytes: &'static [u8]) -> Option<Self> {
        Some(bytes)
    }
}

// safety: slice references retain their allocation and mutable slices are exclusive
unsafe impl ByteSlice for &mut [u8] {
    #[inline]
    fn split_off_front(&mut self, mid: usize) -> Self {
        assert!(mid <= self.len());
        let (head, tail) = mem::take(self).split_at_mut(mid);
        *self = tail;
        head
    }

    fn from_static(bytes: &'static [u8]) -> Option<Self> {
        bytes.is_empty().then_some(&mut [])
    }
}

// safety: the buffer lives on the heap, so moving its handle keeps the address, and splitting
// leaves both halves' bytes in place. `Bytes` never modifies shared bytes, and `BytesMut` owns
// its range exclusively.
unsafe impl ByteSlice for Bytes {
    #[inline]
    fn split_off_front(&mut self, mid: usize) -> Self {
        self.split_to(mid)
    }

    fn from_static(bytes: &'static [u8]) -> Option<Self> {
        Some(Self::from_static(bytes))
    }

    // not through `split_off_front`, which would count a reference for every byte
    #[inline]
    fn take_u8(&mut self) -> Option<u8> {
        self.try_get_u8().ok()
    }
}

// safety: see `Bytes`
unsafe impl ByteSlice for BytesMut {
    #[inline]
    fn split_off_front(&mut self, mid: usize) -> Self {
        self.split_to(mid)
    }

    fn from_static(bytes: &'static [u8]) -> Option<Self> {
        bytes.is_empty().then(Self::new)
    }

    // see `Bytes`
    #[inline]
    fn take_u8(&mut self) -> Option<u8> {
        self.try_get_u8().ok()
    }
}
