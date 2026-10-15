use core::mem;

use bytes::BytesMut;

/// A destination for encoded bytes
pub trait Output {
    fn put_slice(&mut self, bytes: &[u8]);
}

impl Output for Vec<u8> {
    #[inline]
    fn put_slice(&mut self, bytes: &[u8]) {
        self.extend_from_slice(bytes);
    }
}

impl Output for BytesMut {
    #[inline]
    fn put_slice(&mut self, bytes: &[u8]) {
        self.extend_from_slice(bytes);
    }
}

/// Fills the slice from the front, leaving the rest unwritten
///
/// # Panics
///
/// Panics if the bytes don't fit.
impl Output for &mut [u8] {
    #[inline]
    fn put_slice(&mut self, bytes: &[u8]) {
        let (head, tail) = mem::take(self).split_at_mut(bytes.len());
        head.copy_from_slice(bytes);
        *self = tail;
    }
}

impl<O: Output> Output for &mut O {
    #[inline]
    fn put_slice(&mut self, bytes: &[u8]) {
        (**self).put_slice(bytes);
    }
}

/// Writes an encoding to an [`Output`], counting its bytes to align lists of integers
pub struct Writer<O> {
    out: O,
    position: usize,
}

impl<O: Output> Writer<O> {
    /// Starts an encoding, which a decoder must read from its start
    #[inline]
    pub fn new(out: O) -> Self {
        Self::with_position(out, 0)
    }

    /// Continues an encoding of which `position` bytes are already written
    #[inline]
    pub fn with_position(out: O, position: usize) -> Self {
        Self { out, position }
    }

    /// Bytes written since the encoding started
    #[inline]
    pub fn position(&self) -> usize {
        self.position
    }

    #[inline]
    pub fn put_slice(&mut self, bytes: &[u8]) {
        self.out.put_slice(bytes);
        self.position += bytes.len();
    }

    #[inline]
    pub fn put_u8(&mut self, byte: u8) {
        self.put_slice(&[byte]);
    }

    #[inline]
    pub fn put_zeros(&mut self, len: usize) {
        for _ in 0..len {
            self.put_u8(0);
        }
    }

    #[inline]
    pub fn into_inner(self) -> O {
        self.out
    }
}
