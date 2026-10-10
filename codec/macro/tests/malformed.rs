use std::{
    alloc::{GlobalAlloc, Layout, System},
    num::NonZeroU8,
    time::{Duration, SystemTime},
};

use ql_codec::{Codec, Decode, Error};

#[derive(Debug, PartialEq, Eq, Codec)]
pub enum Command {
    Reboot,
    SetBrightness(u8),
}

/// Refuses allocations over 64 MiB, so reserving memory for a corrupt length aborts the test
/// instead of quietly succeeding under overcommit
struct CappedAllocator;

// SAFETY: There's no safe way to limit allocations. This delegates to `System` and only ever
// refuses one, which `GlobalAlloc` permits.
unsafe impl GlobalAlloc for CappedAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        if layout.size() > 64 << 20 {
            return std::ptr::null_mut();
        }
        // SAFETY: The caller upholds `alloc`'s contract, which is the same for `System`.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: `ptr` came from `System.alloc` with this `layout`.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CappedAllocator = CappedAllocator;

#[test]
fn corrupt_lengths() {
    let huge_length = [0xff, 0xff, 0xff, 0xff, 0x0f];
    let input = [huge_length.as_slice(), &[0; 1 << 20]].concat();
    // Reserving room for `u32::MAX` items, or for an item per byte left, would hit the
    // allocator's cap.
    assert_eq!(
        Vec::<[u8; 1024]>::decode_bytes(&input),
        Err(Error::UnexpectedEof)
    );
    assert_eq!(Vec::<u8>::decode_bytes(&input), Err(Error::UnexpectedEof));
}

#[test]
fn invalid_values() {
    assert_eq!(
        Command::decode_bytes(&[2, 1, 50]),
        Err(Error::InvalidDiscriminant)
    );
    assert_eq!(bool::decode_bytes(&[2]), Err(Error::InvalidDiscriminant));
    assert_eq!(
        Option::<u8>::decode_bytes(&[2, 0]),
        Err(Error::InvalidDiscriminant)
    );
    assert_eq!(
        Result::<u8, u8>::decode_bytes(&[2, 0]),
        Err(Error::InvalidDiscriminant)
    );
    assert_eq!(NonZeroU8::decode_bytes(&[0]), Err(Error::InvalidData));
    assert_eq!(
        String::decode_bytes(&[2, 0xc3, 0x28]),
        Err(Error::InvalidUtf8)
    );
    assert_eq!(
        Duration::decode_bytes(&[[0; 8].as_slice(), &1_000_000_000u32.to_le_bytes()].concat()),
        Err(Error::InvalidRange)
    );
    assert_eq!(
        SystemTime::decode_bytes(&[u64::MAX.to_le_bytes().as_slice(), &[0; 4]].concat()),
        Err(Error::InvalidRange)
    );
}
