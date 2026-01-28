// TODO? test benchmarks on 32bit targets, 64bit BE works but is extremely annoying to test on VM (thus benchmarks by emulation are not ideal.)
// Original implementation taken from https://doc.rust-lang.org/src/core/slice/memchr.rs.html

//Check comprehensive tests in ./test.rs please

// Please see commentary on assembly analysis at bottom

/// TODO: change to work with rust std.
use crate::num::repeat_u8; //usize::repeat... is a private function in std lib internals, mock it up with this.
// [u8;size_of::<usize>] ^

// to match with STD
use core::intrinsics::const_eval_select;

// USE THIS TO ENABLE BETTER INTRINSICS AKA CTLZ_NONZERO/CTTZ_NONZERO
use core::num::NonZeroUsize;
//https://doc.rust-lang.org/src/core/num/nonzero.rs.html#599
//https://doc.rust-lang.org/beta/std/intrinsics/fn.ctlz_nonzero.html
//https://doc.rust-lang.org/beta/std/intrinsics/fn.cttz_nonzero.html

const LO_USIZE: usize = repeat_u8(0x01);
const HI_USIZE: usize = repeat_u8(0x80);
const USIZE_BYTES: usize = size_of::<usize>();

// Simple code simplification tools (replace with in the functions if wanted)
#[inline]
pub(crate) const fn find_first_nul(num: NonZeroUsize) -> usize {
    #[cfg(target_endian = "little")]
    {
        (num.trailing_zeros() >> 3) as usize
    }

    #[cfg(target_endian = "big")]
    {
        (num.leading_zeros() >> 3) as usize
    }
}
// as above
#[inline]
pub(crate) const fn find_last_nul(num: NonZeroUsize) -> usize {
    #[cfg(target_endian = "big")]
    {
        USIZE_BYTES - 1 - ((num.trailing_zeros()) >> 3) as usize
    }

    #[cfg(target_endian = "little")]
    {
        USIZE_BYTES - 1 - ((num.leading_zeros()) >> 3) as usize
    }
}

#[inline]
// Make this private eventually, only needed for tests (as public)
pub(crate) const fn contains_zero_byte(input: usize) -> Option<NonZeroUsize> {
    // Classic HASZERO trick. (Mycroft)
    NonZeroUsize::new(input.wrapping_sub(LO_USIZE) & !input & HI_USIZE)
}

#[inline]
#[must_use]
pub const fn memchr(x: u8, text: &[u8]) -> Option<usize> {
    // Fast path for small slices.
    if text.len() < 2 * USIZE_BYTES {
        return memchr_naive(x, text);
    }

    // The runtime version behaves the same as the compiletime version, it's just more optimized.
    const_eval_select((x, text), memchr_naive, memchr_aligned)
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
        // TEST this? for the unaligned head, if overall len>8, we could do an unaligned read and check for zero bytes in that
        // Although that *WOULD* require generating the repeated constant, an unaligned load, etc... maybe not worth the complexity.
        offset = offset.min(len);
        let slice = &text[..offset]; //compiler elides checks on this, no panic branch.
        if let Some(index) = memchr_naive(x, slice) {
            return Some(index);
        }
    }

    // search the (aligned)body of the text
    let repeated_x = repeat_u8(x);
    while offset <= len - 2 * USIZE_BYTES {
        // SAFETY: the while's predicate guarantees a distance of at least 2 * usize_bytes
        // between the offset and the end of the slice.
        // the body is trivially aligned due to align_to, avoid the cost of unaligned reads(same as memchr in STD)
        unsafe {
            let lower = *(ptr.add(offset) as *const usize);
            let upper = *(ptr.add(offset + USIZE_BYTES) as *const usize);

            // check this branch first (lower has precedence, obvs, we want the FIRST match)
            // use nonzerousize for faster intrinsics (skipping all 0 case, faster on most architectures)
            // then  XOR to turn the matching bytes to NUL and NUL to `x`

            // on forward search, we dont need to care about borrow propagation affecting trailing_zeros (ON LE)
            // However we do have to care about borrow propagation on BE
            #[cfg(target_endian = "little")]
            let maybe_match_lower = contains_zero_byte(lower ^ repeated_x);
            // Luckily, it's the same number of operations to check for 0 byte as original memchr, woo!
            #[cfg(target_endian = "big")]
            let maybe_match_lower = contains_zero_byte_borrow_fix(lower ^ repeated_x);

            if let Some(lower_valid) = maybe_match_lower {
                // Replace with actual definition if wanted
                let zero_byte_pos = find_first_nul(lower_valid);
                // Early return on finding the first NUL
                return Some(offset + zero_byte_pos);
            }

            #[cfg(target_endian = "little")]
            let maybe_match_upper = contains_zero_byte(upper ^ repeated_x);
            #[cfg(target_endian = "big")]
            let maybe_match_upper = contains_zero_byte_borrow_fix(upper ^ repeated_x);

            if let Some(upper_valid) = maybe_match_upper {
                let zero_byte_pos = find_first_nul(upper_valid);

                return Some(offset + USIZE_BYTES + zero_byte_pos);
            }
        }

        offset += USIZE_BYTES * 2;
    }

    // Find the byte in the unaligned tail if not already found in the aligned loop.

    let slice =
            // SAFETY: offset is within bounds
                unsafe { core::slice::from_raw_parts(ptr.add(offset), len - offset) };

    memchr_naive(x, slice).map(|i| offset + i)
}

/*
Detects zero bytes in a word using a borrow-safe SWAR algorithm.

This function uses a hybrid two-stage approach to detect zero bytes whilst avoiding
cross-byte false positives caused by borrow propagation in the classic SWAR algorithm.

Algorithm:

Stage 1: Fast rejection using classic SWAR

The classic HASZERO test (`(input - 0x0101...) & ~input & 0x8080...`) provides a
fast early-out for the common case where no zero bytes exist. However, this test
can produce false positives when the subtraction borrows across byte boundaries.

Stage 2: Borrow correction

When stage 1 detects potential zero bytes, we apply a correction mask to eliminate
false positives. The key insight: if a byte's LSB is 1 (e.g., 0x01), it cannot be
zero but may appear as a false positive due to borrow from an adjacent zero byte.

Example of borrow propagation (LE byte order):
- Input: `[0x00, 0x01]`
- Subtracting 0x0101.. borrows from the 0x01 byte when processing 0x00
- Classic SWAR reports both bytes as candidates despite only 0x00 being truly zero

The correction `classic &= !input << 7` clears spurious bits:
- `!input << 7` shifts each byte's LSB into the high bit (0x80 position)
- ANDing with the classic mask eliminates candidates with LSB=1
- *Since `!input` was already computed, this reuses it efficiently*

Performance:

- Adds only 2 instructions vs. classic SWAR (branch + shift)
- Early exit on no-match case maintains fast-path performance
- Avoids the 3-MOV overhead of alternative borrow-free algorithms (e.g., Wojciech Muła's)
- Branch cost is acceptable since callers (memchr/memrchr) already branch on the result

Comparison with alternatives:

Alternative borrow-free approach (http://0x80.pl/notesen/2016-11-28-simd-strfind.html):
```c
const uint64_t t0 = (~x & 0x7f7f7f7f7f7f7f7fllu) + 0x0101010101010101llu;
const uint64_t t1 = (~x & 0x8080808080808080llu);
uint64_t zeros = t0 & t1;
```
This has similar instruction count but lacks early-out optimisation, which is critical
for memchr/memrchr performance.

*/
#[inline]
#[must_use]
pub(crate) const fn contains_zero_byte_borrow_fix(input: usize) -> Option<NonZeroUsize> {
    /* Stage 1: Classic SWAR test for fast rejection */
    let mut classic = input.wrapping_sub(LO_USIZE) & !input & HI_USIZE;
    if classic == 0 {
        return None;
    }

    /* Stage 2: Eliminate borrow-induced false positives */
    classic &= !input << 7;

    /*
    SAFETY: `classic != 0` from stage 1 guarantees at least one true zero byte exists.
    The stage 2 correction only clears false positives, never true matches.
    */
    Some(unsafe { NonZeroUsize::new_unchecked(classic) })
}

/// Returns the last index matching the byte `x` in `text`.
///
#[must_use]
#[allow(clippy::missing_inline_in_public_items)] // Match semantics of std
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

    // Skip the checked indexing, adds ~10 instructions!

    /*
    SAFETY: `offset` is computed as `len - suffix.len()`, so `offset <= len`.
    Therefore the range `offset..` is a valid subslice of `text`.
    */
    if let Some(index) = unsafe {
        text.get_unchecked(offset..)
            .iter()
            .rposition(|elt| *elt == x)
    } {
        return Some(offset + index);
    }

    // Search the body of the text, make sure we don't cross min_aligned_offset.

    // offset is always aligned, so just testing `>` is sufficient and avoids possible

    // overflow.

    let repeated_x = repeat_u8(x);

    while offset > min_aligned_offset {
        // SAFETY: offset starts at len - suffix.len(), as long as it is greater than
        // min_aligned_offset (prefix.len()) the remaining distance is at least 2 * chunk_bytes.
        // SAFETY: the body is trivially aligned due to align_to, avoid the cost of unaligned reads(same as memchr/memrchr in STD)
        let lower = unsafe { *(ptr.add(offset - 2 * USIZE_BYTES) as *const usize) };
        // I would write this as the below, unfortunately I want to keep semantics(although trivial, in track with STDLIB)
        // let lower = unsafe { ptr.add(offset - 2 * USIZE_BYTES).cast::<usize>().read() };
        // SAFETY: as above
        let upper = unsafe { *(ptr.add(offset - USIZE_BYTES) as *const usize) };

        // Break if there is a matching byte.
        // **CHECK UPPER FIRST**
        //XOR to turn the matching bytes to NUL
        // This swar algorithm has the benefit of not propagating 0xFF rightwards/leftwards after a match is found

        #[cfg(target_endian = "big")]
        let maybe_match_upper = contains_zero_byte(upper ^ repeated_x);
        #[cfg(target_endian = "little")]
        // because of borrow issues propagating to LSB we need to do a fix for LE, not for BE though, slight win?!
        let maybe_match_upper = contains_zero_byte_borrow_fix(upper ^ repeated_x);

        if let Some(num) = maybe_match_upper {
            // replace this function with actual definition if wanted

            let zero_byte_pos = find_last_nul(num);

            return Some(offset - USIZE_BYTES + zero_byte_pos);
        }

        #[cfg(target_endian = "big")]
        let maybe_match_lower = contains_zero_byte(lower ^ repeated_x);
        #[cfg(target_endian = "little")]
        let maybe_match_lower = contains_zero_byte_borrow_fix(lower ^ repeated_x);

        if let Some(num) = maybe_match_lower {
            // as above.
            let zero_byte_pos = find_last_nul(num);

            return Some(offset - 2 * USIZE_BYTES + zero_byte_pos);
        }

        offset -= 2 * USIZE_BYTES;
    }
    // Find the last match in the remaining prefix.
    /*
     SAFETY: `offset` is monotonically decreased from `max_aligned_offset <= len`,
     and the loop condition guarantees `offset >= min_aligned_offset >= 0`.
     Thus `..offset` is always a valid range for `text`.
    */
    unsafe {
        text.get_unchecked(..offset)
            .iter()
            .rposition(|elt| *elt == x)
    }
}

/*
MY STUPID COMMENTARY

FROM HACKERS DELIGHT

https://github.com/lancetw/ebook-1/blob/master/02_algorithm/Hacker%27s%20Delight%202nd%20Edition.pdf

WE DONT USE zbyter because it requires A LOT more instructions to check for 0 byte,

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
y = ~(y | x | 0x7F7F7F7F); // 80 00 00000000
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

**ALSO NOTE, NO POINT REIMPLEMENTING TRAILING/LEADING ZEROS FOR WEIRD ARCHITECTURES, since LLVM will have a good builtin if the arch
lacks the instruction and has to software emulate it. I trust LLVM maintainers to be a lot better than me at this!**

*/

/*


// Basically my optimisation has the same instruction counts as the original borrow free version
// However it allows the early out. Nice win!
 */

/*
#[inline(never)]
// Wojciech's translation into Rust
pub const fn contains_zero_woj(x: usize) -> Option<NonZeroUsize> {
    let t0 = (!x & !HI_USIZE) + LO_USIZE;
    let t1 = !x & HI_USIZE;
    NonZeroUsize::new(t0 & t1)
}
*/

/*

2 extra instructions for Original borrow free version
 memchr_stuff[f91d351e7b3844a]::memchr_new::contains_zero_woj:
 not     rdi
 movabs  rax, 9187201950435737471
 and     rax, rdi
 movabs  rcx, 72340172838076673
 add     rcx, rax
 movabs  rax, -9187201950435737472
 and     rax, rdi
 and     rax, rcx
 ret



 memchr_stuff[f91d351e7b3844a]::memchr_new::contains_zero_byte:
 movabs  rcx, -72340172838076673
 add     rcx, rdi
 not     rdi
 movabs  rax, -9187201950435737472
 and     rax, rdi
 and     rax, rcx
 ret

*/

/*
#[inline(never)]
pub const fn contains_zero_byte_new(x: usize) -> Option<NonZeroUsize> {
    NonZeroUsize::new(x.wrapping_sub(LO_USIZE) & !x & HI_USIZE & (!x << 7))
}

*/
/*

memchr_stuff[f91d351e7b3844a]::memchr_new::contains_zero_byte_new:
    movabs  rcx, -72340172838076673
add     rcx, rdi
not     rdi
and     rcx, rdi
shl     rdi, 7
movabs  rax, -9187201950435737472
and     rax, rdi
and     rax, rcx
ret

   */
