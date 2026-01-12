use rand::rngs::StdRng;
use rand::{Rng, RngCore, SeedableRng, rng};
// simple toggleable tests
const DETERMINISTIC: bool = false;
const TEST_SIZE: usize = 10000;
const RANDOM_SEED: u64 = 4269;
const MAX_SIZED_STRING: usize = 20000;

pub fn generate_random_byte_strings(count: usize, deterministic: bool) -> Vec<Vec<u8>> {
    let mut rng: Box<dyn RngCore> = if deterministic {
        Box::new(StdRng::seed_from_u64(RANDOM_SEED))
    } else {
        Box::new(rng())
    };

    let mut strings = Vec::with_capacity(count);

    for _ in 0..count {
        // random strings with varying lengths from 10 to MAX SIZED STRING
        let length = rng.random_range(0..=MAX_SIZED_STRING);
        let bytes: Vec<u8> = (0..length).map(|_| rng.random()).collect();
        strings.push(bytes);
    }

    strings
}

#[cfg(test)]
mod tests {
    use super::DETERMINISTIC;
    use super::TEST_SIZE;
    use super::generate_random_byte_strings;

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
}
