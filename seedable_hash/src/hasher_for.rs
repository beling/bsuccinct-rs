//use std::borrow::Borrow;

pub trait HasherFor<K: ?Sized> {
    //fn hash<Q>(&self, key: &Q, seed: u64) -> u64 where K: Borrow<Q>;
    fn hash(&self, key: &K, seed: u64) -> u64;
}

pub trait ByteHasher {
    fn hash_bytes(&self, bytes: &[u8], seed: u64) -> u64;
}

macro_rules! impl_by_to_ne_bytes {
    ($($t:ty),*) => {
        $(
            impl<BH: ByteHasher> HasherFor<$t> for BH {
                #[inline(always)] fn hash(&self, key: &$t, seed: u64) -> u64 {
                    self.hash_bytes(&key.to_ne_bytes(), seed)
                }
            }
        )*
    }
}

macro_rules! impl_by_borrow {
    ($($t:ty),*) => {
        $(
            impl<BH: ByteHasher> HasherFor<$t> for BH {
                #[inline(always)] fn hash(&self, key: &$t, seed: u64) -> u64 {
                    self.hash_bytes(std::borrow::Borrow::borrow(key), seed)
                }
            }
        )*
    }
}

impl_by_to_ne_bytes!(u8, u16, u32, u64, usize, i8, i16, i32, i64, isize, f32, f64);
impl_by_borrow!([u8], Box<[u8]>, Vec<u8>);

impl ByteHasher for gxhash::GxHasher {
    #[inline(always)] fn hash_bytes(&self, bytes: &[u8], seed: u64) -> u64 {
        self.hash_bytes(bytes, seed)
    }
}
