use ph::phast::{BucketToActivateEvaluator, Weights};

#[derive(Default, Clone, Copy)]
#[repr(transparent)]
pub struct F(pub f64);

impl PartialEq for F {
    #[inline(always)] fn eq(&self, other: &Self) -> bool { self.cmp(other).is_eq() }
}

impl PartialOrd for F {
    #[inline(always)] fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(other)) }
}

impl Eq for F {}

impl Ord for F {
    #[inline(always)] fn cmp(&self, other: &Self) -> std::cmp::Ordering { self.0.total_cmp(&other.0) }
}

pub struct WeightsF {
    pub size_weights: Box<[f64]>,
}

impl WeightsF {
    pub fn new(bits_per_seed: u8, slice_len: u16) -> Self {
        Weights::new(bits_per_seed, slice_len).into()
    }
}

impl From<ph::phast::Weights> for WeightsF {
    fn from(value: ph::phast::Weights) -> Self {
        Self { size_weights: value.0.iter().map(|v| *v as f64).collect() }
    }
}

impl BucketToActivateEvaluator for &WeightsF {
    type Value = F;

    const MIN: Self::Value = F(f64::MIN);

    fn eval(&self, bucket_nr: usize, bucket_size: usize) -> Self::Value {
        let sw = self.size_weights.get(bucket_size-1).copied()
            .unwrap_or_else(|| {
                let len = self.size_weights.len();
                let l = self.size_weights[len-1];
                let p = self.size_weights[len-2];
                l + (l-p) * (bucket_size - len) as f64
            });
        F(sw - 1024.0 * bucket_nr as f64)
    }
}