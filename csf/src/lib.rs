#![doc = include_str!("../README.md")]

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