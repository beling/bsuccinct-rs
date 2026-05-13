use ph::{phast::{CoreConf, Core, DefaultCompressedArray, SeedChooser, SeedChooserCore}, seeds::SeedSize};
use crate::function::{Function, Hasher, FunctionProperties};

impl<C: Core, SS: SeedSize, SCC: SeedChooserCore> FunctionProperties for ph::phast::Function<C, SS, SCC, DefaultCompressedArray, Hasher> {
    #[inline(always)] fn output_range(&self) -> usize {
        self.output_range()
    }
    
    fn levels(&self) -> usize {
        self.levels()
    }
}

impl<C: Core, SS: SeedSize, SCC: SeedChooserCore> Function for ph::phast::Function<C, SS, SCC, DefaultCompressedArray, Hasher> {
    #[inline(always)] fn get(&self, key: u64) -> usize {
        self.get(&key)
    }
}

pub fn phast<SS, CC, SC>(keys: &[u64], params: ph::phast::Conf<SS, CC, Hasher>, threads_num: usize, seed_chooser: SC) -> ph::phast::Function<CC::Core, SS, SC::Core, DefaultCompressedArray, Hasher>
where SS: SeedSize, CC: CoreConf, SC: SeedChooser
{
    ph::phast::Function::with_slice_conf_threads_sc(keys, params, threads_num, seed_chooser)
}



impl<C: Core, SS: SeedSize, SCC: SeedChooserCore> FunctionProperties for ph::phast::Function2<C, SS, SCC, DefaultCompressedArray, Hasher> {
    #[inline(always)] fn output_range(&self) -> usize {
        self.output_range()
    }
    
    fn levels(&self) -> usize {
        self.levels()
    }
}

impl<C: Core, SS: SeedSize, SCC: SeedChooserCore> Function for ph::phast::Function2<C, SS, SCC, DefaultCompressedArray, Hasher> {
    #[inline(always)] fn get(&self, key: u64) -> usize {
        self.get(&key)
    }
}

pub fn phast2<SS, CC, SC>(keys: &[u64], params: ph::phast::Conf<SS, CC>, threads_num: usize, seed_chooser: SC) -> ph::phast::Function2<CC::Core, SS, SC::Core, DefaultCompressedArray, Hasher>
where SS: SeedSize, CC: CoreConf, SC: SeedChooser
{
    ph::phast::Function2::with_slice_conf_threads_sc(keys, params, threads_num, seed_chooser)
}




impl<C: Core, SS: SeedSize, SCC: SeedChooserCore> FunctionProperties for ph::phast::KFunction<C, SS, SCC, DefaultCompressedArray, Hasher> {
    #[inline(always)] fn output_range(&self) -> usize {
        self.output_range()
    }
    
    fn levels(&self) -> usize {
        self.levels()
    }
}

impl<C: Core, SS: SeedSize, SCC: SeedChooserCore> Function for ph::phast::KFunction<C, SS, SCC, DefaultCompressedArray, Hasher> {
    #[inline(always)] fn get(&self, key: u64) -> usize {
        self.get(&key)
    }
}

pub fn kphast<SS, CC, SC>(keys: &[u64], params: ph::phast::Conf<SS, CC, Hasher>, threads_num: usize, seed_chooser: SC) -> ph::phast::KFunction<CC::Core, SS, SC::Core, DefaultCompressedArray, Hasher>
where SS: SeedSize, CC: CoreConf, SC: SeedChooser
{
    ph::phast::KFunction::with_slice_conf_threads_sc(keys, params, threads_num, seed_chooser)
}




impl<C: Core, SS: SeedSize> FunctionProperties for ph::phast::NBFunction<C, SS, Hasher> {
    #[inline(always)] fn output_range(&self) -> usize {
        self.output_range()
    }
    
    fn levels(&self) -> usize {
        self.seed() as usize + 1
    }
}

impl<C: Core, SS: SeedSize> Function for ph::phast::NBFunction<C, SS, Hasher> {
    #[inline(always)] fn get(&self, key: u64) -> usize {
        self.get(&key)
    }
}

pub fn nbphast<SS, CC>(keys: &[u64], params: ph::phast::Conf<SS, CC, Hasher>, threads_num: usize) -> ph::phast::NBFunction<CC::Core, SS, Hasher>
where SS: SeedSize, CC: CoreConf
{
    ph::phast::NBFunction::with_slice_conf_threads(keys, params, threads_num)
}