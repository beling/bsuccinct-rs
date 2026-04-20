use ph::{phast::{Generic, Perfect, SeedChooser, SeedChooserCore}, seeds::SeedSize};
use crate::function::{Function, Hasher, OutputRange};

impl<SS: SeedSize, SCC: SeedChooserCore> OutputRange for Perfect<SS, SCC, Hasher> {
    #[inline(always)] fn output_range(&self) -> usize {
        self.output_range()
    }
}

impl<SS: SeedSize, SCC: SeedChooserCore> Function for Perfect<SS, SCC, Hasher> {
    #[inline(always)] fn get(&self, key: u64) -> usize {
        self.get(&key)
    }
}

pub fn perfect<SS: SeedSize, SC: SeedChooser>(keys: &[u64], params: ph::phast::Conf<SS, Generic>, threads_num: usize, seed_chooser: SC) -> Perfect<SS, SC::Core, Hasher>
{
    Perfect::with_slice_p_threads_hash_sc(keys, &params,
     threads_num, Hasher::default(), seed_chooser)
}