use crate::num::repeat_u8;
use rand::rngs::StdRng;
use rand::{Rng, RngCore, SeedableRng, rng};
// A lot of these tests are simply *OVERKILL* however just remove/toggle a bit when done.

// simple toggleable tests
const DETERMINISTIC: bool = true;
const TEST_SIZE: usize = 10000;
const RANDOM_SEED: u64 = 4269;
const MAX_SIZED_STRING: usize = 20000;

const LO_USIZE: usize = repeat_u8(0x01);
const HI_USIZE: usize = repeat_u8(0x80);
const USIZE_BYTES: usize = size_of::<usize>();

pub fn generate_random_byte_strings(count: usize, deterministic: bool) -> Vec<Vec<u8>> {
    let mut rng: Box<dyn RngCore> = if deterministic {
        Box::new(StdRng::seed_from_u64(RANDOM_SEED))
    } else {
        Box::new(rng())
    };

    let mut strings = Vec::with_capacity(count);

    for _ in 0..count {
        // random strings with varying lengths from 0 to MAX SIZED STRING
        let length = rng.random_range(0..=MAX_SIZED_STRING);
        let bytes: Vec<u8> = (0..length).map(|_| rng.random()).collect();
        strings.push(bytes);
    }

    strings
}

pub fn generate_random_usize_byte_arrays(
    count: usize,
    deterministic: bool,
) -> Vec<[u8; size_of::<usize>()]> {
    let mut rng: Box<dyn RngCore> = if deterministic {
        Box::new(StdRng::seed_from_u64(RANDOM_SEED))
    } else {
        Box::new(rng())
    };

    let mut arrays = Vec::with_capacity(count);
    for _ in 0..count {
        let mut bytes = [0u8; size_of::<usize>()];
        rng.fill_bytes(&mut bytes);
        arrays.push(bytes);
    }

    arrays
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::{
        memchr_new::{
            contains_zero_byte_borrow_fix, find_first_nul, find_last_nul,
        },
        num::repeat_u8,
    };
    use core::num::NonZeroUsize;

    #[test]
    fn generate_permutations_bit_manip() {
        const VALUES: [u8; 3] = [0x00, 0x01, 0x80];
        let total = 3usize.pow(USIZE_BYTES as u32);

        for mut state in 0..total {
            let mut bytes = [0u8; USIZE_BYTES];
            for i in 0..USIZE_BYTES {
                let digit = state % 3;
                state /= 3;
                bytes[i] = VALUES[digit];
            }

            let expected_first = bytes.iter().position(|&b| b == 0);
            let expected_last = bytes.iter().rposition(|&b| b == 0);

            let word = usize::from_ne_bytes(bytes);

            // Recompute the borrow-fix mask using the same logic as
            // `contains_zero_byte_borrow_fix`.
            let mut classic = word.wrapping_sub(LO_USIZE) & (!word) & HI_USIZE;
            let expected_mask = if classic == 0 {
                None
            } else {
                classic &= !(word << 7);
                Some(unsafe { NonZeroUsize::new_unchecked(classic) })
            };

            let actual_mask = contains_zero_byte_borrow_fix(word);
            assert_eq!(
                expected_mask, actual_mask,
                "borrow-fix mask mismatch; bytes={bytes:?} word={word:#x}"
            );

            match (expected_first, expected_last, actual_mask) {
                (None, None, None) => {}
                (Some(first), Some(last), Some(mask)) => {
                    assert_eq!(
                        first,
                        find_first_nul(mask),
                        "first zero-byte index mismatch; bytes={bytes:?} word={word:#x}"
                    );
                    assert_eq!(
                        last,
                        find_last_nul(mask),
                        "last zero-byte index mismatch; bytes={bytes:?} word={word:#x}"
                    );

                    // Strong property: every 0x80 bit in the mask must correspond
                    // to a real 0x00 byte lane!
                    for i in 0..USIZE_BYTES {
                        let lane_bit = ((mask.get() >> (i * 8)) & 0x80) != 0;
                        if lane_bit {
                            assert_eq!(
                                0x00,
                                bytes[i],
                                "false-positive lane bit at index {i}; bytes={bytes:?} word={word:#x} mask={:#x}",
                                mask.get()
                            );
                        }
                    }
                }
                _ => {
                    panic!(
                        "expected/actual mismatch; bytes={bytes:?} word={word:#x} expected_first={expected_first:?} expected_last={expected_last:?} actual_mask={actual_mask:?}"
                    );
                }
            }
        }
    }

    fn test_memchr(search: u8, sl: &[u8]) {
        let memchrtest = crate::memchr_new::memchr(search, sl);
        let realans = sl.iter().position(|b| *b == search);
        assert!(
            memchrtest == realans,
            "test failed in memchr: expected {realans:?}, got {memchrtest:?} for byte {search:#04x}\n
            searching for {} with ASCII value {search} in slice {}",
            char::from_u32(search as _).unwrap(),String::from_utf8_lossy(sl)
        );
    }

    fn test_memrchr(search: u8, sl: &[u8]) {
        let realans = sl.iter().rposition(|b| *b == search);
        let memrchrtest = crate::memchr_new::memrchr(search, sl);
        assert!(
            memrchrtest == realans,
            "test failed in memrchr: expected {realans:?}, got {memrchrtest:?} for byte {search:#04x}\n
            searching for {} with ASCII value {search} in slice {}",
            char::from_u32(search as _).unwrap(),String::from_utf8_lossy(sl)
        );
    }

    #[test]
    fn tmemchr() {
        let byte_strings = generate_random_byte_strings(TEST_SIZE, DETERMINISTIC);
        let random_chars = 0..=u8::MAX;

        for byte in random_chars {
            for string in &byte_strings {
                test_memchr(byte, string);
            }
        }
    }

    #[test]
    fn tmemrchr() {
        let byte_strings = generate_random_byte_strings(TEST_SIZE, DETERMINISTIC);
        let random_chars = 0..=u8::MAX;

        for byte in random_chars {
            for string in &byte_strings {
                test_memrchr(byte, string);
            }
        }
    }

    #[test]
    fn test_reversed() {
        let arrays = generate_random_usize_byte_arrays(TEST_SIZE, DETERMINISTIC);

        for bytes in arrays.iter() {
            for i in 0..=u8::MAX {
                let word = usize::from_ne_bytes(*bytes) ^ repeat_u8(i);

                let expected_pos = bytes.iter().rposition(|&b| b == i);
                #[cfg(target_endian = "little")]
                let detected_pos =
                    crate::memchr_new::contains_zero_byte_borrow_fix(word).map(find_last_nul);
                #[cfg(target_endian = "big")]
                let detected_pos = crate::memchr_new::contains_zero_byte(word).map(find_last_nul);

                assert_eq!(
                    detected_pos, expected_pos,
                    "Mismatch for word={word:#018x} bytes={bytes:?} in contains last zero byte!"
                );
            }
        }
    }

    #[test]
    fn test_forward() {
        let arrays = generate_random_usize_byte_arrays(TEST_SIZE, DETERMINISTIC);

        for bytes in arrays.iter() {
            for i in 0..=u8::MAX {
                let word = usize::from_ne_bytes(*bytes) ^ repeat_u8(i);

                let expected_pos = bytes.iter().position(|&b| b == i);
                #[cfg(target_endian = "little")]
                let detected_pos = crate::memchr_new::contains_zero_byte(word).map(find_first_nul);
                #[cfg(target_endian = "big")]
                let detected_pos =
                    crate::memchr_new::contains_zero_byte_borrow_fix(word).map(find_first_nul);

                assert_eq!(
                    detected_pos, expected_pos,
                    "Mismatch for word={word:#018x} bytes={bytes:?} in contains zero byte!"
                );
            }
        }
    }
}
