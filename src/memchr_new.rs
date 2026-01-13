// TODO? test on 32bit targets! (ok fuck 16 i dont think  rust even supports 16)
// Original implementation taken from https://doc.rust-lang.org/src/core/slice/memchr.rs.html

/// TODO: change to work with rust std.
use crate::num::repeat_u8;

/*

usize::repeat... is a private function in std lib internals, mock it up with this.

#[inline]
pub const fn repeat_u8(x: u8) -> usize {
    usize::from_ne_bytes([x; size_of::<usize>()])
}

*/

// USE THIS TO ENABLE BETTER INTRINSICS AKA CTLZ_NONZERO/CTTZ_NONZERO
use core::num::NonZeroUsize;
//https://doc.rust-lang.org/src/core/num/nonzero.rs.html#599
//https://doc.rust-lang.org/beta/std/intrinsics/fn.ctlz_nonzero.html
//https://doc.rust-lang.org/beta/std/intrinsics/fn.cttz_nonzero.html

#[inline]
pub(crate) const fn contains_zero_byte(x: usize) -> Option<NonZeroUsize> {
    NonZeroUsize::new(x.wrapping_sub(LO_USIZE) & !x & HI_USIZE)
}

const LO_USIZE: usize = repeat_u8(0x01);
const HI_USIZE: usize = repeat_u8(0x80);
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
        let slice = &text[..offset]; //compiler elides checks on this, no panic branch.
        if let Some(index) = memchr_naive(x, slice) {
            return Some(index);
        }
    }

    // search the body of the text
    let repeated_x = repeat_u8(x);
    while offset <= len - 2 * USIZE_BYTES {
        // SAFETY: the while's predicate guarantees a distance of at least 2 * usize_bytes
        // between the offset and the end of the slice.
        // the body is trivially aligned due to align_to, avoid the cost of unaligned reads(same as memchr in STD)
        unsafe {
            let u = *(ptr.add(offset) as *const usize);
            let v = *(ptr.add(offset + USIZE_BYTES) as *const usize);

            // break if there is a matching byte
            // ! OPTIMIZATION !
            // check this branch first (lower has precedence, obvs)
            // use nonzerousize for faster intrinsics (skipping all 0 case, faster on most architectures)
            // then  XOR to turn the matching bytes to NUL and NUL to `x`
            if let Some(lower) = contains_zero_byte(u ^ repeated_x) {
                #[cfg(target_endian = "little")]
                let byte_pos = (lower.trailing_zeros() >> 3) as usize;
                #[cfg(target_endian = "big")]
                let byte_pos = (lower.leading_zeros() >> 3) as usize;

                debug_assert!(
                    Some(byte_pos) == u.to_ne_bytes().iter().position(|b| b | *b == x),
                    "manual verification failed in lower memchr loop"
                );
                return Some(offset + byte_pos);
            }
            if let Some(upper) = contains_zero_byte(v ^ repeated_x) {
                #[cfg(target_endian = "little")]
                let byte_pos = (upper.trailing_zeros() >> 3) as usize;
                #[cfg(target_endian = "big")]
                let byte_pos = (upper.leading_zeros() >> 3) as usize;

                debug_assert!(
                    Some(byte_pos) == v.to_ne_bytes().iter().position(|b| b | *b == x),
                    "manual verification failed in upper memchr loop"
                );

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
MY STUPID COMMENTARY

FROM HACKERS DELIGHT

https://github.com/lancetw/ebook-1/blob/master/02_algorithm/Hacker%27s%20Delight%202nd%20Edition.pdf

WE DONT USE zbyter because it requires A LOT more instructions to check for 0 byte,

I havent tested the mycro one, seems interesting though. I believe its meant for big endian however hence the comment

"

executes in only five instructions exclusive of loading the constants if the machine
has the
and not and
number of trailing zeros instructions. It cannot be used to
compute zbytel(
x), because of a problem with borrows. It would be most useful for  <<-------------BORROW PROBLEM ffs
finding the first 0-byte in a character string on a little-endian machine, or to simply test
for a 0-byte (using only the assignment to y) on a machine of either endianness.
"


int zbytel(unsigned x) {
unsigned y;
int n;
// Original byte: 00 80 other
y = (x & 0x7F7F7F7F)+ 0x7F7F7F7F; // 7F 7F 1xxxxxxx
y = ~(y 1 x 1 0x7F7F7F7F); // 80 00 00000000
n = nlz(y) >> 3; // n = 0 ... 4, 4 if x
return n; // has no 0-byte.
}
FIGURE 6–2. Find leftmost 0-byte, branch-free code.
The position of the rightmost 0-byte is given by the number of trailing 0’s in the final value of y
computed above, divided by 8 (with fraction discarded). Using the expression for computing the
number of trailing 0’s by means of the number of leading zeros instruction (see Section 5–4,
“Counting Trailing 0’s ,” on page 107), this can be computed by replacing the assignment to n in the
procedure above with:
Click here to view code image
n = (32 - nlz(~y & (y - 1))) >> 3;


.


**ALSO NOTE, NO POINT REIMPLEMENTING TRAILING ZEROS FOR WEIRD ARCHITECTURES, since LLVM will have a good builtin if the arch
lacks the instruction and has to software emulate it. I trust LLVM maintainers to be a lot better than me at this!**


*/

#[inline]
#[must_use]
pub const fn contains_zero_byte_reversed(input: usize) -> Option<NonZeroUsize> {
    // Hybrid approach:
    // 1) Use the classic SWAR test as a cheap early-out for the common case
    //    where there are no zero bytes.
    // 2) If the classic test indicates a possible match, compute a borrow/carry-
    //    safe mask that cannot produce cross-byte false positives. This matters
    //    for reverse search where we pick the *last* match.

    // Classic SWAR: may contain false positives due to cross-byte borrow.
    // However considering that we want to check *as quickly* as possible, this is ideal.

    let classic = input.wrapping_sub(LO_USIZE) & !input & HI_USIZE;
    if classic == 0 {
        return None;
    }
    // This function occurs a branch here contains zero byte doesn't, it delegates the branch
    // to the memchr function, this is okay because a *branch still occurs*

    // Borrow-safe (carry-safe) SWAR:
    // mask off high bits so per-byte addition can't carry into the next byte.
    // This adds an extra 4 instructions on x86 without BMI intrinsics (would be fewer with andn? same goes for standard SWAR)
    // Not falling into that trap!
    let zero_mask = classic & !((input & LO_USIZE) << 7);

    // Remove this *probably* due to debug checks already doing so(this just provides amore helpful warning)
    debug_assert!(
        zero_mask != 0,
        "should never be 0 (checked by debug assertions in nonzerousize however too, just this is explicit)"
    );

    // SAFETY: `classic != 0` implies there is at least one real zero byte
    // somewhere in the word (false positives only occur alongside a real zero
    // due to borrow propagation), so `zero_mask` must be non-zero.
    // Use this to get smarter intrinsic (aka ctlz/cttz non_zero)
    Some(unsafe { NonZeroUsize::new_unchecked(zero_mask) })
}

#[inline]
// Check assembly to see if we need this Adrian, you did it lol.
// 1 fewer instruction using this, need to look at more.
const unsafe fn rposition_byte_len(base: *const u8, len: usize, needle: u8) -> Option<usize> {
    let mut i = len;
    while i != 0 {
        i -= 1;
        // SAFETY: trivially within bounds
        if unsafe { base.add(i).read() } == needle {
            return Some(i);
        }
    }
    None
}

/// Returns the last index matching the byte `x` in `text`.
///
#[must_use]
#[inline] // check inline semantics against STD
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
    // SAFETY: trivially within bounds
    if let Some(i) = unsafe { rposition_byte_len(start.add(offset), tail_len, x) } {
        return Some(offset + i);
    }
    /*
    This adds an extra ~10 instructions!(on x86 v1) (from std.) definitely worthwhile to avoid!

     if let Some(index) = text[offset..].iter().rposition(|elt| *elt == x) {
        return Some(offset + index);
    }


     */

    // Search the body of the text, make sure we don't cross min_aligned_offset.

    // offset is always aligned, so just testing `>` is sufficient and avoids possible

    // overflow.

    let repeated_x = repeat_u8(x);

    while offset > min_aligned_offset {
        // SAFETY: offset starts at len - suffix.len(), as long as it is greater than
        // min_aligned_offset (prefix.len()) the remaining distance is at least 2 * chunk_bytes.
        // SAFETY: the body is trivially aligned due to align_to, avoid the cost of unaligned reads(same as memchr/memrchr in STD)
        let lower = unsafe { *(ptr.add(offset - 2 * USIZE_BYTES) as *const usize) };
        let upper = unsafe { *(ptr.add(offset - USIZE_BYTES) as *const usize) };

        // Break if there is a matching byte.
        // **CHECK UPPER FIRST**
        //XOR to turn the matching bytes to NUL
        // This swar algorithm has the benefit of not propagating 0xFF rightwards/leftwards after a match is found
        if let Some(num) = contains_zero_byte_reversed(upper ^ repeated_x) {
            #[cfg(target_endian = "little")]
            let zero_byte_pos = USIZE_BYTES - 1 - (num.leading_zeros() >> 3) as usize;
            #[cfg(target_endian = "big")]
            let zero_byte_pos = USIZE_BYTES - 1 - (num.trailing_zeros() >> 3) as usize;

            debug_assert!(
                Some(zero_byte_pos) == upper.to_ne_bytes().iter().rposition(|b| b | *b == x),
                "manual verification failed in upper memrchr loop"
            );

            return Some(offset - USIZE_BYTES + zero_byte_pos);
        }

        // same as above
        if let Some(num) = contains_zero_byte_reversed(lower ^ repeated_x) {
            #[cfg(target_endian = "little")]
            let zero_byte_pos = USIZE_BYTES - 1 - (num.leading_zeros() >> 3) as usize;
            #[cfg(target_endian = "big")]
            let zero_byte_pos = USIZE_BYTES - 1 - (num.trailing_zeros() >> 3) as usize;

            debug_assert!(
                Some(zero_byte_pos) == lower.to_ne_bytes().iter().rposition(|b| b | *b == x),
                "manual verification failed in lower memrchr loop"
            );

            return Some(offset - 2 * USIZE_BYTES + zero_byte_pos);
        }

        offset -= 2 * USIZE_BYTES;
    }
    // SAFETY: trivially within bounds
    // Find the byte before the point the body loop stopped.
    unsafe { rposition_byte_len(start, offset, x) }
}
