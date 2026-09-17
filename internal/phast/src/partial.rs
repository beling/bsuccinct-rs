use ph::{phast::{Core, CoreConf, Partial, PartialML, SeedChooserConf, SeedChooserCore}, seeds::SeedSize};
use seedable_hash::{BuildDefaultSeededHasher, BuildSeededHasher};
use crate::function::{FunctionProperties, PartialFunction};

impl<C: Core, SS: SeedSize, SCC: SeedChooserCore> FunctionProperties for Partial<C, SS, SCC, ()> {
    #[inline(always)] fn output_range(&self) -> usize {
        self.output_range()
    }
    
    fn levels(&self) -> usize {
        1
    }
}

impl<C: Core, SS: SeedSize, SCC: SeedChooserCore> PartialFunction for Partial<C, SS, SCC, ()> {
    #[inline(always)] fn get(&self, key: u64) -> Option<usize> {
        self.get_for_hash(key)
    }
}

pub fn partial<SS: SeedSize, CC: CoreConf, SC: SeedChooserConf>(keys: &[u64], conf: ph::phast::Conf<SS, CC>, threads_num: usize, seed_chooser: SC) -> Partial<CC::Core, SS, SC::Core, ()>
{
    Partial::with_hashes_conf_threads_sc(keys.to_owned().as_mut_slice(), &conf, threads_num, seed_chooser)
}

impl<C: Core, SS: SeedSize, SCC: SeedChooserCore, S> FunctionProperties for PartialML<C, SS, SCC, S> {
    #[inline(always)] fn output_range(&self) -> usize {
        self.output_range()
    }

    fn levels(&self) -> usize {
        self.levels()
    }
}

impl<C: Core, SS: SeedSize, SCC: SeedChooserCore, S: BuildSeededHasher> PartialFunction for PartialML<C, SS, SCC, S> {
    #[inline(always)] fn get(&self, key: u64) -> Option<usize> {
        self.get(&key)
    }
}

pub fn partialml<SS: SeedSize, CC: CoreConf, SC: SeedChooserConf>(keys: &[u64], conf: ph::phast::Conf<SS, CC>, threads_num: usize, seed_chooser: SC) -> PartialML<CC::Core, SS, SC::Core, BuildDefaultSeededHasher>
{
    PartialML::with_slice_conf_threads_sc_u(keys, conf, threads_num, seed_chooser).0
}