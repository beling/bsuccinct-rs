use crate::phast::{conf::Conf, cyclic::{GenericUsedValue, UsedValueSet}, seed_chooser::{best_seed_big, best_seed_small, SMALL_BUCKET_LIMIT}, SeedChooser};

/// Choose best seed without shift component.
#[derive(Clone, Copy)]
pub struct Walzer(u16);

impl Walzer {
    #[inline(always)] fn subslice(&self, hash_code: u64) -> usize {
        (wymum_xor(wymum_xor(seed as u64 ^ 0xa076_1d64_78bd_642f, 0x1d8e_4e27_c47d_124f), key) as u16 & self.slice_len_minus_one) as usize
    }
}

impl SeedChooser for Walzer {
    type UsedValues = UsedValueSet;
    
    #[inline(always)] fn f(self, primary_code: u64, seed: u16, conf: &Conf) -> usize {
        conf.slice_begin(key) +
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