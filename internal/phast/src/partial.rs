use ph::{seeds::SeedSize, phast::{Partial, SeedChooser}};
use crate::function::{OutputRange, PartialFunction};

impl<SS: SeedSize, SC: SeedChooser> OutputRange for Partial<SS, SC, ()> {
    #[inline(always)] fn output_range(&self) -> usize {
        self.output_range()
    }
}

impl<SS: SeedSize, SC: SeedChooser> PartialFunction for Partial<SS, SC, ()> {
    #[inline(always)] fn get(&self, key: u64) -> Option<usize> {
        self.get_for_hash(key)
    }
}

pub fn partial<SS: SeedSize, SC: SeedChooser+Sync>(keys: &[u64], bucket_size_100: u16, threads_num: usize, seed_size: SS, seed_chooser: SC) -> Partial<SS, SC, ()>
{
    Partial::with_hashes_bps_bs_threads_sc(keys.to_owned().as_mut_slice(), seed_size,
     bucket_size_100,
     threads_num, seed_chooser)
}