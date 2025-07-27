use crate::phast::{conf::{self, Conf}, cyclic::{GenericUsedValue, UsedValueSet}, seed_chooser::{best_seed_big, best_seed_small, SMALL_BUCKET_LIMIT}, SeedChooser};

/// Choose best seed without shift component.
#[derive(Clone, Copy)]
pub struct Walzer(pub u16);

#[inline(always)]
const fn mix64(mut x: u64) -> u64 {
    x = (x ^ (x >> 30)).wrapping_mul(0xbf58476d1ce4e5b9u64);
    x = (x ^ (x >> 27)).wrapping_mul(0x94d049bb133111ebu64);
    x ^ (x >> 31)
}

impl Walzer {
    #[inline] pub fn new(subslice_len: u16) -> Self {
        Self(subslice_len - 1)
    }

    #[inline(always)] fn extra_slice_shift(&self, hash_code: u64) -> u16 {
        mix64(hash_code) as u16
        //conf::mix(hash_code, 0x51_7c_c1_b7_27_22_0a_95) as u16
        //hash_code as u16
         & self.0
    }

    #[inline(always)] fn in_slice(hash_code: u64, seed: u16, conf: &Conf) -> u16 {
        conf::mix(conf::mix(seed as u64 ^ 0xa076_1d64_78bd_642f, 0x1d8e_4e27_c47d_124f), hash_code) as u16
        //conf::mult_hi((seed as u64).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95 /*0x1d8e_4e27_c47d_124f*/), hash_code) as u16
         & conf.slice_len_minus_one
    }
}

impl SeedChooser for Walzer {
    const WINDOW_SIZE: u16 = 256;

    type UsedValues = UsedValueSet;

    #[inline(always)] fn extra_shift(self, _bits_per_seed: u8) -> u16 {
        self.0
    }

    /*fn bucket_evaluator(&self, bits_per_seed: u8, slice_len: u16) -> Weights {
        Weights(
            if slice_len <= 256 { match (bits_per_seed, slice_len) {
                (..=6, ..=128) => [-98439, 68040, 81130, 86896, 91188, 93897, 296481],   // 6, 3.0, 128
                (..=6, _) => [-81980, 50520, 90817, 106897, 116472, 123937, 287280], // 6, 3.0, slice=256
                (_, ..=128) => [-173163, 58917, 73926, 83423, 88222, 92168, 206758], // 8, 4.1, slice=128
                (..=7, _) => [-85977, 81531, 98837, 107586, 113333, 117710, 120656],  // 7, 3.5, slice=256
                (_, _) => [-85787, 84108, 99553, 107291, 112859, 117377, 119965],   // 8, 4.1, slice=256
            }} else { match (bits_per_seed, slice_len) {
                (..=7, ..=512) => [-95834, 38499, 103035, 124756, 137603, 147839, 155448],  // 7, 3.5, slice=512
                (_, ..=512) => [-68137, 80516, 110189, 123629, 132794, 140850, 145685], // 8, 4.1, slice=512
                (..=8, ..=1024) => [-49776, 28610, 120514, 154976, 177328, 193499, 204936],  // 8, 4.1, slice=1024
                (..=8, ..=2048) => [-14014, -11926, 63698, 144877, 194056, 353593, 360338],  // 8, 4.1, slice=2048
                (9, ..=1024) => [-60439, 49207, 121850, 149181, 166713, 179181, 187815],  // 5.1, slice=1024
                (9, ..=2048) => [48168, 48328, 132443, 197796, 234543, 260358, 279164],  // 5.1, slice=2048
                (10, ..=1024) => [-4759, 9930, 87924, 125082, 143308, 165460, 165095], // 5.7, slice=1024
                (10, ..=2048) => [-3419, 8042, 98860, 145429, 176433, 198538, 214441],   // 5.7, slice=2048
                (_, ..=1024) => [-1560, 25555, 96323, 156791, 189688, 201315, 198828],    // 6.3, slice=1024
                (11, ..=2048) => [-294, 2300, 161956, 227418, 278332, 344537, 342726],  // 6.3, slice=2048
                (11, ..=4096) => [-2674, 19194, 37310, 111428, 167443, 205425, 236469], // 6.3, slice=4096
                (_, ..=2048) => [-1914, 10973, 70225, 173122, 240880, 305750, 293320],   // 12, 6.8, slice=2048
                (_, ..=4096) => [-2651, -447, 16106, 163680, 223955, 353813, 339271],  // 12, 6.8, slice=4096
                (_, _) => [-4309, -487, 21662, 26095, 83370, 157063, 543843],   // 12, 6.8, slice=8192
            }
        })
    }*/

    /*fn conf(self, output_range: usize, input_size: usize, bits_per_seed: u8, bucket_size_100: u16, preferred_slice_len: u16) -> Conf {
        let max_shift = self.extra_shift(bits_per_seed);
        let slice_len = match output_range.saturating_sub(max_shift as usize) {
            n @ ..8192 => (n/2+1).next_power_of_two() as u16,
            _ => 8192
        }.min(if preferred_slice_len != 0 { preferred_slice_len } else {
            match bits_per_seed {
                ..=4 => 128,
                ..=7 => 256,
                8 => 512,
                9 => 1024,
                10 => 2048,
                11 => 4096,
                _ => 8192
            }
        });
        Conf::new(output_range, input_size, bucket_size_100, slice_len, max_shift)
    }*/
    
    #[inline(always)] fn f(self, hash_code: u64, seed: u16, conf: &Conf) -> usize {
        conf.slice_begin(hash_code) + (self.extra_slice_shift(hash_code) + Self::in_slice(hash_code, seed, conf)) as usize
    }

    /*#[inline(always)] fn f_slice(primary_code: u64, slice_begin: usize, seed: u16, conf: &Conf) -> usize {
        slice_begin + conf.in_slice(primary_code, seed)
    }*/

    #[inline(always)]
    fn best_seed(self, used_values: &mut Self::UsedValues, keys: &[u64], conf: &Conf, bits_per_seed: u8) -> u16 {
        let mut best_seed = 0;
        let mut best_value = usize::MAX;
        if keys.len() <= SMALL_BUCKET_LIMIT {
            best_seed_small(self, &mut best_value, &mut best_seed, used_values, keys, conf, 1<<bits_per_seed)
        } else {
            best_seed_big(self, &mut best_value, &mut best_seed, used_values, keys, conf, 1<<bits_per_seed)
        };
        if best_seed != 0 { // can assign seed to the bucket
            for key in keys {
                used_values.add(self.f(*key, best_seed, conf));
            }
        };
        best_seed
    }
}