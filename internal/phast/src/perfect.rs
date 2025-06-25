use ph::{seeds::SeedSize, phast::{Perfect, SeedChooser}};
use crate::function::{Function, Hasher, OutputRange};

impl<SS: SeedSize, SC: SeedChooser> OutputRange for Perfect<SS, SC, Hasher> {
    #[inline(always)] fn minimal_output_range(&self, keys_num: usize) -> usize {
        self.minimal_output_range(keys_num)
    }

    #[inline(always)] fn output_range(&self) -> usize {
        self.output_range()
    }
}

impl<SS: SeedSize, SC: SeedChooser> Function for Perfect<SS, SC, Hasher> {
    #[inline(always)] fn get(&self, key: u64) -> usize {
        self.get(&key)
    }
}

pub fn perfect<SS: SeedSize, SC: SeedChooser+Sync>(keys: &[u64], bucket_size_100: u16, threads_num: usize, seed_size: SS, seed_chooser: SC) -> Perfect<SS, SC, Hasher>
{
    Perfect::with_slice_bps_bs_threads_hash_sc(keys, seed_size,
     bucket_size_100,
     threads_num, Hasher::default(), seed_chooser)
}