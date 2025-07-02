/// Evaluate bucket to be activate.
pub trait BucketToActivateEvaluator {
    /// Type of evaluation value.
    type Value: PartialEq + PartialOrd + Ord;

    /// Value lower than each value returned by `eval`.
    const MIN: Self::Value;

    /// Returns value for bucket with given index and size.
    /// The leftmost bucket with the largest value will be activated.
    fn eval(&self, bucket_nr: usize, bucket_size: usize) -> Self::Value;
}

/*
/// Activates bucket that maximizes 1024 * size - self.0 * number.
#[repr(transparent)]
pub struct Linear (pub u16);

impl BucketToActivateEvaluator for Linear {
    type Value = isize;

    const MIN: Self::Value = isize::MIN;

    #[inline(always)]
    fn eval(&self, bucket_nr: usize, bucket_size: usize) -> Self::Value {
        (bucket_size * 1024) as isize - (bucket_nr as isize * self.0 as isize)
    }
}

/// Activates bucket (of size S) that maximizes w2 * sqr(S) + w1 * S - 1024 * number.
pub struct Quadric {
    w2: i32,
    w1: i32,
    max_sqr_bucket: usize,
    size1_correction: i32
}

impl Quadric {
    pub fn new(w2: i32, w1: i32, size1_correction: i32) -> Self {
        Self {
            w2, w1,
            max_sqr_bucket: if w2 < 0 && w1 > 0 {
                (-w1 / (2*w2)) as usize
                //usize::MAX
            } else {
                usize::MAX
            },
            size1_correction,
        }
    }
}

impl BucketToActivateEvaluator for Quadric {
    type Value = isize;

    const MIN: Self::Value = isize::MIN;

    #[inline(always)]
    fn eval(&self, bucket_nr: usize, bucket_size: usize) -> Self::Value {
        let bucket_size_q = bucket_size.min(self.max_sqr_bucket) as isize;
        (bucket_size_q * bucket_size_q * self.w2 as isize) + (bucket_size as isize) * self.w1 as isize - bucket_nr as isize * 1024
        - if bucket_size == 1 { self.size1_correction as isize } else { 0 }
    }
}*/

#[derive(Clone)]
pub struct Weights(pub [i32; 7]);

impl Weights {
    pub fn new(bits_per_seed: u8, slice_len: u16) -> Self {
        Self(match (bits_per_seed, slice_len) {
            (_, 1025..) => [-1978, 14936, 89762, 150112, 190119, 224213, 343071], //8, 4.3
            (0..=4, 0..=512) => [-126969, 15686, 67995, 99429, 116711, 218955, 233075], //2.5
            (0..=4, _) => [-67844, 12942, 103312, 155604, 191240, 199105, 203210],  //2.5
            (5, 0..=512) => [-125171, 31908, 74770, 100065, 115115, 126729, 164878],    //2.9
            (5, _) => [-61359, 22918, 98732, 144970, 180112, 206496, 225555],    //2.9
            (6, 0..=512) => [-67857, 49430, 91006, 113610, 131179, 139109, 265291], //3.2
            (6, _) => [-55666, 36632, 104571, 145873, 173644, 195822, 221577],   //3.2
            (7, 0..=512) => [-67100, 66220, 100180, 115051, 131394, 142288, 148202],   //3.7
            (7, _) => [-50734, 49098, 107496, 143459, 169287, 189260, 204132],   //3.7
            (8, 0..=512) => [-61642, 85224, 112939, 129036, 140809, 150323, 155582], //4.3
            (8, _) => [-50171, 59462, 109868, 141865, 163564, 181092, 192852],    //4.3
            (9, 0..=512) => [-60668, 86903, 117046, 132208, 140749, 149552, 153428], //5.3
            (9, _) => [-58532, 61384, 117335, 146309, 164136, 179495, 187003],   //5.3
            (10, 0..=512) => [-65892, 66203, 136361, 155795, 162095, 171627, 174716],    //5.9
            (10, _) => [-65204, 67367, 119335, 145691, 163238, 179459, 185645],   //5.9
            (_, 0..=512) => [-64904, 67974, 141210, 154142, 162631, 171673, 174504],    //6.5
            (_, _) => [-63000, 69496, 123197, 147274, 164471, 179677, 184910], //6.5
        })
    }
}

impl BucketToActivateEvaluator for Weights {
    type Value = i64;

    const MIN: Self::Value = i64::MIN;

    fn eval(&self, bucket_nr: usize, bucket_size: usize) -> Self::Value {
        let sw = self.0.get(bucket_size-1).copied()
            .unwrap_or_else(|| {
                let len = self.0.len();
                let l = self.0[len-1];
                let p = self.0[len-2];
                l + (l-p) * (bucket_size - len) as i32
            }) as i64;
        sw - 1024 * bucket_nr as i64
    }
}