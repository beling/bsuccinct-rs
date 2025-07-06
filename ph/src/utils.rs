//! Utility functions.

use binout::{AsIs, Serializer};
use bitm::{ArrayWithRank101111, ceiling_div};
pub use seedable_hash::{map64_to_64, map32_to_32};

pub type ArrayWithRank = ArrayWithRank101111;

/// Reads `number_of_bits` bits, rounded up to multiple of 64, from `input`.
pub fn read_bits<R: std::io::Read + ?Sized>(input: &mut R, number_of_bits: usize) -> std::io::Result<Box<[u64]>> {
    AsIs::read_n(input, ceiling_div(number_of_bits, 64))
}

/// Checks if `phf` is valid partial (`None` results are ignored) perfect hash function. Panics if it is not.
pub fn verify_partial_phf<K: std::fmt::Display, G: Fn(&K)->Option<usize>>(expected_range: usize, keys: impl IntoIterator<Item=K>, phf: G) {
    use bitm::{BitVec, BitAccess};
    let mut seen_values = Box::with_zeroed_bits(expected_range);
    for key in keys {
        if let Some(v) = phf(&key) {
            assert!(v < expected_range, "f({key})={v} exceeds maximum value {}", expected_range-1);
            assert!(!seen_values.get_bit(v as usize), "f returned the same value {v} for {key} and another key");
            seen_values.set_bit(v);
        }
    }
}

/// Checks if `phf` is valid k-perfect hash function. Panics if it is not (also if `phf` returns `None` for any key).
pub fn verify_phf<K: std::fmt::Display, G: Fn(&K)->Option<usize>>(expected_range: usize, keys: impl IntoIterator<Item=K>, phf: G) {
    verify_partial_phf(expected_range, keys, |key| {
        let v = phf(key);
        assert!(v.is_some(), "f does not assign the value to the key {} which is in the input", key);
        v
    });
}

/// Checks if `kphf` is valid partial (`None` results are ignored) k-perfect hash function. Panics if it is not.
pub fn verify_partial_kphf<K: std::fmt::Display, G: Fn(&K)->Option<usize>>(k: u8, expected_range: usize, keys: impl IntoIterator<Item=K>, kphf: G) {
    if k == 1 { verify_partial_phf(expected_range, keys, kphf); return; }
    let mut seen_values = vec![0; expected_range];
    for key in keys {
        if let Some(v) = kphf(&key) {
            assert!(v < expected_range, "f({key})={v} exceeds maximum value {}", expected_range-1);
            assert!(seen_values[v as usize] < k, "f returned the same value {v} for {key} and {k} another keys");
            seen_values[v as usize] += 1;
        }
    }
}

/// Checks if `kphf` is valid partial k-perfect hash function. Panics if it is not (also if `phf` returns `None` for any key).
pub fn verify_kphf<K: std::fmt::Display, G: Fn(&K)->Option<usize>>(k: u8, expected_range: usize, keys: impl IntoIterator<Item=K>, kphf: G) {
    verify_partial_kphf(k, expected_range, keys, |key| {
        let v = kphf(key);
        assert!(v.is_some(), "f does not assign the value to the key {} which is in the input", key);
        v
    });
}


#[cfg(test)]
pub(crate) mod tests {
    use super::verify_phf;

    pub fn test_mphf<K: std::fmt::Display+Clone, G: Fn(&K)->Option<usize>>(mphf_keys: &[K], mphf: G) {
        verify_phf(mphf_keys.len(), mphf_keys.iter().cloned(), mphf);
    }

    pub fn test_mphf_u64<K: std::fmt::Display+Clone, G: Fn(&K)->Option<u64>>(mphf_keys: &[K], mphf: G) {
        test_mphf(mphf_keys, |k| mphf(k).map(|v| v as usize));
    }
}