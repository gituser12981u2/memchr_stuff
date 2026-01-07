#![allow(internal_features)]
#![feature(core_intrinsics, const_eval_select)]
#![allow(
    clippy::blanket_clippy_restriction_lints,
    clippy::host_endian_bytes,
    clippy::implicit_return,
    clippy::doc_markdown,
    clippy::single_call_fn,
    clippy::arithmetic_side_effects,
    clippy::min_ident_chars,
    clippy::indexing_slicing,
    clippy::cast_ptr_alignment,
    clippy::as_conversions,
    clippy::ptr_as_ptr,
    clippy::multiple_unsafe_ops_per_block,
    clippy::items_after_statements,
    clippy::missing_docs_in_private_items,
    clippy::default_numeric_fallback,
    clippy::absolute_paths,
    clippy::allow_attributes,
    clippy::allow_attributes_without_reason
)]

pub mod memchr_new;
pub mod memchr_old;
pub mod num;
#[macro_export]
macro_rules! find_swar_last_index {
    // SWAR
    ($num:expr) => {{
        #[cfg(target_endian = "big")]
        {
            (((usize::BITS - 1) - $num.trailing_zeros()) >> 3) as usize
        }
        #[cfg(target_endian = "little")]
        {
            (((usize::BITS - 1) - $num.leading_zeros()) >> 3) as usize
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
