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

/// Calculates a mask that has 0 only at positions divided by `multiplier`.
const fn zero_at_each(multiplier: u8) -> u64 {
    let mut result = u64::MAX;
    let mut i = 0;
    while i < 64 {
        result ^= 1<<i;
        i += multiplier;
    }
    result
}

/// Common code for checking each `MULTIPLIER` position.
pub(crate) struct Multiplier<const MULTIPLIER: u8>;

impl<const MULTIPLIER: u8> Multiplier<MULTIPLIER> {
    pub(crate) const MASK: u64 = zero_at_each(MULTIPLIER); // mask that has 0 only at positions divided by `MULTIPLIER`
    pub(crate) const STEP: usize = 64 - 64 % MULTIPLIER as usize;  // number of bits to use from each 64-bit fragment of used bitmap.

    /**
     * Returns the lowest collision-free shift which is lower than `shift_end`.
     * or `None` if there are no collision-free shifts lower than `shift_end`.
     * 
     * For each key, `without_shift` contains begin index of the key slice and initial key position in this slice.
     * The final value for each key is: its slice begin index + its initial position in slice + returned shift.
     * 
     * `used_values` shows values already used by the keys from other buckets.
     */
    #[inline]
    pub(crate) fn best_in_range<const UVS: usize>(shift_end: u16, without_shift: &mut [(usize, u16)], used_values: &CyclicSet<UVS>) -> Option<u16> {
        without_shift.sort_unstable_by_key(|(sb, sh0)| sb+*sh0 as usize);  // maybe it is better to postpone self-collision test?
        if without_shift.windows(2).any(|v| v[0].0+v[0].1 as usize==v[1].0+v[1].1 as usize) {
            return None;
        }
        for shift in (0..shift_end).step_by(Self::STEP) {
            let mut used = Self::MASK;
            for &(sb, sh0) in without_shift.iter() {
                used |= used_values.get64(sb + sh0 as usize + shift as usize);
            }
            if used != u64::MAX {
                let total_shift = shift + used.trailing_ones() as u16;
                if total_shift >= shift_end { return None; }

                /*without_shift.sort_unstable_by_key(|(sb, sh0)| sb+*sh0 as usize);  // maybe it is better to postpone self-collision test?
                if without_shift.windows(2).any(|v| v[0].0+v[0].1 as usize==v[1].0+v[1].1 as usize) {
                    return None;
                }*/

                return Some(total_shift);
            }
        }
        None
    }

    pub(crate) fn multiple_rounded_up(mut shift_end: u16) -> u16 {
        if MULTIPLIER != 1 {    // round up shift_end to MULTIPLIER
            let r = shift_end % MULTIPLIER as u16;
            if r != 0 {
                shift_end -= r;
                shift_end += MULTIPLIER as u16;
            }
        }
        shift_end
    }
}

#[derive(Clone, Copy, Default)]
pub struct ShiftOnly<const MULTIPLIER: u8>;

//pub static SELF_COLLISION_KEYS: AtomicU64 = AtomicU64::new(0);
//pub static SELF_COLLISION_BUCKETS: AtomicU64 = AtomicU64::new(0);

impl<const MULTIPLIER: u8> SeedChooser for ShiftOnly<MULTIPLIER> {
    const FUNCTION2_THRESHOLD: usize = 8192;

    type UsedValues = UsedValueSetLarge;

    fn bucket_evaluator(&self, bits_per_seed: u8, slice_len: u16) -> Weights {
        Weights(match MULTIPLIER {
            1 => if slice_len <= 256 { match (bits_per_seed, slice_len) {
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
            }},
            2 => match (bits_per_seed, slice_len) {
                (..=6, ..=256) => [-115873, 67318, 91593, 101415, 109433, 114568, 118100], // 6, 3.0, slice=256
                (_, ..=256) => [-83435, 78758, 97979, 107759, 114723, 120029, 124071],  // 7, 3.5, slice=256
                (..=7, ..=512) => [-68902, 66418, 102860, 120541, 131314, 141179, 146993],  // 7, 3.5, slice=512
                (_, ..=512) => [-69259, 68202, 106995, 125358, 137996, 147309, 153416], // 8, 4.1, slice=512
                (..=8, ..=1024) => [-61818, 63888, 120146, 148699, 166377, 182094, 192529],  // 8, 4.1, slice=1024
                (9, ..=1024) => [-64174, 1575, 129667, 173689, 195108, 206753, 210827],  // 5.1, slice=1024
                (..=9, ..=2048) => [40622, 47541, 152267, 207384, 240842, 267175, 286192],  // 5.1, slice=2048
                (_, ..=1024) => [-4917, 9392, 71117, 135275, 160314, 167119, 166593], // 10, 5.7, slice=1024
                (_, ..=2048) => [470, 3610, 158817, 228496, 259877, 277325, 288015],   // 10, 5.7, slice=2048
                (..=10, ..=4096) => [484, 3928, 85136, 206010, 290275, 298268, 319235],   // 10, 5.7, slice=4096
                (_, ..=4096) => [484, 3928, 85136, 206010, 290275, 298268, 319235],   // 11, 6.3, slice=4096
                (_, _) => [23, 5319, 18739, 32026, 106955, 561834, 601688]  // 11, 6.3, slice=8192
            },
            _ => match (bits_per_seed, slice_len) {
                (..=6, ..=256) => [-165673, 47937, 70800, 81727, 88912, 94702, 214772], // 6, 3.0, slice=256
                (_, ..=256) => [-89014, 62253, 95123, 111163, 120044, 127382, 132357],  // 7, 3.5, slice=256
                (..=7, ..=512) => [-105937, 52230, 90111, 108005, 120336, 130447, 136519],  // 7, 3.5, slice=512
                (_, ..=512) => [-113986, 13037, 89948, 119210, 139847, 155848, 172341], // 8, 4.1, slice=512
                (..=8, ..=1024) => [-77866, 40205, 107467, 137648, 160524, 180888, 191145],  // 8, 4.1, slice=1024
                (..=8, ..=2048) => [-9369, -14197, 98154, 153962, 191298, 221367, 404397],  // 8, 4.1, slice=2048
                (9, ..=1024) => [-63625, -52346, 123410, 236815, 285122, 235436, 296425],  // 5.1, slice=1024
                (9, ..=2048) => [34106, 44961, 169290, 228772, 269614, 295101, 318090],  // 5.1, slice=2048
                (10, ..=1024) => [-2792, -18731, 104163, 218647, 279408, 306571, 305898], // 5.7, slice=1024
                (10, ..=2048) => [-12545, -13976, 111831, 216764, 425316, 428156, 403192],   // 5.7, slice=2048
                (11, ..=1024) => [-3372, 22898, -29159, 125875, 188541, 240114, 287330],    // 6.3, slice=1024
                (11, ..=2048) => [-400, 836, 33973, 47879, 569389, 574778, 609280],  // 6.3, slice=2048
                (_, ..=1024) => [-5628, 25696, -33780, -1799, 20017, 241036, 477819], // 12, 6.8, slice=1024
                (_, _) => [-3230, 20553, 54706, 68651, 180485, 87885, 266428],   // 12, 6.8, slice=2048
            }
        })
    }

    fn conf(self, output_range: usize, input_size: usize, bits_per_seed: u8, bucket_size_100: u16, preferred_slice_len: u16) -> Conf {
        let max_shift = self.extra_shift(bits_per_seed);
        let slice_len = match output_range.saturating_sub(max_shift as usize) {
            n @ ..8192 => (n/2+1).next_power_of_two() as u16,
            _ => 8192
        }.min(if preferred_slice_len != 0 { preferred_slice_len } else { match MULTIPLIER {
            1 => match bits_per_seed {
                ..=4 => 128,
                ..=7 => 256,
                8 => 512,
                9 => 1024,
                10 => 2048,
                11 => 4096,
                _ => 8192
            },
            /*2 => match bits_per_seed {
                ..=4 => 128,
                ..=6 => 256,
                7 => 512,
                8 => 1024,
                _ => 2048   // TODO check 9
            },*/
            _ => match bits_per_seed {
                ..=4 => 128,
                ..=6 => 256,
                7 => 512,
                8 => 1024,
                9 => 2048,
                10 => 4096,
                _ => 8192
            },
        }});
        Conf::new(output_range, input_size, bucket_size_100, slice_len, max_shift)
    }

    #[inline(always)] fn extra_shift(self, bits_per_seed: u8) -> u16 {
        ((MULTIPLIER as u16) << bits_per_seed) - 2*MULTIPLIER as u16
    }

    #[inline(always)] fn f(self, primary_code: u64, seed: u16, conf: &Conf) -> usize {
        conf.f_shift0(primary_code) + (seed-1) as usize*MULTIPLIER as usize
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
        let last_shift = ((MULTIPLIER as u16) << bits_per_seed) - MULTIPLIER as u16;
        for shift in (0..last_shift).step_by(Multiplier::<MULTIPLIER>::STEP) {
            let used = occupy_sum(Multiplier::<MULTIPLIER>::MASK, used_values, &without_shift, shift);
            if used != u64::MAX {
                if self_collide(without_shift) { return 0; }    // maybe it is better to postpone self-collision test? 4.46 6.76 9.02
                let total_shift = shift + used.trailing_ones() as u16;
                if total_shift >= last_shift /*|| self_collide(without_shift)*/ { return 0; }   //total_shift+1 is too large
                //if self_collide(without_shift) { return 0; }    // maybe it is better to postpone self-collision test? 4.43 6.77
                mark_used(used_values, without_shift, total_shift);
                return total_shift / MULTIPLIER as u16 + 1;
            }
        }
        0
    }
}

