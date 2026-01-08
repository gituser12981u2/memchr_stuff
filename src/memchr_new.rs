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

const LO_USIZE: usize = repeat_u8(0x01);
const HI_USIZE: usize = repeat_u8(0x80);
const INVERTED_HIGH: usize = !HI_USIZE;
const _: () = const { assert!(INVERTED_HIGH == repeat_u8(0x7F), "should be equal") };
const USIZE_BYTES: usize = size_of::<usize>();

#[inline(never)]
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

/*
http://www.icodeguru.com/Embedded/Hacker%27s-Delight/043.htm



Figure 6-2 Find leftmost 0-byte, branch-free code.

int zbytel(unsigned x) {

   unsigned y;

   int n;

                        ?/ Original byte: 00 80 other

   y = (x & 0x7F7F7F7F) + 0x7F7F7F7F;   // 7F 7F 1xxxxxxx

   y = ~(y | x | 0x7F7F7F7F);           // 80 00 00000000

   n = nlz(y) >> 3;             // n = 0 ... 4, 4 if x

   return n;                  ?// has no 0-byte.

}

The position of the rightmost 0-byte is given by the number of trailing 0's in the final value of y computed above, divided by 8 (with fraction discarded). Using the expression for computing the number of trailing 0's by means of the number of leading zeros instruction (see Section 5- 4, "Counting Trailing 0's," on page 84), this can be computed by replacing the assignment to n in the procedure above with:

n = (32 - nlz(~y & (y - 1))) >> 3;

This is a 12-instruction solution, if the machine has nor and and not.

In most situations on PowerPC, incidentally, a procedure to find the rightmost 0-byte would not be needed. Instead, the words can be loaded with the load word byte-reverse instruction (lwbrx).

The procedure of Figure 6-2 is more valuable on a 64-bit machine than on a 32-bit one, because on a 64-bit machine the procedure (with obvious modifications) requires about the same number of instructions (seven or ten, depending upon how the constant is generated), whereas the technique of Figure 6-1 requires 23 instructions worst case.

*/
#[inline]
#[must_use]
// TODO TIDY UP SAFETY (write up a safety proof for this)
// * ESSENTIALLY, if fast SWAR!=0 then this will also not be 0, different formulations
// of the same logic. only difference is this takes more instructions (due to right/left propagation in HASZERO)
const unsafe fn find_zero_byte_reversed(x: usize) -> usize {
    debug_assert!(do_fast_swar(x) != 0);
    let y = (x & INVERTED_HIGH).wrapping_add(INVERTED_HIGH);
    // essentially, this algorithm can only be used after the SWAR algorithm has been done on the XOR'ed usize previously,
    // TODO write verbose safety stuff for PR
    // Utilise NonZeroUsize solely for intrinsic benefit (cttz/ctlz)_nonzero
    let ans = unsafe { NonZeroUsize::new_unchecked(!(y | x | INVERTED_HIGH)) };
    #[cfg(target_endian = "little")]
    {
        (ans.leading_zeros() >> 3) as usize
    }
    #[cfg(target_endian = "big")]
    {
        (ans.trailing_zeros() >> 3) as usize
    }
    // use USIZE_BYTES-1 - result of <this> to find the position
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
#[inline(never)]
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
        if do_fast_swar(xorred_upper) != 0 {
            // Then apply alternative SWAR (guaranteed to be nonzero)
            // We need to use an alternative SWAR method because HASZERO propagates 0xFF right(or left, depending on endianness) wise after match
            // this could be done with a byte swap but thats 1 (or more, depending on arch) instructions
            // use this only when a match is FOUND
            // SAFETY: GUARANTEED NON ZERO
            let zero_byte_pos = USIZE_BYTES - 1 - unsafe { find_zero_byte_reversed(xorred_upper) };
            return Some(offset - USIZE_BYTES + zero_byte_pos);
        }

        let xorred_lower = lower ^ repeated_x;
        if do_fast_swar(xorred_lower) != 0 {
            // TODO, TIDY UP SAFETY? use nonzerousize etc.
            // same stuff as above otherwise
            // SAFETY: GUARANTEED NON ZERO
            let zero_byte_pos = USIZE_BYTES - 1 - unsafe { find_zero_byte_reversed(xorred_lower) };

            return Some(offset - 2 * USIZE_BYTES + zero_byte_pos);
        }

        offset -= 2 * USIZE_BYTES;
    }
    // SAFETY: trivially within bounds
    // Find the byte before the point the body loop stopped.
    unsafe { rposition_byte_len(start, offset, x) }
}
