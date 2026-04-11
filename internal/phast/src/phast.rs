use ph::{phast::{Conf, Core, DefaultCompressedArray, SeedChooser, SeedChooserCore}, seeds::SeedSize};
use crate::function::{Function, Hasher, OutputRange};

impl<C: Core, SS: SeedSize, SCC: SeedChooserCore> OutputRange for ph::phast::Function<C, SS, SCC, DefaultCompressedArray, Hasher> {
    #[inline(always)] fn output_range(&self) -> usize {
        self.output_range()
    }
}

impl<C: Core, SS: SeedSize, SCC: SeedChooserCore> Function for ph::phast::Function<C, SS, SCC, DefaultCompressedArray, Hasher> {
    #[inline(always)] fn get(&self, key: u64) -> usize {
        self.get(&key)
    }
}

pub fn phast<P, SC>(keys: &[u64], params: P, threads_num: usize, seed_chooser: SC) -> ph::phast::Function<P::Core, P::SeedSize, SC::Core, DefaultCompressedArray, Hasher>
where P: Conf, SC: SeedChooser
{
    ph::phast::Function::with_slice_p_threads_hash_sc(keys, &params,
     threads_num, Hasher::default(), seed_chooser)
}


impl<C: Core, SS: SeedSize, SCC: SeedChooserCore> OutputRange for ph::phast::Function2<C, SS, SCC, DefaultCompressedArray, Hasher> {
    #[inline(always)] fn output_range(&self) -> usize {
        self.output_range()
    }
}

impl<C: Core, SS: SeedSize, SCC: SeedChooserCore> Function for ph::phast::Function2<C, SS, SCC, DefaultCompressedArray, Hasher> {
    #[inline(always)] fn get(&self, key: u64) -> usize {
        self.get(&key)
    }
}

pub fn phast2<P, SC>(keys: &[u64], params: P, threads_num: usize, seed_chooser: SC) -> ph::phast::Function2<P::Core, P::SeedSize, SC::Core, DefaultCompressedArray, Hasher>
where P: Conf, SC: SeedChooser
{
    ph::phast::Function2::with_slice_p_threads_hash_sc(keys, &params,
     threads_num, Hasher::default(), seed_chooser)
}