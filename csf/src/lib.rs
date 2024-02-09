#![doc = include_str!("../README.md")]
//! # Space overhead
//! 
//! ## Distributions
//! 
//! The plots show the space overhead (over entropy of the distribution of function values) of the static functions
//! included in the `csf`, for two families of functions that differ in the distribution of values:
//! - Functions with ***equal*** distribution map *2<sup>20</sup> = ed+i* keys to *d+1* different values,
//!   where *d=1,...,255* and *i=1,...,e*.
//!   One value is assigned to *i* keys, and each of the other *d* values is assigned to *e* keys.
//!   The entropy of the distribution of function values is in the range *(log<sub>2</sub>(d), log<sub>2</sub>(d+1)]*
//!   and increases with both *d* and *i*.
//! - Functions with ***dominated*** (by a single value) distribution map *2<sup>20</sup>* keys to *256* different values.
//!   One value is assigned to *2<sup>20</sup>-255k* keys and each of the remaining *255* values is assigned to *k* keys,
//!   where *k=1,...,2<sup>20</sup>/256*. The entropy of the distribution of function values increases with *k*.
//! 
//! The data for the plots are generated with the [csf_benchmark](https://crates.io/crates/csf_benchmark) program.
//! 
//! ## Static functions
#![doc=include_str!("../plots/equal_abs.svg")]
#![doc=include_str!("../plots/equal_rel.svg")]
#![doc=include_str!("../plots/dominated_abs.svg")]
#![doc=include_str!("../plots/dominated_rel.svg")]

pub mod coding;

pub mod fp;
pub mod ls;

pub use dyn_size_of::GetSize;
pub use bitm::bits_to_store;

/// Calculates the minimal number of bits needed to store any of the given `values`.
/// 
/// # Example
///
/// ```
/// use csf::bits_to_store_any_of;
///
/// assert_eq!(bits_to_store_any_of([2u8, 7, 5, 7]), 3);
/// assert_eq!(bits_to_store_any_of([0u8]), 0);
/// assert_eq!(bits_to_store_any_of::<u32>([]), 0);
/// ```
pub fn bits_to_store_any_of<V: Into<u64>>(values: impl IntoIterator<Item = V>) -> u8 {
    values.into_iter().map(|v|Into::<u64>::into(v)).max().map_or(0, bits_to_store)
}

/// Calculates the minimal number of bits needed to store any of the given `values`.
#[inline] pub fn bits_to_store_any_of_ref<'a, V: Clone + Into<u64> + 'a>(values: impl IntoIterator<Item = &'a V>) -> u8 {
    bits_to_store_any_of(values.into_iter().cloned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bits_to_store_any_of() {
        assert_eq!(bits_to_store_any_of::<u32>([]), 0);
        assert_eq!(bits_to_store_any_of([0u8]), 0);
        assert_eq!(bits_to_store_any_of([0u8, 1]), 1);
        assert_eq!(bits_to_store_any_of([2u8, 7, 3]), 3);
        assert_eq!(bits_to_store_any_of([u64::MAX, 2, 67]), 64);

        assert_eq!(bits_to_store_any_of_ref::<u32>([].iter()), 0);
        assert_eq!(bits_to_store_any_of_ref([u64::MAX, 2, 67].iter()), 64);
    }
}