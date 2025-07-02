use ph::{phast::{Params, Perfect, SeedChooser}, seeds::SeedSize};
use crate::function::{Function, Hasher, OutputRange};

impl<SS: SeedSize, SC: SeedChooser> OutputRange for Perfect<SS, SC, Hasher> {
    #[inline(always)] fn output_range(&self) -> usize {
        self.output_range()
    }
}

impl<SS: SeedSize, SC: SeedChooser> Function for Perfect<SS, SC, Hasher> {
    #[inline(always)] fn get(&self, key: u64) -> usize {
        self.get(&key)
    }
}

pub fn perfect<SS: SeedSize, SC: SeedChooser+Sync>(keys: &[u64], params: Params<SS>, threads_num: usize, seed_chooser: SC) -> Perfect<SS, SC, Hasher>
{
    Perfect::with_slice_p_threads_hash_sc(keys, &params,
     threads_num, Hasher::default(), seed_chooser)
}