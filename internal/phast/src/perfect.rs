use ph::{phast::{Core, CoreConf, Perfect, SeedChooser, SeedChooserCore}, seeds::SeedSize};
use crate::function::{Function, Hasher, OutputRange};

impl<C: Core, SS: SeedSize, SCC: SeedChooserCore> OutputRange for Perfect<C, SS, SCC, Hasher> {
    #[inline(always)] fn output_range(&self) -> usize {
        self.output_range()
    }
}

impl<C: Core, SS: SeedSize, SCC: SeedChooserCore> Function for Perfect<C, SS, SCC, Hasher> {
    #[inline(always)] fn get(&self, key: u64) -> usize {
        self.get(&key)
    }
}

pub fn perfect<CC: CoreConf, SS: SeedSize, SC: SeedChooser>(keys: &[u64], conf: ph::phast::Conf<SS, CC, Hasher>, threads_num: usize, seed_chooser: SC)
     -> Perfect<CC::Core, SS, SC::Core, Hasher>
{
    Perfect::with_slice_conf_threads_sc(keys, conf, threads_num, seed_chooser)
}