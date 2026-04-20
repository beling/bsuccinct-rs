use ph::{phast::{CoreConf, Core, Partial, SeedChooser, SeedChooserCore}, seeds::SeedSize};
use crate::function::{OutputRange, PartialFunction};

impl<C: Core, SS: SeedSize, SCC: SeedChooserCore> OutputRange for Partial<C, SS, SCC, ()> {
    #[inline(always)] fn output_range(&self) -> usize {
        self.output_range()
    }
}

impl<C: Core, SS: SeedSize, SCC: SeedChooserCore> PartialFunction for Partial<C, SS, SCC, ()> {
    #[inline(always)] fn get(&self, key: u64) -> Option<usize> {
        self.get_for_hash(key)
    }
}

pub fn partial<SS: SeedSize, CC: CoreConf, SC: SeedChooser>(keys: &[u64], params: ph::phast::Conf<SS, CC>, threads_num: usize, seed_chooser: SC) -> Partial<CC::Core, SS, SC::Core, ()>
{
    Partial::with_hashes_p_threads_sc(keys.to_owned().as_mut_slice(), &params,
     threads_num, seed_chooser)
}