#[cfg(not(target_pointer_width = "64"))]
compile_error!("etc not testing on anything except 64 bit");

use core::mem::size_of;
use std::num::NonZeroUsize;

#[inline]
const fn repeat_u8(x: u8) -> usize {
    usize::from_ne_bytes([x; size_of::<usize>()])
}

#[macro_export]
macro_rules! find_swar_last_index {
    // SWAR
    ($num:expr) => {{
        #[cfg(target_endian = "big")]
        {
            // `$num` has the high bit (0x80) set in each byte that matched.
            // On big-endian, the last byte index corresponds to the least-significant
            // set bit in the word.
            let tz = $num.trailing_zeros();
            (((usize::BITS - 1) - tz) / 8) as usize
        }
        #[cfg(target_endian = "little")]
        {
            // `$num` has the high bit (0x80) set in each byte that matched.
            // On little-endian, the last byte index corresponds to the most-significant
            // set bit in the word.
            let lz = $num.leading_zeros();
            (((usize::BITS - 1) - lz) / 8) as usize
        }
    }};
}
/*

the rightmost 0-byte.
Figure 6-2 shows a branch-free procedure for this function. The idea is to convert each 0-byte to 0x80,
and each nonzero byte to 0x00, and then use number of leading zeros. This procedure executes in
eight instructions if the machine has the number of leading zeros and nor instructions. Some similar
tricks are described in [Lamp].
Figure 6-2 Find leftmost 0-byte, branch-free code.
int zbytel(unsigned x) {
unsigned y;
int n;
// Original byte: 00 80 other
y = (x & 0x7F7F7F7F) + 0x7F7F7F7F; // 7F 7F 1xxxxxxx
y = ~(y | x | 0x7F7F7F7F); // 80 00 00000000
n = nlz(y) >> 3; // n = 0 ... 4, 4 if x
return n; // has no 0-byte.
}
The position of the rightmost 0-byte is given by the number of trailing 0's in the final value of y
computed above, divided by 8 (with fraction discarded). Using the expression for computing the
number of trailing 0's by means of the number of leading zeros instruction (see Section 5- 4, "Counting
Trailing 0's," on page 84), this can be computed by replacing the assignment to n in the procedure
above with:
n = (32 - nlz(~y & (y - 1))) >> 3

*/
#[inline]
pub const fn find_last_char_in_word(c: u8, input: usize) -> Option<usize> {
    let x = input ^ repeat_u8(c);
    let y = contains_zero_byte_reversed(x);

    match y {
        Some(num) => Some(find_swar_last_index!(num)),
        None => None,
    }
}

#[inline]
pub const fn contains_zero_byte_reversed(x: usize) -> Option<NonZeroUsize> {
    const MASK: usize = repeat_u8(0x7F);

    let y = (x & MASK).wrapping_add(MASK);
    return NonZeroUsize::new(!(y | x | MASK));
}

#[cfg(test)]
mod tests {
    use super::find_last_char_in_word;

    fn reference(c: u8, x: usize) -> Option<usize> {
        x.to_ne_bytes().iter().rposition(|b| *b == c)
    }

    #[test]
    fn exhaustive_match_masks_all_bytes() {
        for c in 0u8..=u8::MAX {
            let filler = c.wrapping_add(1); // guaranteed != c

            for mask in 0u16..=0xFF {
                let mut bytes = [filler; 8];
                for i in 0..8 {
                    if (mask & (1u16 << i)) != 0 {
                        bytes[i] = c;
                    }
                }

                let x = usize::from_ne_bytes(bytes);
                let got = find_last_char_in_word(c, x);
                let expected = bytes.iter().rposition(|b| *b == c);

                assert_eq!(
                    got, expected,
                    "c={c} mask=0x{mask:02x} bytes={bytes:?} x=0x{x:016x}"
                );
            }
        }
    }

    #[test]
    fn boundary_nonmatching_values() {
        const CS: &[u8] = &[0x00, 0x01, 0x7F, 0x80, 0xFE, 0xFF];
        const FILLS: &[u8] = &[0x00, 0x01, 0x7F, 0x80, 0xFE, 0xFF, 0x55, 0xAA];

        for &c in CS {
            for &fill in FILLS {
                if fill == c {
                    continue;
                }

                for mask in 0u16..=0xFF {
                    let mut bytes = [fill; 8];
                    for i in 0..8 {
                        if (mask & (1u16 << i)) != 0 {
                            bytes[i] = c;
                        }
                    }
                    let x = usize::from_ne_bytes(bytes);

                    let got = find_last_char_in_word(c, x);
                    let expected = reference(c, x);

                    assert_eq!(
                        got, expected,
                        "c={c} fill={fill} mask=0x{mask:02x} bytes={bytes:?} x=0x{x:016x}"
                    );
                }
            }
        }
    }
}

fn test_slice(search: u8, x: &[u8; 8]) {
    let as_usize = usize::from_ne_bytes(*x);
    let zero_pos = x.iter().rposition(|c| *c == search);

    let last_char = find_last_char_in_word(search, as_usize);
    if zero_pos != last_char {
        eprintln!("{:?}", last_char);
        let as_array = &[search];

        eprintln!(
            "error   approx pos is {last_char:?}  true pos is {zero_pos:?} for char '{}'\n\n",
            String::from_utf8_lossy(as_array)
        );
    }
}

fn main() {
    const BYTE_SLICE: &[u8; 8] = b"0174d58/";
    const TEST1: &[u8; 8] = b"236/fdin";
    const TEST2: &[u8; 8] = &[0, 0, 0, 0, 0, 0, 0, 25];

    test_slice(b'0', BYTE_SLICE);
    test_slice(b'/', BYTE_SLICE);
    test_slice(b'2', TEST1);
    test_slice(b'2', TEST2);
}
