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

pub struct Weights(pub [i32; 7]);

impl Weights {
    pub fn new(bits_per_seed: u8, partition_size: u16) -> Self {
        Self(match (bits_per_seed, partition_size) {
            (0..=4, 0..=512) => [-126969, 15686, 67995, 99429, 116711, 218955, 233075], //2.5
            (0..=4, _) => [-67844, 12942, 103312, 155604, 191240, 199105, 203210],  //2.5
            (5, 0..=512) => [-125171, 31908, 74770, 100065, 115115, 126729, 164878],    //2.9
            (5, _) => [-61359, 22918, 98732, 144970, 180112, 206496, 225555],    //2.9
            (6, 0..=512) => [-67857, 49430, 91006, 113610, 131179, 139109, 265291], //3.2
            (6, _) => [-54990, 35659, 103915, 146017, 172731, 196182, 221450],   //3.2
            (7, 0..=512) => [-67100, 66220, 100180, 115051, 131394, 142288, 148202],   //3.7
            (7, _) => [-54348, 50410, 106437, 141724, 167803, 184975, 200762],   //3.7
            (8, 0..=512) => [-61642, 85224, 112939, 129036, 140809, 150323, 155582], //4.3
            (8, _) => [-52442, 60938, 110037, 140343, 163340, 180429, 192161],    //4.3
            (9, 0..=512) => [-60668, 86903, 117046, 132208, 140749, 149552, 153428], //5.3
            (9, _) => [-63810, 64097, 116638, 143572, 162978, 179283, 187029],   //5.3
            (_, 0..=512) => [-65892, 66203, 136361, 155795, 162095, 171627, 174716],    //5.9
            (_, _) => [-66184, 64417, 120321, 146569, 163302, 179408, 185470]   //5.9
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