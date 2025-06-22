use ph::{fmph::SeedSize, phast::{DefaultCompressedArray, Perfect, SeedChooser}, GetSize};

pub type Hasher = ph::Seedable<fxhash::FxBuildHasher>;

pub trait Function: GetSize {
    fn get(&self, key: u64) -> Option<usize>;

    fn minimal_output_range(&self, keys_num: usize) -> usize;

    fn output_range(&self) -> usize;
}

impl<SS: SeedSize, SC: SeedChooser> Function for Perfect<SS, SC, Hasher> {
    #[inline(always)] fn get(&self, key: u64) -> Option<usize> {
        Some(self.get(&key))
    }

    #[inline(always)] fn minimal_output_range(&self, keys_num: usize) -> usize {
        self.minimal_output_range(keys_num)
    }

    #[inline(always)] fn output_range(&self) -> usize {
        self.output_range()
    }
}

pub fn perfect<SS: SeedSize, SC: SeedChooser+Sync>(keys: &[u64], bucket_size_100: u16, threads_num: usize, seed_size: SS, seed_chooser: SC) -> Perfect<SS, SC, Hasher>
{
    Perfect::with_slice_bps_bs_threads_hash_sc(keys, seed_size,
     bucket_size_100,
     threads_num, Hasher::default(), seed_chooser)
}

impl<SS: SeedSize, SC: SeedChooser> Function for ph::phast::Function<SS, SC, DefaultCompressedArray, Hasher> {
    #[inline(always)] fn get(&self, key: u64) -> Option<usize> {
        Some(self.get(&key))
    }

    #[inline(always)] fn minimal_output_range(&self, keys_num: usize) -> usize {
        self.minimal_output_range(keys_num)
    }

    #[inline(always)] fn output_range(&self) -> usize {
        self.output_range()
    }
}

pub fn phast<SS: SeedSize, SC: SeedChooser+Sync>(keys: &[u64], bucket_size_100: u16, threads_num: usize, seed_size: SS, seed_chooser: SC) -> ph::phast::Function<SS, SC, DefaultCompressedArray, Hasher>
{
    ph::phast::Function::with_slice_bps_bs_threads_hash_sc(keys, seed_size,
     bucket_size_100,
     threads_num, Hasher::default(), seed_chooser)
}