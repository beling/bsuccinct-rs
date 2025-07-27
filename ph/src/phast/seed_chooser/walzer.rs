use crate::phast::{conf::{mix, Conf}, cyclic::{GenericUsedValue, UsedValueSet}, seed_chooser::{best_seed_big, best_seed_small, SMALL_BUCKET_LIMIT}, SeedChooser};

/// Choose best seed without shift component.
#[derive(Clone, Copy)]
pub struct Walzer(pub u16);

#[inline(always)]
const fn mix64(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9u64);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111ebu64);
    x ^ (x >> 31)
}

impl Walzer {
    #[inline] pub fn new(subslice_len: u16) -> Self {
        Self(subslice_len - 1)
    }

    #[inline(always)] fn subslice_shift(&self, hash_code: u64, conf: &Conf) -> u16 {
        mix64(hash_code) as u16 & conf.slice_len_minus_one
    }

    #[inline(always)] fn in_subslice(&self, hash_code: u64, seed: u16) -> u16 {
        mix(mix(seed as u64 ^ 0xa076_1d64_78bd_642f, 0x1d8e_4e27_c47d_124f), hash_code) as u16 & self.0
    }
}

impl SeedChooser for Walzer {
    type UsedValues = UsedValueSet;

    #[inline(always)] fn extra_shift(self, _bits_per_seed: u8) -> u16 {
        self.0
    }
    
    #[inline(always)] fn f(self, hash_code: u64, seed: u16, conf: &Conf) -> usize {
        conf.slice_begin(hash_code) + (self.subslice_shift(hash_code, conf) + self.in_subslice(hash_code, seed)) as usize
    }

    /*#[inline(always)] fn f_slice(primary_code: u64, slice_begin: usize, seed: u16, conf: &Conf) -> usize {
        slice_begin + conf.in_slice(primary_code, seed)
    }*/

    #[inline(always)]
    fn best_seed(self, used_values: &mut Self::UsedValues, keys: &[u64], conf: &Conf, bits_per_seed: u8) -> u16 {
        let mut best_seed = 0;
        let mut best_value = usize::MAX;
        if keys.len() <= SMALL_BUCKET_LIMIT {
            best_seed_small(self, &mut best_value, &mut best_seed, used_values, keys, conf, 1<<bits_per_seed)
        } else {
            best_seed_big(self, &mut best_value, &mut best_seed, used_values, keys, conf, 1<<bits_per_seed)
        };
        if best_seed != 0 { // can assign seed to the bucket
            for key in keys {
                used_values.add(conf.f(*key, best_seed));
            }
        };
        best_seed
    }
}