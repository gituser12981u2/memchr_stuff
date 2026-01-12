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
        // random strings with varying lengths from 0 to MAX SIZED STRING
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

    #[test]
    #[cfg(target_pointer_width = "64")] // ill add 32bit if i feel like it
    fn test_contains_zero_byte_reversed() {
        use crate::memchr_new::contains_zero_byte_reversed;

        assert_eq!(contains_zero_byte_reversed(usize::MAX), None);
        assert_eq!(contains_zero_byte_reversed(0x0101010101010101usize), None);
        assert_eq!(contains_zero_byte_reversed(0x8080808080808080usize), None);
        assert_eq!(contains_zero_byte_reversed(0xFFFFFFFFFFFFFFFFusize), None);

        const USIZE_BYTES: usize = size_of::<usize>();

        // test zero byte in every possible position(surprisingly, this is rather quick!)
        for byte_pos in 0..USIZE_BYTES {
            let shift = byte_pos * 8;
            let word = !0usize ^ (0xFFusize << shift); // All 0xFF except one 0x00 byte
            let result = contains_zero_byte_reversed(word);
            assert!(
                result.is_some(),
                "Expected Some for zero byte at position {byte_pos}, word: {word:#018x}"
            );

            let mask = result.unwrap().get();
            // should have a bit set in the high bit of the zero byte position
            let expected_bit = 0x80usize << shift;
            assert!(
                mask & expected_bit != 0,
                "Expected bit at position {byte_pos} to be set, mask: {mask:#018x}, expected_bit: {expected_bit:#018x}"
            );
        }

        // should detect at least one
        assert!(contains_zero_byte_reversed(0x0000000000000000usize).is_some());
        assert!(contains_zero_byte_reversed(0x00FF00FF00FF00FFusize).is_some());
        assert!(contains_zero_byte_reversed(0xFF00FF00FF00FF00usize).is_some());

        // these test the borrow-safe mask logic
        assert!(contains_zero_byte_reversed(0x0001020304050607usize).is_some());
        assert!(contains_zero_byte_reversed(0x0706050403020100usize).is_some());
        assert!(contains_zero_byte_reversed(0xFF00FFFFFFFFFFFF).is_some());
        assert!(contains_zero_byte_reversed(0xFFFFFFFFFFFFFF00).is_some());

        // words with 0x80 but no zero bytes return None
        // (these could cause false positives in naive SWAR)
        assert_eq!(contains_zero_byte_reversed(0x8081828384858687usize), None);
        assert_eq!(contains_zero_byte_reversed(0xFF80FF80FF80FF80usize), None);
    }

    #[test]
    #[cfg(target_pointer_width = "64")] // ill add 32bit if i feel like it (this proves enough for now.)
    fn test_contains_zero_byte_reversed_with_iter() {
        use crate::memchr_new::contains_zero_byte_reversed;

        const USIZE_BYTES: usize = size_of::<usize>();

        // test that the mask returned by contains_zero_byte_reversed correctly
        // identifies the position of zero bytes when compared with iter().rposition()

        for byte_pos in 0..USIZE_BYTES {
            let shift = byte_pos * 8;
            let word = !0usize ^ (0xFFusize << shift); // All 0xFF except one 0x00 byte
            let word_bytes = word.to_ne_bytes();

            // find rightmost (last) zero byte using iter().rposition()
            let expected_pos = word_bytes.iter().rposition(|&b| b == 0);
            assert_eq!(
                expected_pos,
                Some(byte_pos),
                "iter().rposition() should find zero at byte_pos {byte_pos}"
            );

            // Verify contains_zero_byte_reversed detects it
            let result = contains_zero_byte_reversed(word);
            assert!(
                result.is_some(),
                "Should detect zero byte at position {byte_pos}"
            );

            // calculate position from the mask
            let mask = result.unwrap();
            #[cfg(target_endian = "little")]
            let detected_pos = USIZE_BYTES - 1 - (mask.leading_zeros() >> 3) as usize;
            #[cfg(target_endian = "big")]
            let detected_pos = USIZE_BYTES - 1 - (mask.trailing_zeros() >> 3) as usize;

            assert_eq!(
                detected_pos, byte_pos,
                "Position calculated from mask should match byte_pos {byte_pos}"
            );
        }

        //  multiple zero bytes - should find the rightmost one
        let test_cases = [
            (0x00FF00FFFFFFFFFFusize, "two zero bytes"),
            (0x0000FFFFFFFFFFFFusize, "adjacent zeros at start"),
            (0xFFFFFFFFFFFF0000usize, "adjacent zeros at end"),
            (0x00FF00FF00FF00FFusize, "alternating zeros"),
            (0xFF00FF00FF00FF00usize, "alternating zeros offset"),
        ];

        for (word, desc) in test_cases {
            let word_bytes = word.to_ne_bytes();
            let expected_pos = word_bytes.iter().rposition(|&b| b == 0);

            if expected_pos.is_none() {
                continue;
            }

            let result = contains_zero_byte_reversed(word);
            assert!(result.is_some(), "Should detect zero bytes in {desc}");

            let mask = result.unwrap();
            #[cfg(target_endian = "little")]
            let detected_pos = USIZE_BYTES - 1 - (mask.leading_zeros() >> 3) as usize;
            #[cfg(target_endian = "big")]
            let detected_pos = USIZE_BYTES - 1 - (mask.trailing_zeros() >> 3) as usize;

            assert_eq!(
                detected_pos,
                expected_pos.unwrap(),
                "Should find rightmost zero byte in {desc}: word={word:#018x}"
            );
        }

        let all_zeros = 0x0000000000000000usize;
        let all_zeros_bytes = all_zeros.to_ne_bytes();
        let expected_pos = all_zeros_bytes.iter().rposition(|&b| b == 0);
        assert_eq!(
            expected_pos,
            Some(USIZE_BYTES - 1),
            "zeros word should have rightmost zero at last position"
        );

        let result = contains_zero_byte_reversed(all_zeros);
        assert!(
            result.is_some(),
            "Should detect zero bytes in all-zeros word"
        );
    }
}
