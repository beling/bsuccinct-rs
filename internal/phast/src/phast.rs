use ph::{phast::{stats::BuildProgressRaport, DefaultCompressedArray, Params, SeedChooser}, seeds::SeedSize};
use crate::function::{Function, Hasher, OutputRange};

impl<SS: SeedSize, SC: SeedChooser> OutputRange for ph::phast::Function<SS, SC, DefaultCompressedArray, Hasher> {
    #[inline(always)] fn output_range(&self) -> usize {
        self.output_range()
    }
}

impl<SS: SeedSize, SC: SeedChooser> Function for ph::phast::Function<SS, SC, DefaultCompressedArray, Hasher> {
    #[inline(always)] fn get(&self, key: u64) -> usize {
        self.get(&key)
    }
}

pub fn phast<SS: SeedSize, SC: SeedChooser+Sync>(keys: &[u64], params: Params<SS>, threads_num: usize, seed_chooser: SC) -> ph::phast::Function<SS, SC, DefaultCompressedArray, Hasher>
{
    ph::phast::Function::with_slice_p_threads_hash_sc(keys, &params,
     threads_num, Hasher::default(), seed_chooser, &mut BuildProgressRaport::default())
}


impl<SS: SeedSize, SC: SeedChooser> OutputRange for ph::phast::Function2<SS, SC, DefaultCompressedArray, Hasher> {
    #[inline(always)] fn output_range(&self) -> usize {
        self.output_range()
    }
}

impl<SS: SeedSize, SC: SeedChooser> Function for ph::phast::Function2<SS, SC, DefaultCompressedArray, Hasher> {
    #[inline(always)] fn get(&self, key: u64) -> usize {
        self.get(&key)
    }
}

pub fn phast2<SS: SeedSize, SC: SeedChooser+Sync>(keys: &[u64], params: Params<SS>, threads_num: usize, seed_chooser: SC) -> ph::phast::Function2<SS, SC, DefaultCompressedArray, Hasher>
{
    ph::phast::Function2::with_slice_p_threads_hash_sc(keys, &params,
     threads_num, Hasher::default(), seed_chooser, &mut BuildProgressRaport::default())
}