//! Utility functions.

use binout::{AsIs, Serializer};
use bitm::{ArrayWithRank101111, ceiling_div};
pub use seedable_hash::{map64_to_64, map32_to_32};

pub type ArrayWithRank = ArrayWithRank101111;

/// Reads `number_of_bits` bits, rounded up to multiple of 64, from `input`.
pub fn read_bits<R: std::io::Read + ?Sized>(input: &mut R, number_of_bits: usize) -> std::io::Result<Box<[u64]>> {
    AsIs::read_n(input, ceiling_div(number_of_bits, 64))
}


#[cfg(test)]
pub(crate) mod tests {
    pub fn test_phf<K: std::fmt::Display, G: Fn(&K)->Option<u64>>(expected_range: usize, keys: impl IntoIterator<Item=K>, phf: G) {
        use bitm::{BitVec, BitAccess};
        let mut seen_values = Box::with_zeroed_bits(expected_range);
        for key in keys {
            let v = phf(&key);
            assert!(v.is_some(), "f does not assign the value to the key {} which is in the input", key);
            let v = v.unwrap() as usize;
            assert!(v < expected_range, "f({key})={v} exceeds maximum value {}", expected_range-1);
            assert!(!seen_values.get_bit(v as usize), "f returned the same value {v} for {key} and another key");
            seen_values.set_bit(v);
        }
    }

    pub fn test_mphf<K: std::fmt::Display+Clone, G: Fn(&K)->Option<u64>>(mphf_keys: &[K], mphf: G) {
        test_phf(mphf_keys.len(), mphf_keys.iter().cloned(), mphf);
    }

}