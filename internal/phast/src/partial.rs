use ph::{phast::{ConfTrait, ParamsTrait, Partial, SeedChooser}, seeds::SeedSize};
use crate::function::{OutputRange, PartialFunction};

impl<C: ConfTrait, SS: SeedSize, SC: SeedChooser> OutputRange for Partial<C, SS, SC, ()> {
    #[inline(always)] fn output_range(&self) -> usize {
        self.output_range()
    }
}

impl<C: ConfTrait, SS: SeedSize, SC: SeedChooser> PartialFunction for Partial<C, SS, SC, ()> {
    #[inline(always)] fn get(&self, key: u64) -> Option<usize> {
        self.get_for_hash(key)
    }
}

pub fn partial<P: ParamsTrait, SC: SeedChooser>(keys: &[u64], params: P, threads_num: usize, seed_chooser: SC) -> Partial<P::Conf, P::SeedSize, SC, ()>
{
    Partial::with_hashes_p_threads_sc(keys.to_owned().as_mut_slice(), &params,
     threads_num, seed_chooser)
}