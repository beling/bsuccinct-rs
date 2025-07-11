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
        Self(if slice_len <= 256 {  // this is used only for small number of keys
            match (bits_per_seed, slice_len) {
                (..=4, ..=32) => [-64542, 121567, 125058, 126982, 128486, 129929, 131003], // 2.5
                (..=4, ..=64) => [-64511, 116865, 123821, 127467, 130311, 132528, 134191], // 2.5
                (..=4, ..=128) => [-64100, 107340, 121197, 128499, 133718, 137312, 140441], // 2.5
                (..=4, _) => [-73492, 86604, 113513, 128141, 138220, 145456, 151294],  // 2.5
                (..=6, ..=32) => [-63646, 124629, 127000, 128169, 129621, 130183, 130981],  // 6, 3.2
                (..=6, ..=64) => [-63968, 120034, 125091, 127987, 130094, 131516, 132634],  // 6, 3.2
                (..=6, ..=128) => [-64639, 112284, 121682, 127366, 131200, 134360, 136609], // 6, 3.2
                (..=6, _) => [-72990, 97195, 115735, 127046, 134403, 140267, 144429], // 6, 3.2
                (_, ..=32) => [-60034, 117057, 129045, 130280, 131078, 131608, 132110],   // 8, 4.3
                (_, ..=64) => [-61931, 123320, 127144, 129416, 130764, 132175, 132978],   // 8, 4.3
                (_, ..=128) => [-64853, 115515, 122738, 127413, 130280, 132894, 134336],    // 8, 4.3
                (_, _) => [-73167, 104363, 117831, 126314, 132226, 137072, 139738],    // 8, 4.3
            }
        } else {    // for 512+
            match (bits_per_seed, slice_len) {
                (..=4, ..=512) => [-126969, 15686, 67995, 99429, 116711, 218955, 233075], // 2.5
                (..=4, ..=1024) => [-67844, 12942, 103312, 155604, 191240, 199105, 203210],  // 2.5
                (5, ..=512) => [-125171, 31908, 74770, 100065, 115115, 126729, 164878],    // 5, 2.9
                (5, ..=1024) => [-61359, 22918, 98732, 144970, 180112, 206496, 225555],    // 5, 2.9
                (6, ..=512) => [-67857, 49430, 91006, 113610, 131179, 139109, 265291], // 3.2
                (6, ..=1024) => [-55666, 36632, 104571, 145873, 173644, 195822, 221577],   // 3.2
                (7, ..=512) => [-67100, 66220, 100180, 115051, 131394, 142288, 148202],   // 3.7
                (7, ..=1024) => [-50734, 49098, 107496, 143459, 169287, 189260, 204132],   // 3.7
                (8, ..=512) => [-61642, 85224, 112939, 129036, 140809, 150323, 155582], // 4.3
                (8, ..=1024) => [-50171, 59462, 109868, 141865, 163564, 181092, 192852],    // 4.3
                (..=8, _) => [-1978, 14936, 89762, 150112, 190119, 224213, 343071], // 8, 4.3, 2048
                (9, ..=512) => [-60668, 86903, 117046, 132208, 140749, 149552, 153428], // 5.3
                (9, ..=1024) => [-58532, 61384, 117335, 146309, 164136, 179495, 187003],   // 5.3
                (9, _) => [-2028, 10459, 102197, 161103, 201199, 227967, 354134],  // 9, 5.3, 2048
                (10, ..=512) => [-65892, 66203, 136361, 155795, 162095, 171627, 174716],    // 5.9
                (10, ..=1024) => [-65204, 67367, 119335, 145691, 163238, 179459, 185645],   // 5.9
                (10, _) => [-1683, 8322, 119258, 171679, 203830, 233213, 320945], // 10, 5.9, 2048
                (_, ..=512) => [-64904, 67974, 141210, 154142, 162631, 171673, 174504],    // 11, 6.5, 512
                (_, ..=1024) => [-63000, 69496, 123197, 147274, 164471, 179677, 184910], // 11, 6.5, 1024
                (_, _) => [-1566, 8599, 116024, 185394, 213039, 237292, 249657] // 11, 6.5, 2048
            }
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