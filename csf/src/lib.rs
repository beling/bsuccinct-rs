#![doc = include_str!("../README.md")]

pub mod coding;

pub mod fp;
pub mod ls;

pub use dyn_size_of::GetSize;

/// Calculates the minimal number of bits needed to store values from `0` to given `max_value`.
///
/// # Example
///
/// ```
/// use csf::bits_to_store;
///
/// assert_eq!(bits_to_store(0u8), 0);
/// assert_eq!(bits_to_store(1u16), 1);
/// assert_eq!(bits_to_store(7u32), 3);
/// assert_eq!(bits_to_store(8u64), 4);
/// ```
#[inline] pub fn bits_to_store<V: Into<u64>>(max_value: V) -> u8 {
    let max_value: u64 = max_value.into();
    (if max_value.is_power_of_two() {
        max_value.trailing_zeros()+1
    } else {
        max_value.checked_next_power_of_two().unwrap_or(0).trailing_zeros()
    }) as u8
}

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
    fn test_bits_to_store() {
        assert_eq!(bits_to_store(0u32), 0);
        assert_eq!(bits_to_store(1u32), 1);
        assert_eq!(bits_to_store(2u32), 2);
        assert_eq!(bits_to_store(3u32), 2);
        assert_eq!(bits_to_store(4u32), 3);
        assert_eq!(bits_to_store(7u32), 3);
        assert_eq!(bits_to_store(8u32), 4);
        assert_eq!(bits_to_store(u32::MAX-1), 32);
        assert_eq!(bits_to_store(u32::MAX), 32);
        assert_eq!(bits_to_store(u64::MAX), 64);
    }

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