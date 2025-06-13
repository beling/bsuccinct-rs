use crate::{phast::{conf::Conf, cyclic::{GenericUsedValue, UsedValueMultiSetU8}, SeedChooser}, seeds::SeedSize};

/// Choose best seed without shift component.
#[derive(Clone, Copy)]
pub struct SeedOnlyK(pub(crate) u8);

#[inline(always)]
fn best_seed_k<SC: SeedChooser, SS: SeedSize>(k: u8, seed_chooser: SC, best_value: &mut usize, best_seed: &mut u16, used_values: &mut UsedValueMultiSetU8, keys: &[u64], conf: &Conf<SS>) {
    //assert!(keys.len() <= SMALL_BUCKET_LIMIT);  // seems to speeds up a bit
    //let mut values_used_by_seed = arrayvec::ArrayVec::<_, SMALL_BUCKET_LIMIT>::new(); // Vec::with_capacity(keys.len());
    let mut values_used_by_seed = Vec::with_capacity(keys.len());
    for seed in SC::FIRST_SEED..conf.seeds_num() {    // seed=0 is special = no seed,
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

    #[inline(always)] fn f<SS: SeedSize>(self, primary_code: u64, seed: u16, conf: &Conf<SS>) -> usize {
        conf.f(primary_code, seed)
    }

    #[inline(always)]
    fn best_seed<SS: SeedSize>(self, used_values: &mut Self::UsedValues, keys: &[u64], conf: &Conf<SS>) -> u16 {
        let mut best_seed = 0;
        let mut best_value = usize::MAX;
        best_seed_k(self.0, self, &mut best_value, &mut best_seed, used_values, keys, conf);
        if best_seed != 0 { // can assign seed to the bucket
            for key in keys {
                used_values.add(conf.f(*key, best_seed));
            }
        };
        best_seed
    }
}