use bitm::ceiling_div;

use crate::phast::{conf::Conf, cyclic::{GenericUsedValue, UsedValueMultiSetU8}};
use super::SeedChooser;

/// [`SeedChooser`] to build `k`-perfect functions.
/// `k` is given as a parameter of this chooser.
/// 
/// Should be used with [`Perfect`].
/// 
/// It chooses best seed with quite strong hasher, without shift component,
/// which should lead to quite small size, but long construction time.
#[derive(Clone, Copy)]
pub struct SeedOnlyK(pub u8);

#[inline(always)]
fn best_seed_k<SC: SeedChooser>(k: u8, seed_chooser: SC, best_value: &mut usize, best_seed: &mut u16, used_values: &mut UsedValueMultiSetU8, keys: &[u64], conf: &Conf, seeds_num: u16) {
    //assert!(keys.len() <= SMALL_BUCKET_LIMIT);  // seems to speeds up a bit
    //let mut values_used_by_seed = arrayvec::ArrayVec::<_, SMALL_BUCKET_LIMIT>::new(); // Vec::with_capacity(keys.len());
    let mut values_used_by_seed = Vec::with_capacity(keys.len());
    for seed in SC::FIRST_SEED..seeds_num {    // seed=0 is special = no seed,
        values_used_by_seed.clear();
        for key in keys.iter().copied() {
            let value = seed_chooser.f(key, seed, conf);
            if used_values[value] == k { break; }
            used_values.add(value);
            values_used_by_seed.push(value);
        }
        if values_used_by_seed.len() == keys.len() {
            let seed_value = values_used_by_seed.iter().sum();
            if seed_value < *best_value {
                *best_value = seed_value;
                *best_seed = seed;
            }
        }
        for v in &values_used_by_seed {
            used_values[*v] -= 1;
        }
    }
}

impl SeedChooser for SeedOnlyK {
    type UsedValues = UsedValueMultiSetU8;

    /// Returns maximum number of keys mapped to each output value; `k` of `k`-perfect function.
    #[inline(always)] fn k(self) -> u8 { self.0 }

    /// Returns output range of minimal (perfect or k-perfect) function for given number of keys.
    #[inline(always)] fn minimal_output_range(self, num_of_keys: usize) -> usize { ceiling_div(num_of_keys, self.k() as usize) }

    #[inline(always)] fn f(self, primary_code: u64, seed: u16, conf: &Conf) -> usize {
        conf.f(primary_code, seed)
    }

    #[inline(always)]
    fn best_seed(self, used_values: &mut Self::UsedValues, keys: &[u64], conf: &Conf, bits_per_seed: u8) -> u16 {
        let mut best_seed = 0;
        let mut best_value = usize::MAX;
        best_seed_k(self.0, self, &mut best_value, &mut best_seed, used_values, keys, conf, 1<<bits_per_seed);
        if best_seed != 0 { // can assign seed to the bucket
            for key in keys {               
                used_values.add(conf.f(*key, best_seed));
            }
        };
        best_seed
    }
}