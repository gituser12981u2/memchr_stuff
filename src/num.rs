//! Stripped from rust/library/core/src/num for testing
//! https://github.com/rust-lang/rust/blob/main/library/core/src/num/mod.rs#L1298

/// Returns an `usize` where every byte is equal to `x`.
#[inline]
pub(crate) const fn repeat_u8(x: u8) -> usize {
    usize::from_ne_bytes([x; size_of::<usize>()])
}