// TODO? test benchmarks on 32bit targets, 64bit BE works but is extremely slow on VM (thus benchmarks by emulation are not ideal.)
// Original implementation taken from https://doc.rust-lang.org/src/core/slice/memchr.rs.html
#![allow(dead_code)] //remove when finished
//Check comprehensive tests in ./test.rs please


// NOTE: this was written in a shitty vim on a 2gb laptop because my laptop broke
// when I get a new one, ill tidy it up, editing is painful.
// Unfortunately I got too bored....

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
#[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
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

// TODO test if can get compiler to generate vectorised instructions
// without using intrinsics
// Then do this for SSE2/loong arch
// as seen in approach here https://github.com/rust-lang/rust/blob/94a0cd15f5976fa35e5e6784e621c04e9f958e57/library/core/src/slice/ascii.rs#L582

// install this for testing when I get a new PC
// https://www.qemu.org/docs/master/system/target-loongarch.html


// SSE2 is baseline on x86_64, so we can use it for optimised memchr
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
fn memchr_aligned(x: u8, text: &[u8]) -> Option<usize> {
    use core::arch::x86_64::{__m128i as Element};
    use core::arch::x86_64::_mm_set1_epi8 as BROADCAST;
    use core::arch::x86_64::_mm_cmpeq_epi8 as COMPARE;
    use core::arch::x86_64::_mm_load_si128 as LOAD_ALIGNED;
    use core::arch::x86_64::_mm_movemask_epi8 as  MOVEMASK;
    use core::num::NonZeroI32;

 
    const ALIGNMENT: usize = align_of::<Element>();  
    const CHUNK_SIZE: usize = 4*ALIGNMENT;  // Process 4x  chunks at a time
    // The runtime version behaves the same as the compile time version, it's
    // just more optimised.

    // Scan for a single byte value by reading 4 SIMD registers at a time.
    //
    // Split `text` in three parts
    // - unaligned initial part, before the first  aligned address in text
    // - body, scan by 4x Alignment sized chunks at a time
    // - the last remaining part, < `CHUNK_SIZE`

    // search up to an aligned boundary
    let len = text.len();
    let ptr = text.as_ptr();
    let mut offset = ptr.align_offset(ALIGNMENT);

    if offset > 0 {
        offset = offset.min(len);
        let slice = &text[..offset];
        if let Some(index) = memchr_naive(x, slice) {
            return Some(index);
        }
    }

    // search the (aligned) body of the text using intrinsics
    unsafe {
        let needle = BROADCAST(x.cast_signed());
        
        while offset + CHUNK_SIZE <= len {
            // SAFETY: the while's predicate guarantees a distance of at least CHUNK_SIZE bytes
            // between the offset and the end of the slice.
            // The pointer is aligned to CHUNK_SIZE boundary.

           
            let chunk_ptr = ptr.add(offset).cast::<Element>();
           //debug_assert!(chunk_ptr.is_aligned_to(ALIGNMENT));
            
            // Load 4x ALIGNMENT sized -byte aligned chunks 
            let chunk0 = LOAD_ALIGNED(chunk_ptr);
            let chunk1 = LOAD_ALIGNED(chunk_ptr.add(1));
            let chunk2 = LOAD_ALIGNED(chunk_ptr.add(2));
            let chunk3 = LOAD_ALIGNED(chunk_ptr.add(3));
            
            // Compare each chunk with needle
            let cmp0 = COMPARE(chunk0, needle);
            let cmp1 = COMPARE(chunk1, needle);
            let cmp2 = COMPARE(chunk2, needle);
            let cmp3 = COMPARE(chunk3, needle);
            
            // Get bitmasks for each comparison and use a smarter intrinsic to use cttz_nonzero
            // Check each mask in order (first match wins)
            if let Some(vmask0)=NonZeroI32::new(MOVEMASK(cmp0)) {
                let byte_pos = vmask0.trailing_zeros() as usize;
                return Some(offset + byte_pos);
            }
            if let Some(vmask1)=NonZeroI32::new(MOVEMASK(cmp1)){
                let byte_pos = vmask1.trailing_zeros() as usize;
                return Some(offset +ALIGNMENT+ byte_pos);
            }

          if let Some(vmask2)=NonZeroI32::new(MOVEMASK(cmp2)){
                let byte_pos = vmask2.trailing_zeros() as usize;
                return Some(offset +(2*ALIGNMENT)+ byte_pos);
            }
            if let Some(vmask3)=NonZeroI32::new(MOVEMASK(cmp3)) {
                let byte_pos = vmask3.trailing_zeros() as usize;
                return Some(offset + (3*ALIGNMENT) + byte_pos);
            }
            
            offset += CHUNK_SIZE;
        }
    }

    // Find the byte in the unaligned tail if not already found in the aligned loop.
    let slice = unsafe { core::slice::from_raw_parts(ptr.add(offset), len - offset) };
    memchr_naive(x, slice).map(|i| offset + i)
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

#[inline]
#[must_use]
pub(crate) const fn contains_zero_byte_borrow_fix(input: usize) -> Option<NonZeroUsize> {
    /*
    Hybrid approach:
    1) Use the classic SWAR test as a cheap early-out for the common case
       where there are no zero bytes.
    2) If the classic test indicates a possible match, compute a borrow/carry-
       safe mask that cannot produce cross-byte false positives. This matters
       for reverse search where we pick the *last* match.

    Classic SWAR: may contain false positives due to cross-byte borrow.
    However considering that we want to check *as quickly* as possible, this is ideal.
    */
    let mut classic = input.wrapping_sub(LO_USIZE) & !input & HI_USIZE;
    if classic == 0 {
        return None;
    }
    /*
    This function occurs a branch here meanwhile contains_zero_byte doesn't, it delegates the branch
    to the memchr(on LE) (or opposite on BE) function, this is okay because a *branch still occurs*

    Borrow-safe (carry-safe) SWAR:

    The classic HASZERO mask is perfect for a boolean “any zero byte?” check, but the *per-byte* mask
    can contain extra 0x80 bits when the subtraction `input - 0x01..` borrows across byte lanes.
    That’s a problem here because we don’t just test “non-zero?” — we feed the mask into
    `leading_zeros`/`trailing_zeros` to pick an actual byte index.

    Example (two adjacent bytes, lowest first):
    - `input = [0x00, 0x01]`
    - subtracting `0x01..` borrows from the `0x00` byte into the next byte, so the classic mask may
      report both bytes as candidates even though only the first byte is truly zero.

    `input << 7` moves each byte’s low bit into that byte’s 0x80 position; bytes with LSB=1 (notably
    0x01, which is the common “borrow false-positive” case) get their candidate bit cleared.*/
    classic &= !(input << 7);
    // I didn't find this approach anywhere online, took a lot of work!
    // This approach adds an extra 3 instructions (or 2 if architecture has andn)
    // Meanwhile the typical approach in http://0x80.pl/notesen/2016-11-28-simd-strfind.html#swar
    /*

    // 7th bit set if lower 7 bits are zero
    const uint64_t t0 = (~x & 0x7f7f7f7f7f7f7f7fllu) + 0x0101010101010101llu;
    // 7th bit set if 7th bit is zero
    const uint64_t t1 = (~x & 0x8080808080808080llu);
    uint64_t zeros = t0 & t1;
    */
    // involves 3 mov's as opposed to only needing 2 MOV's here. It does 1 less ALU instruction than my approach but doesnt use an early 'OUT'
    // which is critical for memchr/memrchr
    /*
    SAFETY: `classic != 0` implies there is at least one real zero byte
    somewhere in the word (false positives only occur alongside a real zero
    due to borrow propagation), so  now `classic` must be non-zero.
    Use this to get smarter intrinsic (aka ctlz/cttz non_zero)
    Note: Debug assertions check classic!=0 so check tests for comprehensive validation
    */
    Some(unsafe { NonZeroUsize::new_unchecked(classic) })
}



/// Returns the last index matching the byte `x` in `text`.
// SSE2 is baseline on x86_64, so we can use it for optimised memrchr
#[must_use]
#[cfg(all(target_arch = "x86_64", target_feature = "sse2"))]
pub fn memrchr(x: u8, text: &[u8]) -> Option<usize> {
    use core::arch::x86_64::__m128i as Element;
    use core::arch::x86_64::_mm_set1_epi8 as BROADCAST;
    use core::arch::x86_64::_mm_cmpeq_epi8 as COMPARE;
    use core::arch::x86_64::_mm_load_si128 as LOAD_ALIGNED;
    use core::arch::x86_64:: _mm_movemask_epi8 as MOVEMASK;
   use core::num::NonZeroI32; // for smarter intrinsics
    const ALIGNMENT:usize=align_of::<Element>();
    const CHUNK_SIZE: usize = 4*ALIGNMENT;  // Process 4x aligned chunks at a time
    
    // Scan for a single byte value from the end usingiIntrinsics.
    // Split `text` in three parts:
    // - unaligned tail, after the last aligned address
    // - body, scanned by 4x 'Alignment' chunks at a time (backwards)
    // - unaligned prefix at the start

    let len = text.len();
    let ptr = text.as_ptr();
    
    // Use align_to to get the prefix and suffix lengths
    let (min_aligned_offset, max_aligned_offset) = {
        // We call this just to obtain the length of the prefix and suffix.
        // In the middle we always process four `Elements` at once.
        // SAFETY: transmuting `[u8]` to a tuple of `Element` is safe 
        // except for size differences which are handled by `align_to`.
        let (prefix, _, suffix) = unsafe { text.align_to::<(Element,Element,Element,Element)>() };
        (prefix.len(), len - suffix.len())
    };
    
    let mut offset = max_aligned_offset;
    
    // Search the unaligned tail first
    // SAFETY: `offset` is computed as `len - suffix.len()`, so `offset <= len`.
    // Therefore the range `offset..` is a valid subslice of `text`.
    if let Some(index) = unsafe {
        text.get_unchecked(offset..)
            .iter()
            .rposition(|elt| *elt == x)
    } {
        return Some(offset + index);
    }
    
    // Now search the aligned body going backwards
    unsafe {
        let needle = BROADCAST(x.cast_signed());
        
        
        // Process 4*`ALIGMENT` suzed chunks  going backwards
        // offset is always aligned, so just testing `>` is sufficient and avoids possible overflow.
        while offset > min_aligned_offset {
            // SAFETY: offset starts at len - suffix.len(), as long as it is greater than
            // min_aligned_offset (prefix.len()) the remaining distance is at least CHUNK_SIZE.
            // The body is trivially aligned due to align_to, avoid the cost of unaligned reads
            let chunk_ptr = ptr.add(offset - CHUNK_SIZE).cast::<Element>();
            
            // Load 4x  `ALIGNMENTS` (equal to chunk size)
            let chunk0 = LOAD_ALIGNED(chunk_ptr);
            let chunk1 = LOAD_ALIGNED(chunk_ptr.add(1));
            let chunk2 = LOAD_ALIGNED(chunk_ptr.add(2));
            let chunk3 = LOAD_ALIGNED(chunk_ptr.add(3));
            
            // Compare each chunk with needle
            let cmp0 = COMPARE(chunk0, needle);
            let cmp1 = COMPARE(chunk1, needle);
            let cmp2 = COMPARE(chunk2, needle);
            let cmp3 = COMPARE(chunk3, needle);
            
            
     
            
            // Check each mask in reverse order (last match wins)
            // Check upper first for reverse search
            if let Some(vmask3) = NonZeroI32::new(MOVEMASK(cmp3)){
                let byte_pos = 31 - vmask3.leading_zeros() as usize;
                return Some(offset - CHUNK_SIZE + (3*ALIGNMENT) + byte_pos);
            }
            if let Some(vmask2) = NonZeroI32::new(MOVEMASK(cmp2)) {
                let byte_pos = 31 - vmask2.leading_zeros() as usize;
                return Some(offset - CHUNK_SIZE + (2*ALIGNMENT) + byte_pos);
            }
            if let Some(vmask1) = NonZeroI32::new(MOVEMASK(cmp1)) {
                let byte_pos = 31 - vmask1.leading_zeros() as usize;
                return Some(offset - CHUNK_SIZE + ALIGNMENT + byte_pos);
            }
            if let Some(vmask0) = NonZeroI32::new(MOVEMASK(cmp0)){
                let byte_pos = 31 - vmask0.leading_zeros() as usize;
                return Some(offset - CHUNK_SIZE + byte_pos);
            }
            
            offset -= CHUNK_SIZE;
        }
    }
    
    // Find the last match in the remaining prefix.
    // SAFETY: `offset` is monotonically decreased from `max_aligned_offset <= len`,
    // and the loop condition guarantees `offset >= min_aligned_offset >= 0`.
    // Thus `..offset` is always a valid range for `text`.
    unsafe {
        text.get_unchecked(..offset)
            .iter()
            .rposition(|elt| *elt == x)
    }
}

/// Returns the last index matching the byte `x` in `text`.
///
#[must_use]
#[cfg(not(all(target_arch = "x86_64", target_feature = "sse2")))]
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
