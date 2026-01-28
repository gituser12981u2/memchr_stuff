use crate::memchr_new::{
    USIZE_BYTES, contains_zero_byte, contains_zero_byte_borrow_fix, find_first_nul, find_last_nul,
    memchr_naive,
};

// For `rebar` tests in memchr

// Copy pasted into an amortised finder. I make no excuses for this code.

// simple self validation for myself
const _: () = {
    let mut i: u8 = 0;
    while i < u8::MAX {
        assert!(i == (usize::from_ne_bytes([i; size_of::<usize>()]) as u8));
        i += 1;
    }
};

pub struct Finder(usize);

impl Finder {
    pub const fn new(x: u8) -> Self {
        Self(crate::num::repeat_u8(x))
    }
    #[inline]
    pub fn find_first(&self, text: &[u8]) -> Option<usize> {
        let x = self.0 as u8;
        if text.len() < 2 * USIZE_BYTES {
            return memchr_naive(x, text);
        }

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
        let repeated_x = self.0;
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
    #[inline]
    pub fn find_last(&self, text: &[u8]) -> Option<usize> {
        let x = self.0 as u8;
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

        let repeated_x = self.0;

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
}
