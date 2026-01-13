use rand::rngs::StdRng;
use rand::{Rng, RngCore, SeedableRng, rng};

// A lot of these tests are simply *OVERKILL* however just remove/toggle a bit when done.

// simple toggleable tests
const DETERMINISTIC: bool = true;
const TEST_SIZE: usize = 10000;
const RANDOM_SEED: u64 = 4269;
const MAX_SIZED_STRING: usize = 20000;
const USIZE_BYTES: usize = size_of::<usize>();
use core::num::NonZeroUsize;

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

pub type UsizeByteArray = [u8; size_of::<usize>()];

pub fn generate_random_usize_byte_arrays(count: usize, deterministic: bool) -> Vec<UsizeByteArray> {
    let mut rng: Box<dyn RngCore> = if deterministic {
        Box::new(StdRng::seed_from_u64(RANDOM_SEED))
    } else {
        Box::new(rng())
    };

    let mut arrays = Vec::with_capacity(count);
    for _ in 0..count {
        let mut bytes: UsizeByteArray = [0u8; size_of::<usize>()];
        rng.fill_bytes(&mut bytes);
        arrays.push(bytes);
    }

    arrays
}

const fn find_last_zero_byte(num: NonZeroUsize) -> usize {
    #[cfg(target_endian = "little")]
    {
        USIZE_BYTES - 1 - ((num.leading_zeros() >> 3) as usize)
    }

    #[cfg(target_endian = "big")]
    {
        USIZE_BYTES - 1 - ((num.trailing_zeros() >> 3) as usize)
    }
}
#[cfg(test)]
mod tests {

    use crate::memchr_new::contains_zero_byte_reversed;

    use super::*;

    fn test_memchr(search: u8, sl: &[u8]) {
        let memchrtest = crate::memchr_new::memchr(search, sl);
        let realans = sl.iter().position(|b| *b == search);
        assert!(
            memchrtest == realans,
            "test failed in memchr: expected {realans:?}, got {memchrtest:?} for byte {search:#04x}"
        );
    }

    fn test_memrchr(search: u8, sl: &[u8]) {
        let realans = sl.iter().rposition(|b| *b == search);
        let memrchrtest = crate::memchr_new::memrchr(search, sl);
        assert!(
            memrchrtest == realans,
            "test failed in memrchr: expected {realans:?}, got {memrchrtest:?} for byte {search:#04x}"
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
            let word = usize::from_ne_bytes(*bytes);

            let expected_pos = bytes.iter().rposition(|&b| b == 0);
            let detected_pos = contains_zero_byte_reversed(word).map(find_last_zero_byte);

            assert_eq!(
                detected_pos, expected_pos,
                "Mismatch for word={word:#018x} bytes={bytes:?}"
            );
        }
    }
}
