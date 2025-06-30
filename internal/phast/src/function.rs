use std::hint::black_box;

use ph::GetSize;

#[cfg(feature = "fxhash")]
pub type Hasher = seedable_hash::Seedable<fxhash::FxBuildHasher>;

#[cfg(not(feature = "fxhash"))]
pub type Hasher = seedable_hash::BuildDefaultSeededHasher;

pub trait OutputRange: GetSize {
    fn output_range(&self) -> usize;    
}

pub trait Function: OutputRange {
    fn get(&self, key: u64) -> usize;

    fn get_all(&self, keys: &[u64]) {
        for key in keys {
            black_box(self.get(*key));
        }
    }
}

pub trait PartialFunction: OutputRange {
    fn get(&self, key: u64) -> Option<usize>;

    fn get_all(&self, keys: &[u64]) {
        for key in keys {
            black_box(self.get(*key));
        }
    }
}

