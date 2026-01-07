#![allow(clippy::host_endian_bytes)]
#![allow(clippy::multiple_unsafe_ops_per_block)]
#![allow(clippy::undocumented_unsafe_blocks)]
#![allow(clippy::empty_line_after_doc_comments)]
//#![allow(warnings)]

// I was reading through the std library for random silly things and I found this , https://doc.rust-lang.org/src/core/slice/memchr.rs.html#111-161
// this essentially provides a more rigorous foundation to my SWAR technique.
//the original definition is below the copy pasted code above.
//#![allow(clippy::all)]
//#![allow(warnings)] //the warnings are from memchr and thats a std lib func, too strict lints!
//I really prefer having some strong foundation to rely on, so I'll use it and say stuff it to pride. Make it easy for people to verify.

///copy pasting code here, will probably add something in the readme about it.
///
///I have not (yet, this comment maybe wrong)
/// I might do it, depends on use case.
// ive rewritten memchr to not rely on nightly too, so i can use without any deps

// Original implementation taken from rust-memchr.

// Copyright 2015 Andrew Gallant, bluss and Nicolas Koch

/// TODO: change to work with rust std.
use crate::num::repeat_u8;

use crate::{find_swar_index, find_swar_last_index};
use core::num::NonZeroUsize;

const LO_USIZE: usize = repeat_u8(0x01);
const HI_USIZE: usize = repeat_u8(0x80);
const INVERTED_HIGH: usize = !HI_USIZE;
const _: () = const { assert!(INVERTED_HIGH == repeat_u8(0x7F), "should be equal") };
const USIZE_BYTES: usize = size_of::<usize>();

/// Returns `true` if `x` contains any zero byte.
///
/// From *Matters Computational*, J. Arndt:
///
/// "The idea is to subtract one from each of the bytes and then look for
/// bytes where the borrow propagated all the way to the most significant
/// bit."
#[inline]
#[must_use]
// ! OPTIMIZATION !
pub const fn contains_zero_byte(x: usize) -> Option<NonZeroUsize> /* MINIMUM ADDRESSABLE SIZE =1 BYTE YAY*/
{
    NonZeroUsize::new(x.wrapping_sub(LO_USIZE) & !x & HI_USIZE)
}

#[inline]
const unsafe fn rposition_byte_len(base: *const u8, len: usize, needle: u8) -> Option<usize> {
    let mut i = len;
    while i != 0 {
        i -= 1;
        if unsafe { base.add(i).read() } == needle {
            return Some(i);
        }
    }
    None
}

#[inline]
#[must_use]
pub fn memchr(x: u8, text: &[u8]) -> Option<usize> {
    // Fast path for small slices.
    if text.len() < 2 * USIZE_BYTES {
        return memchr_naive(x, text);
    }

    memchr_aligned(x, text)
}

#[inline]
const fn memchr_naive(x: u8, text: &[u8]) -> Option<usize> {
    let mut i = 0;

    // FIXME(const-hack): Replace with `text.iter().pos(|c| *c == x)`.
    // rust elides the bounds check
    while i < text.len() {
        if text[i] == x {
            return Some(i);
        }

        i += 1;
    }

    None
}

#[inline]
fn memchr_aligned(x: u8, text: &[u8]) -> Option<usize> {
    // The runtime version behaves the same as the compile time version, it's
    // just more optimized.

    // Scan for a single byte value by reading two `usize` words at a time.
    //
    // Split `text` in three parts
    // - unaligned initial part, before the first word aligned address in text
    // - body, scan by 2 words at a time
    // - the last remaining part, < 2 word size

    // search up to an aligned boundary
    let len = text.len();
    let ptr = text.as_ptr();
    let mut offset = ptr.align_offset(USIZE_BYTES);

    if offset > 0 {
        offset = offset.min(len);
        let slice = unsafe { &text.get_unchecked(..offset) };
        if let Some(index) = memchr_naive(x, slice) {
            return Some(index);
        }
    }

    // search the body of the text
    let repeated_x = repeat_u8(x);
    while offset <= len - 2 * USIZE_BYTES {
        // SAFETY: the while's predicate guarantees a distance of at least 2 * usize_bytes
        // between the offset and the end of the slice.
        unsafe {
            let u = ptr.add(offset).cast::<usize>().read();
            let v = ptr.add(offset + USIZE_BYTES).cast::<usize>().read();

            // break if there is a matching byte
            // ! OPTIMIZATION !
            // check this branch first (lower has precedence, obvs)
            if let Some(lower) = contains_zero_byte(u ^ repeated_x) {
                return Some(offset + find_swar_index!(lower));
            }
            if let Some(upper) = contains_zero_byte(v ^ repeated_x) {
                return Some(offset + USIZE_BYTES + find_swar_index!(upper));
            }
        }

        offset += USIZE_BYTES * 2;
    }

    // Find the byte after the point the body loop stopped.

    let slice =
            // SAFETY: offset is within bounds
                unsafe { core::slice::from_raw_parts(text.as_ptr().add(offset), text.len() - offset) };

    memchr_naive(x, slice).map(|i| offset + i)
}

#[inline]
#[must_use]
// TODO TIDY UP SAFETY SHIT
const unsafe fn find_zero_byte_reversed(x: usize) -> usize {
    let y = (x & INVERTED_HIGH).wrapping_add(INVERTED_HIGH);
    let ans = unsafe { NonZeroUsize::new_unchecked(!(y | x | INVERTED_HIGH)) };
    find_swar_last_index!(ans)
}

/// Returns the last index matching the byte `x` in `text`.
///
/// This is directly copy pasted from the internal library with some modifications to make it work for me
/// there were no unstable features so I thought I'll skip a dependency and add this.
///
#[must_use]
#[inline]
pub fn memrchr(x: u8, text: &[u8]) -> Option<usize> {
    // Scan for a single byte value by reading two `usize` words at a time.
    //
    // Split `text` in three parts:
    // - unaligned tail, after the last word aligned address in text,
    // - body, scanned by 2 words at a time,
    // - the first remaining bytes, < 2 word size.
    let len = text.len();
    let ptr = text.as_ptr();

    let (min_aligned_offset, max_aligned_offset) = {
        // We call this just to obtain the length of the prefix and suffix.
        // In the middle we always process two chunks at once.
        // SAFETY: transmuting `[u8]` to `[usize]` is safe except for size differences
        // which are handled by `align_to`.
        let (prefix, _, suffix) = unsafe { text.align_to::<(usize, usize)>() };
        (prefix.len(), len - suffix.len())
    };

    let mut offset = max_aligned_offset;

    let start = text.as_ptr();
    let tail_len = len - offset; // tail is [offset, len)
    if let Some(i) = unsafe { rposition_byte_len(start.add(offset), tail_len, x) } {
        return Some(offset + i);
    }

    let repeated_x = repeat_u8(x);

    while offset > min_aligned_offset {
        // SAFETY: offset starts at len - suffix.len(), as long as it is greater than
        // min_aligned_offset (prefix.len()) the remaining distance is at least 2 * chunk_bytes.

        let u = unsafe { ptr.add(offset - 2 * USIZE_BYTES).cast::<usize>().read() };

        let v = unsafe { ptr.add(offset - USIZE_BYTES).cast::<usize>().read() };

        // Break if there is a matching byte.
        // **CHECK UPPER FIRST**
        let xorred_upper = v ^ repeated_x;
        // use the original SWAR (fewer instructions) to check for zero byte
        if contains_zero_byte(xorred_upper).is_some() {
            // Then apply alternative SWAR (guaranteed to be nonzero)
            // We need to use an alternative SWAR method because HASZERO propagates 0xFF right(or left, depending on endianness) wise after match
            // this could be done with a byte swap but thats 1 (or more, depending on arch) instructions
            // use this only when a match is FOUND
            let zero_byte_pos = unsafe { find_zero_byte_reversed(xorred_upper) };
            return Some(offset - USIZE_BYTES + zero_byte_pos);
        }

        let xorred_lower = u ^ repeated_x;
        if contains_zero_byte(xorred_lower).is_some() {
            // TODO, TIDY UP SAFETY? use nonzerousize etc.
            // same stuff as above otherwise
            let zero_byte_pos = unsafe { find_zero_byte_reversed(xorred_lower) };

            return Some(offset - 2 * USIZE_BYTES + zero_byte_pos);
        }

        offset -= 2 * USIZE_BYTES;
    }

    unsafe { rposition_byte_len(start, offset, x) }
}
