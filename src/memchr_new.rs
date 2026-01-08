#![allow(clippy::host_endian_bytes)]
#![allow(clippy::multiple_unsafe_ops_per_block)]
#![allow(clippy::undocumented_unsafe_blocks)]
#![allow(clippy::empty_line_after_doc_comments)]

// TODO? test on 32bit targets! (ok fuck 16 i dont think  rust even supports 16)
// Original implementation taken from rust-memchr.

/// TODO: change to work with rust std.
use crate::num::repeat_u8;

// USE THIS TO ENABLE BETTER INTRINSICS AKA CTLZ_NONZERO/CTTZ_NONZERO
use core::num::NonZeroUsize;

#[inline]
const fn do_fast_swar(x: usize) -> usize {
    x.wrapping_sub(LO_USIZE) & !x & HI_USIZE
}

/* ASSEMBLY OUTPUT FOR ABOVE
      movabs rcx, -72340172838076673
        add rcx, rdi
        not rdi
        movabs rax, -9187201950435737472
        and rax, rdi
        and rax, rcx
        ret

*/

/*
#[inline(never)]
pub const fn do_slower_swar(x: usize) -> usize {
    !(x.wrapping_add(INVERTED_HIGH) | x) & HI_USIZE
}
*/
/*

   movabs rcx, 9187201950435737471
        add rcx, rdi
        or rcx, rdi
        not rcx
        movabs rax, -9187201950435737472
        and rax, rcx
        ret

*/

const LO_USIZE: usize = repeat_u8(0x01);
const HI_USIZE: usize = repeat_u8(0x80);
const INVERTED_HIGH: usize = !HI_USIZE;
const _: () = const { assert!(INVERTED_HIGH == repeat_u8(0x7F), "should be equal") };
const USIZE_BYTES: usize = size_of::<usize>();

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
    // rust elides the bounds check (asm checked )
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
        let slice = unsafe { text.get_unchecked(..offset) };
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
            // use nonzerousize for faster intrinsics (skipping all 0 case, faster on most architectures)
            // then  XOR to turn the matching bytes to NUL
            if let Some(lower) = NonZeroUsize::new(do_fast_swar(u ^ repeated_x)) {
                #[cfg(target_endian = "little")]
                let byte_pos = (lower.trailing_zeros() >> 3) as usize;
                #[cfg(target_endian = "big")]
                let byte_pos = (lower.leading_zeros() >> 3) as usize;
                return Some(offset + byte_pos);
            }
            if let Some(upper) = NonZeroUsize::new(do_fast_swar(v ^ repeated_x)) {
                #[cfg(target_endian = "little")]
                let byte_pos = (upper.trailing_zeros() >> 3) as usize;
                #[cfg(target_endian = "big")]
                let byte_pos = (upper.leading_zeros() >> 3) as usize;
                return Some(offset + USIZE_BYTES + byte_pos);
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
// * ESSENTIALLY, this stops borrows propagating leftwards
const fn find_zero_byte_reversed(x: usize) -> Option<NonZeroUsize> {
    // Convert the non-zero-byte mask into a zero-byte mask (0x80 set in each
    // byte that is zero). Sa]
    //TODO WRITE UP ALGORITHM
    let zero_mask = !(x.wrapping_add(INVERTED_HIGH) | x) & HI_USIZE;
    // Utilise NonZeroUsize solely for intrinsic benefit (cttz/ctlz)_nonzero.
    NonZeroUsize::new(zero_mask)
}

#[inline]
// Check assembly to see if we need this Adrian, you did it lol.
// 1 fewer instruction using this, need to look at more.
const unsafe fn rposition_byte_len(base: *const u8, len: usize, needle: u8) -> Option<usize> {
    let mut i = len;
    while i != 0 {
        i -= 1;
        // TODO write verbose safety stuff
        if unsafe { base.add(i).read() } == needle {
            return Some(i);
        }
    }
    None
}

/// Returns the last index matching the byte `x` in `text`.
///
#[must_use]
#[inline]
#[allow(clippy::cast_ptr_alignment)] //burntsushi wrote this so...
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

    // SAFETY: trivially within bounds
    let start = text.as_ptr();
    let tail_len = len - offset; // tail is [offset, len)
    if let Some(i) = unsafe { rposition_byte_len(start.add(offset), tail_len, x) } {
        return Some(offset + i);
    }

    // Search the body of the text, make sure we don't cross min_aligned_offset.

    // offset is always aligned, so just testing `>` is sufficient and avoids possible

    // overflow.

    let repeated_x = repeat_u8(x);

    while offset > min_aligned_offset {
        // SAFETY: offset starts at len - suffix.len(), as long as it is greater than
        // min_aligned_offset (prefix.len()) the remaining distance is at least 2 * chunk_bytes.
        // SAFETY: as above
        let lower = unsafe { ptr.add(offset - 2 * USIZE_BYTES).cast::<usize>().read() };
        // SAFETY: as above
        let upper = unsafe { ptr.add(offset - USIZE_BYTES).cast::<usize>().read() };

        // Break if there is a matching byte.
        // **CHECK UPPER FIRST**
        let xorred_upper = upper ^ repeated_x; //XOR to turn the matching bytes to NUL
        // use the original SWAR (fewer instructions) to check for zero byte
        if let Some(valid) = find_zero_byte_reversed(xorred_upper) {
            #[cfg(target_endian = "little")]
            let zero_byte_pos = USIZE_BYTES - 1 - (valid.leading_zeros() >> 3) as usize;
            #[cfg(target_endian = "big")]
            let zero_byte_pos = USIZE_BYTES - 1 - (valid.trailing_zeros() >> 3) as usize;
            return Some(offset - USIZE_BYTES + zero_byte_pos);
        }

        let xorred_lower = lower ^ repeated_x;
        if let Some(valid) = find_zero_byte_reversed(xorred_lower) {
            // TODO, TIDY UP SAFETY? use nonzerousize etc.
            // same stuff as above otherwise
            // SAFETY: GUARANTEED NON ZERO
            #[cfg(target_endian = "little")]
            let zero_byte_pos = USIZE_BYTES - 1 - (valid.leading_zeros() >> 3) as usize;
            #[cfg(target_endian = "big")]
            let zero_byte_pos = USIZE_BYTES - 1 - (valid.trailing_zeros() >> 3) as usize;

            return Some(offset - 2 * USIZE_BYTES + zero_byte_pos);
        }

        offset -= 2 * USIZE_BYTES;
    }
    // SAFETY: trivially within bounds
    // Find the byte before the point the body loop stopped.
    unsafe { rposition_byte_len(start, offset, x) }
}
