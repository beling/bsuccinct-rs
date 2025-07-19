use crate::phast::{conf::Conf, cyclic::{CyclicSet, GenericUsedValue, UsedValueSetLarge}, Weights};
use super::SeedChooser;

#[inline] fn self_collide(without_shift: &mut [usize]) -> bool {
    without_shift.sort_unstable();  // maybe it is better to postpone self-collision test?
    for i in 1..without_shift.len() {
        if without_shift[i-1] == without_shift[i] { // self-collision?
            return true;
        }
    }
    false
}

#[inline] fn shifts0<'k, 'c>(keys: &'k [u64], conf: &'c Conf) -> impl Iterator<Item = usize> + use<'k, 'c> {
    keys.iter().map(|key| conf.f_shift0(*key))
}

#[inline] fn occupy_sum<const UVS: usize>(mut excluded: u64, used_values: &CyclicSet<UVS>, without_shift: &[usize], shift: u16) -> u64 {
    for first in without_shift.iter() {
        excluded |= used_values.get64(*first + shift as usize);
    }
    excluded
}

#[inline] fn mark_used<const UVS: usize>(used_values: &mut CyclicSet<UVS>, without_shift: &[usize], total_shift: u16) {
    for first in without_shift {
        used_values.add(*first + total_shift as usize);
    }
}

#[derive(Clone, Copy, Default)]
pub struct ShiftOnly;

//pub static SELF_COLLISION_KEYS: AtomicU64 = AtomicU64::new(0);
//pub static SELF_COLLISION_BUCKETS: AtomicU64 = AtomicU64::new(0);

impl SeedChooser for ShiftOnly {
    const FUNCTION2_THRESHOLD: usize = 8192;

    type UsedValues = UsedValueSetLarge;

    fn bucket_evaluator(&self, bits_per_seed: u8, slice_len: u16) -> Weights {
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
    }

    fn conf(self, output_range: usize, input_size: usize, bits_per_seed: u8, bucket_size_100: u16, preferred_slice_len: u16) -> Conf {
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
    }

    #[inline(always)] fn extra_shift(self, bits_per_seed: u8) -> u16 {
        (1 << bits_per_seed) - 2
    }

    #[inline(always)] fn f(self, primary_code: u64, seed: u16, conf: &Conf) -> usize {
        conf.f_shift0(primary_code) + (seed-1) as usize
    }

    /*#[inline(always)] fn f_slice(primary_code: u64, slice_begin: usize, seed: u16, conf: &Conf) -> usize {
        slice_begin + conf.in_slice_noseed(primary_code) + (seed-1) as usize*MULTIPLIER as usize
    }*/

    #[inline]
    fn best_seed(self, used_values: &mut Self::UsedValues, keys: &[u64], conf: &Conf, bits_per_seed: u8) -> u16 {
        let mut without_shift_arrayvec: arrayvec::ArrayVec::<usize, 16>;
        let mut without_shift_box: Box<[usize]>;
        let without_shift: &mut [usize] = if keys.len() > 16 {
            without_shift_box = shifts0(keys, conf).collect();
            &mut without_shift_box
        } else {
            without_shift_arrayvec = shifts0(keys, conf).collect();
            &mut without_shift_arrayvec
        };
        //if self_collide(without_shift) { return 0; }    // maybe it is better to postpone self-collision test? 4.51 6.85 9.10
        let last_shift = (1 << bits_per_seed) - 1;
        for shift in (0..last_shift).step_by(64) {
            let used = occupy_sum(0, used_values, &without_shift, shift);
            if used != u64::MAX {
                if self_collide(without_shift) { return 0; }    // maybe it is better to postpone self-collision test? 4.46 6.76 9.02
                let total_shift = shift + used.trailing_ones() as u16;
                if total_shift >= last_shift /*|| self_collide(without_shift)*/ { return 0; }   //total_shift+1 is too large
                //if self_collide(without_shift) { return 0; }    // maybe it is better to postpone self-collision test? 4.43 6.77
                mark_used(used_values, without_shift, total_shift);
                return total_shift + 1;
            }
        }
        0
    }
}

