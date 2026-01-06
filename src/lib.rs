pub mod memchr_new;
pub mod memchr_old;

pub mod num;


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

#[macro_export]
macro_rules! find_swar_index {
    // SWAR
    ($num:expr) => {{
        #[cfg(target_endian = "big")]
        {
            ($num.leading_zeros() >> 3) as usize
        }
        #[cfg(target_endian = "little")]
        {
            ($num.trailing_zeros() >> 3) as usize
        }
    }};
}

#[cfg(test)]
pub mod test;

