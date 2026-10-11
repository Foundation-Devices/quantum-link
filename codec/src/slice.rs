use core::{mem, ops::Deref};

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

    fn empty() -> Self;

    /// backing storage for missing borrowed fields
    fn from_static(bytes: &'static [u8]) -> Option<Self> {
        bytes.is_empty().then(Self::empty)
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

    fn empty() -> Self {
        &[]
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

    fn empty() -> Self {
        &mut []
    }
}
