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
            if let Some(lower) = contains_zero_byte(u ^ repeated_x) {
                #[cfg(target_endian = "little")]
                let byte_pos = (lower.trailing_zeros() >> 3) as usize;
                #[cfg(target_endian = "big")]
                let byte_pos = (lower.leading_zeros() >> 3) as usize;
                return Some(offset + byte_pos);
            }
            if let Some(upper) = contains_zero_byte(v ^ repeated_x) {
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


FROM HACKERS DELIGHT

https://github.com/lancetw/ebook-1/blob/master/02_algorithm/Hacker%27s%20Delight%202nd%20Edition.pdf

WE DONT USE zbyter because it requires A LOT more instructions to check for 0 byte,

I havent tested the mycro one, seems interesting though.


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


However this *FIX* also exists, I may try it.. obviously it's 32bit and needs to be scaled to be word size independent.


FIGURE 5–24. Number of trailing zeros, Gaudet’s algorithm.
As shown, the code uses the C “conditional expression” in six places. This construct has the form a?
b:c. Its value is b if a is true (nonzero), and c if a is false (zero). Although a conditional expression
must, in general, be compiled into compares and branches, for the simple cases in Figure 5–24
branching can be avoided if the machine has a compare for equality to zero instruction that sets a
target register to 1 if the operand is 0, and to 0 if the operand is nonzero. Branching can also be
avoided by using conditional move instructions. Using compare, the assignment to b3 can be compiled
into five instructions on the basic RISC: two to generate the hex constant, an and, the compare, and a
shift left of 3. (The first, second, and last conditional expressions require one, three, and four
instructions, respectively.)
The code can be compiled into a total of 30 instructions. All six lines with the conditional
expressions can run in parallel. On a machine with a sufficient degree of parallelism, it executes in
ten cycles. Present machines don’t have that much parallelism, so as a practical matter it might help to
change the first two uses of y in the program to x. This permits the first three executable statements to
run in parallel.
David Seal [Seal2] devised an algorithm for computing ntz(x) that is based on the idea of
compressing the 232 possible values of x to a small dense set of integers and doing a table lookup. He
uses the expression x & – x to reduce the number of possible values to a small number. The value of
this expression is a word that contains a single 1-bit at the position of the least significant 1-bit in x,
or is 0 if x = 0. Thus, x & – x has only 33 possible values. But they are not dense; they range from 0
to 231.
To produce a dense set of 33 integers that uniquely identify the 33 values of x & –x, Seal found a
certain constant which, when multiplied by x & –x, produces the identifying value in the high-order
six bits of the low-order half of the product of the constant and x & –x. Since x & – x is an integral
power of 2 or is 0, the multiplication amounts to a left shift of the constant, or it is a multiplication by
0. Using only the high-order five bits is not sufficient, because 33 distinct values are needed.
The code is shown in Figure 5–25, where table entries shown as u are unused.



\
int ntz(unsigned x) {
static char table[64] =
{32, 0, 1,12, 2, 6, u,13, 3, u, 7, u, u, u, u,14,
10, 4, u, u, 8, u, u,25, u, u, u, u, u,21,27,15,
31,11, 5, u, u, u, u, u, 9, u, u,24, u, u,20,26,
30, u, u, u, u,23, u,19, 29, u,22,18,28,17,16, u};
x = (x & -x)*0x0450FBAF;
return table[x >> 26];
}

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

    let start = text.as_ptr();
    let tail_len = len - offset; // tail is [offset, len)
    // SAFETY: trivially within bounds
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
        //XOR to turn the matching bytes to NUL
        // This swar algorithm has the benefit of not propagating 0xFF rightwards/leftwards after a match is found
        if let Some(num) = contains_zero_byte_reversed(upper ^ repeated_x) {
            #[cfg(target_endian = "little")]
            let zero_byte_pos = USIZE_BYTES - 1 - (num.leading_zeros() >> 3) as usize;
            #[cfg(target_endian = "big")]
            let zero_byte_pos = USIZE_BYTES - 1 - (num.trailing_zeros() >> 3) as usize;

            return Some(offset - USIZE_BYTES + zero_byte_pos);
        }

        // same as above
        if let Some(num) = contains_zero_byte_reversed(lower ^ repeated_x) {
            #[cfg(target_endian = "little")]
            let zero_byte_pos = USIZE_BYTES - 1 - (num.leading_zeros() >> 3) as usize;
            #[cfg(target_endian = "big")]
            let zero_byte_pos = USIZE_BYTES - 1 - (num.trailing_zeros() >> 3) as usize;

            return Some(offset - 2 * USIZE_BYTES + zero_byte_pos);
        }

        offset -= 2 * USIZE_BYTES;
    }
    // SAFETY: trivially within bounds
    // Find the byte before the point the body loop stopped.
    unsafe { rposition_byte_len(start, offset, x) }
}
