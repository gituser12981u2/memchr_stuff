#![allow(warnings)]
//! Stripped from rust/library/core/src/num for testing
//! "https://github.com/rust-lang/rust/blob/main/library/core/src/num/mod.rs#L1298"

/// Returns an `usize` where every byte is equal to `x`.
#[inline]
pub const fn repeat_u8(x: u8) -> usize {
    usize::from_ne_bytes([x; size_of::<usize>()])
}

/// Returns an `usize` where every byte pair is equal to `x`.
#[inline]
#[allow(unused)]
pub(crate) const fn repeat_u16(x: u16) -> usize {
    let mut r = 0usize;
    let mut i = 0;
    while i < size_of::<usize>() {
        // Use `wrapping_shl` to make it work on targets with 16-bit `usize`
        r = r.wrapping_shl(16) | (x as usize);
        i += 2;
    }
    r
}
