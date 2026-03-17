use ph::{phast::{ConfTrait, DefaultCompressedArray, ParamsTrait, SeedChooser}, seeds::SeedSize};
use crate::function::{Function, Hasher, OutputRange};

impl<C: ConfTrait, SS: SeedSize, SC: SeedChooser> OutputRange for ph::phast::Function<C, SS, SC, DefaultCompressedArray, Hasher> {
    #[inline(always)] fn output_range(&self) -> usize {
        self.output_range()
    }
}

impl<C: ConfTrait, SS: SeedSize, SC: SeedChooser> Function for ph::phast::Function<C, SS, SC, DefaultCompressedArray, Hasher> {
    #[inline(always)] fn get(&self, key: u64) -> usize {
        self.get(&key)
    }
}

pub fn phast<P, SC>(keys: &[u64], params: P, threads_num: usize, seed_chooser: SC) -> ph::phast::Function<P::Conf, P::SeedSize, SC, DefaultCompressedArray, Hasher>
where P: ParamsTrait, SC: SeedChooser
{
    ph::phast::Function::with_slice_p_threads_hash_sc(keys, &params,
     threads_num, Hasher::default(), seed_chooser)
}


impl<C: ConfTrait, SS: SeedSize, SC: SeedChooser> OutputRange for ph::phast::Function2<C, SS, SC, DefaultCompressedArray, Hasher> {
    #[inline(always)] fn output_range(&self) -> usize {
        self.output_range()
    }
}

impl<C: ConfTrait, SS: SeedSize, SC: SeedChooser> Function for ph::phast::Function2<C, SS, SC, DefaultCompressedArray, Hasher> {
    #[inline(always)] fn get(&self, key: u64) -> usize {
        self.get(&key)
    }
}

pub fn phast2<P, SC>(keys: &[u64], params: P, threads_num: usize, seed_chooser: SC) -> ph::phast::Function2<P::Conf, P::SeedSize, SC, DefaultCompressedArray, Hasher>
where P: ParamsTrait, SC: SeedChooser
{
    ph::phast::Function2::with_slice_p_threads_hash_sc(keys, &params,
     threads_num, Hasher::default(), seed_chooser)
}