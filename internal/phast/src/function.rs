use ph::GetSize;

pub type Hasher = ph::Seedable<fxhash::FxBuildHasher>;

pub trait OutputRange: GetSize {
    fn minimal_output_range(&self, keys_num: usize) -> usize;

    fn output_range(&self) -> usize;
    
}

pub trait Function: OutputRange {
    fn get(&self, key: u64) -> usize;
}

pub trait PartialFunction: OutputRange {
    fn get(&self, key: u64) -> Option<usize>;
}

