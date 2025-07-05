use crate::phast::{conf::{mix_key_seed, Conf}, cyclic::{GenericUsedValue, UsedValueSet}, Weights};
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

#[inline] fn occupy_sum(mut excluded: u64, used_values: &UsedValueSet, without_shift: &[usize], shift: u16) -> u64 {
    for first in without_shift.iter() {
        excluded |= used_values.get64(*first + shift as usize);
    }
    excluded
}

#[inline] fn mark_used(used_values: &mut UsedValueSet, without_shift: &[usize], total_shift: u16) {
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
struct Multiplier<const MULTIPLIER: u8>;

impl<const MULTIPLIER: u8> Multiplier<MULTIPLIER> {
    const MASK: u64 = zero_at_each(MULTIPLIER); // mask that has 0 only at positions divided by `MULTIPLIER`
    const STEP: usize = 64 - 64 % MULTIPLIER as usize;  // number of bits to use from each 64-bit fragment of used bitmap.

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
    fn best_in_range(shift_end: u16, without_shift: &mut [(usize, u16)], used_values: &UsedValueSet) -> Option<u16> {
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
                return Some(total_shift);
            }
        }
        None
    }

    fn multiple_rounded_up(mut shift_end: u16) -> u16 {
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
    type UsedValues = UsedValueSet;

    fn bucket_evaluator(&self, bits_per_seed: u8, slice_len: u16) -> Weights {
        Weights(match MULTIPLIER {  // TODO optimize 4 128
            1 => match (bits_per_seed, slice_len) {
                (..=6, ..=256) => [-81980, 50520, 90817, 106897, 116472, 123937, 287280], // 6, 3.0, slice=256
                (_, ..=256) => [-85977, 81531, 98837, 107586, 113333, 117710, 120656],  // 7, 3.5, slice=256
                (..=7, ..=512) => [-95834, 38499, 103035, 124756, 137603, 147839, 155448],  // 7, 3.5, slice=512
                (_, ..=512) => [-68137, 80516, 110189, 123629, 132794, 140850, 145685], // 8, 4.1, slice=512
                (..=8, ..=1024) => [-49776, 28610, 120514, 154976, 177328, 193499, 204936],  // 8, 4.1, slice=1024
                (..=8, _) => [-14014, -11926, 63698, 144877, 194056, 353593, 360338],  // 8, 4.1, slice=2048
                (9, ..=1024) => [-60439, 49207, 121850, 149181, 166713, 179181, 187815],  // 5.1, slice=1024
                (9, _) => [48168, 48328, 132443, 197796, 234543, 260358, 279164],  // 5.1, slice=2048
                (10, ..=1024) => [-4759, 9930, 87924, 125082, 143308, 165460, 165095], // 5.7, slice=1024
                (10, _) => [-3419, 8042, 98860, 145429, 176433, 198538, 214441],   // 5.7, slice=2048
                (11, ..=1024) => [-1560, 25555, 96323, 156791, 189688, 201315, 198828],    // 6.3, slice=1024
                (11, _) => [-294, 2300, 161956, 227418, 278332, 344537, 342726],  // 6.3, slice=2048
                (_, ..=1024) => [-4990, 23096, -60766, 148667, 192850, 217777, 214747], // 12, 6.8, slice=1024
                (_, _) => [-1914, 10973, 70225, 173122, 240880, 305750, 293320],   // 12, 6.8, slice=2048
            },
            2 => match (bits_per_seed, slice_len) {
                (..=6, ..=256) => [-115873, 67318, 91593, 101415, 109433, 114568, 118100], // 6, 3.0, slice=256
                (_, ..=256) => [-83435, 78758, 97979, 107759, 114723, 120029, 124071],  // 7, 3.5, slice=256
                (..=7, ..=512) => [-68902, 66418, 102860, 120541, 131314, 141179, 146993],  // 7, 3.5, slice=512
                (_, ..=512) => [-69259, 68202, 106995, 125358, 137996, 147309, 153416], // 8, 4.1, slice=512
                (..=8, ..=1024) => [-61818, 63888, 120146, 148699, 166377, 182094, 192529],  // 8, 4.1, slice=1024
                (..=8, _) => [-11265, -3609, 98176, 161487, 203684, 362099, 424579],  // 8, 4.1, slice=2048
                (9, ..=1024) => [-64174, 1575, 129667, 173689, 195108, 206753, 210827],  // 5.1, slice=1024
                (9, _) => [40622, 47541, 152267, 207384, 240842, 267175, 286192],  // 5.1, slice=2048
                (10, ..=1024) => [-4917, 9392, 71117, 135275, 160314, 167119, 166593], // 5.7, slice=1024
                (10, _) => [470, 3610, 158817, 228496, 259877, 277325, 288015],   // 5.7, slice=2048
                (11, ..=1024) => [-2775, 21749, -46062, 167191, 209130, 225950, 225267],    // 6.3, slice=1024
                (11, _) => [-329, 2456, 101429, 227303, 292154, 361298, 352134],  // 6.3, slice=2048
                (_, ..=1024) => [-5686, 25837, -33718, -1789, 19954, 231032, 475063], // 12, 6.8, slice=1024
                (_, _) => [-2348, 10665, 53981, 67995, 242406, 278044, 488961],   // 12, 6.8, slice=2048
            },
            _ => match (bits_per_seed, slice_len) {
                (..=6, ..=256) => [-165673, 47937, 70800, 81727, 88912, 94702, 214772], // 6, 3.0, slice=256
                (_, ..=256) => [-89014, 62253, 95123, 111163, 120044, 127382, 132357],  // 7, 3.5, slice=256
                (..=7, ..=512) => [-105937, 52230, 90111, 108005, 120336, 130447, 136519],  // 7, 3.5, slice=512
                (_, ..=512) => [-113986, 13037, 89948, 119210, 139847, 155848, 172341], // 8, 4.1, slice=512
                (..=8, ..=1024) => [-77866, 40205, 107467, 137648, 160524, 180888, 191145],  // 8, 4.1, slice=1024
                (..=8, _) => [-9369, -14197, 98154, 153962, 191298, 221367, 404397],  // 8, 4.1, slice=2048
                (9, ..=1024) => [-63625, -52346, 123410, 236815, 285122, 235436, 296425],  // 5.1, slice=1024
                (9, _) => [34106, 44961, 169290, 228772, 269614, 295101, 318090],  // 5.1, slice=2048
                (10, ..=1024) => [-2792, -18731, 104163, 218647, 279408, 306571, 305898], // 5.7, slice=1024
                (10, _) => [-12545, -13976, 111831, 216764, 425316, 428156, 403192],   // 5.7, slice=2048
                (11, ..=1024) => [-3372, 22898, -29159, 125875, 188541, 240114, 287330],    // 6.3, slice=1024
                (11, _) => [-400, 836, 33973, 47879, 569389, 574778, 609280],  // 6.3, slice=2048
                (_, ..=1024) => [-5628, 25696, -33780, -1799, 20017, 241036, 477819], // 12, 6.8, slice=1024
                (_, _) => [-3230, 20553, 54706, 68651, 180485, 87885, 266428],   // 12, 6.8, slice=2048
            }
        })
    }

    fn conf(self, output_range: usize, input_size: usize, bits_per_seed: u8, bucket_size_100: u16, preferred_slice_len: u16) -> Conf {
        let max_shift = self.extra_shift(bits_per_seed);
        let slice_len = match output_range.saturating_sub(max_shift as usize) {
            n @ 0..64 => (n/2+1).next_power_of_two() as u16,
            64..1300 => 64,
            1300..1750 => 128,
            1750..7500 => 256,
            7500..150000 => 512,
            150000..250000 => 1024,
            _ => 2048,
        }.min(if preferred_slice_len != 0 { preferred_slice_len } else { match MULTIPLIER {
            1 => match bits_per_seed {
                ..=4 => 128,
                ..=7 => 256,
                8 => 512,
                9 => 1024,
                _ => 2048
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
                _ => 2048
            },
        }});
        Conf::new(output_range, input_size, bucket_size_100, slice_len, max_shift)
    }

    #[inline(always)] fn extra_shift(self, bits_per_seed: u8) -> u16 {
        (1 << bits_per_seed) * MULTIPLIER as u16 - 2*MULTIPLIER as u16
    }

    #[inline(always)] fn f(self, primary_code: u64, seed: u16, conf: &Conf) -> usize {
        conf.f_shift0(primary_code) + (seed-1) as usize*MULTIPLIER as usize
    }

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
        if self_collide(without_shift) { return 0; }    // maybe it is better to postpone self-collision test?
        let last_shift = ((MULTIPLIER as u16) << bits_per_seed) - MULTIPLIER as u16;
        for shift in (0..last_shift).step_by(Multiplier::<MULTIPLIER>::STEP) {
            let used = occupy_sum(Multiplier::<MULTIPLIER>::MASK, used_values, &without_shift, shift);
            if used != u64::MAX {
                let total_shift = shift + used.trailing_ones() as u16;
                if total_shift >= last_shift { return 0; }   //total_shift+1 is too large
                mark_used(used_values, without_shift, total_shift);
                return total_shift / MULTIPLIER as u16 + 1;
            }
        }
        0
    }
}

#[derive(Clone, Copy)]
pub struct ShiftOnlyWrapped<const MULTIPLIER: u8>;

impl<const MULTIPLIER: u8> SeedChooser for ShiftOnlyWrapped<MULTIPLIER> {
    type UsedValues = UsedValueSet;

    fn bucket_evaluator(&self, bits_per_seed: u8, slice_len: u16) -> Weights {
        Weights(match MULTIPLIER {
            1 => match (bits_per_seed, slice_len) {
                (..=6, ..=256) => [-76632, 59701, 89939, 103115, 111040, 117906, 283652], // 6, 3.0, slice=256
                (..=6, ..=512) => [-102171, 30195, 95877, 122987, 138980, 152173, 206055],  // 6, 3.0, slice=512
                (_, ..=256) => [-84425, 81165, 96951, 106065, 112137, 117421, 122309],  // 7, 3.5, slice=256
                (7, ..=512) => [-69271, 61152, 101770, 119869, 132454, 141236, 148273],  // 7, 3.5, slice=512
                (8, ..=512) => [-66903, 81776, 107154, 122354, 132033, 140641, 146584], // 4.1, slice=512
                (8, ..=1024) => [-50666, 55977, 116129, 145446, 164172, 180129, 192120],  // 4.1, slice=1024
                (_, ..=512) => [-45845, 91690, 122225, 138169, 149160, 155706, 164757],  // 5.1, slice=512
                (9, ..=1024) => [-51695, 68190, 121468, 146481, 164082, 178054, 186488],  // 5.1, slice=1024
                (..=9, _) => [-3365, 12300, 85113, 138418, 170087, 197668, 215654],  // 9, 5.1, slice=2048
                (10, ..=1024) => [-4011, 15045, 106558, 112844, 133305, 145623, 154991], // 5.7, slice=1024
                (..=10, _) => [-3301, 12449, 83323, 139924, 169323, 198105, 212187],   // 10, 5.7, slice=2048
                (11, ..=1024) => [-1524, 23928, 115028, 153353, 187370, 191075, 197861],    // 6.3, slice=1024
                (11, _) => [-1777, 22788, 106158, 139632, 174143, 200775, 214797],  // 6.3, slice=2048
                (_, ..=1024) => [-2190, 30393, 114587, 141471, 162103, 177602, 183787], // 12, 6.8, slice=1024
                (_, _) => [-2355, 16099, 113987, 153868, 183912, 213486, 226897],   // 12, 6.8, slice=2048
            },
            _ => match (bits_per_seed, slice_len) {
                (..=6, ..=256) => [-113309, 70659, 92719, 103205, 111784, 117218, 121395], // 6, 3.0, slice=256
                (..=6, ..=512) => [-113437, 36479, 87740, 109716, 124793, 137012, 209528], // 6, 3.0, slice=512
                (_, ..=256) => [-83108, 76805, 93889, 104574, 111919, 117701, 137200],  // 7, 3.5, slice=256
                (7, ..=512) => [-11364, 71851, 100238, 116988, 128732, 138656, 145275],  // 7, 3.5, slice=512
                (8, ..=512) => [-67763, 78133, 104489, 121464, 133392, 140946, 155107], // 4.1, slice=512
                (8, ..=1024) => [-50137, 65904, 111782, 139890, 159029, 175922, 186995],  // 4.1, slice=1024
                (..=8, _) => [-3445, 11224, 85129, 138005, 176794, 209479, 234058], // 8, 4.1, slice=2048
                (_, ..=512) => [-45845, 91690, 122225, 138169, 149160, 155706, 164757],  // 5.1, slice=512
                (9, ..=1024) => [-49692, 70537, 115707, 143201, 163216, 178981, 188448],  // 5.1, slice=1024
                (..=9, _) => [-3514, 12002, 85880, 136849, 171127, 197699, 215787],  // 9, 5.1, slice=2048
                (10, ..=1024) => [-4383, 16783, 82867, 112554, 131931, 148281, 156013], // 5.7, slice=1024
                (..=10, _) => [-3386, 13051, 82525, 133516, 169004, 198445, 214518],   // 10, 5.7, slice=2048
                (11, ..=1024) => [-1562, 24796, 122828, 155722, 174139, 191420, 198085],    // 6.3, slice=1024
                (11, _) => [-2243, 21043, 83146, 136599, 172186, 200713, 215298],  // 6.3, slice=2048
                (_, ..=1024) => [-2244, 32777, 107196, 142051, 161424, 177763, 183475], // 12, 6.8, slice=1024
                (_, _) => [-2808, 16026, 90346, 150206, 185508, 214963, 227887],   // 12, 6.8, slice=2048
            }
        })
    }

    fn conf(self, output_range: usize, input_size: usize, bits_per_seed: u8, bucket_size_100: u16, preferred_slice_len: u16) -> Conf {
        let max_shift = self.extra_shift(bits_per_seed);
        let slice_len = match output_range.saturating_sub(max_shift as usize) {
            n @ 0..64 => (n/2+1).next_power_of_two() as u16,
            64..1300 => 64,
            1300..1750 => 128,
            1750..7500 => 256,
            7500..150000 => 512,
            150000..250000 => 1024,
            _ => 2048,
        }.min(if preferred_slice_len != 0 { preferred_slice_len } else { match MULTIPLIER {
            1 => match bits_per_seed {
                ..=5 => 256,
                ..=7 => 512,   // or 6 => 256 for smaller size
                ..=9 => 1024,   // or 8 => 512 for smaller size
                _ => 2048   // or 10 => 1024 for smaller size
            },
            2 => match bits_per_seed {
                ..=5 => 256,
                ..=7 => 512,
                8 => 1024,
                _ => 2048   // for 9, 1024 is also a good choice
            },
            _ => match bits_per_seed {
                ..=4 => 256,
                ..=7 => 512,
                8 => 1024,
                _ => 2048
            },
        }});        
        Conf::new(output_range, input_size, bucket_size_100, slice_len, max_shift)
    }

    #[inline(always)] fn f(self, primary_code: u64, seed: u16, conf: &Conf) -> usize {
        conf.slice_begin(primary_code) + ((primary_code as usize).wrapping_add((seed-1) as usize*MULTIPLIER as usize) & conf.slice_len_minus_one as usize)
    }

    #[inline]
    fn best_seed(self, used_values: &mut Self::UsedValues, keys: &[u64], conf: &Conf, bits_per_seed: u8) -> u16 {
        let mut without_shift_arrayvec: arrayvec::ArrayVec::<(usize, u16), 16>;
        let mut without_shift_box: Box<[(usize, u16)]>;
        let without_shift: &mut [(usize, u16)] = if keys.len() > 16 {
            without_shift_box = keys.iter().map(|key| (conf.slice_begin(*key), *key as u16 & conf.slice_len_minus_one)).collect();
            &mut without_shift_box
        } else {
            without_shift_arrayvec = keys.iter().map(|key| (conf.slice_begin(*key), *key as u16 & conf.slice_len_minus_one)).collect();
            &mut without_shift_arrayvec
        };

        let slice_len = conf.slice_len();
        let mut score_without_shift: usize = 1<<20;
        let mut best_score = usize::MAX;
        let mut total_end_shift = ((MULTIPLIER as u16) << bits_per_seed) - MULTIPLIER as u16;
        // note that total_last_shift itself is not allowed
        let mut shift_sum = 0;
        let mut best_total_shift = u16::MAX;
        loop {  // while total_last_shift > 0
            let max_sh0 = without_shift.iter().map(|(_, sh0)| *sh0).max().unwrap();
            let mut shift_end = Multiplier::<MULTIPLIER>::multiple_rounded_up(slice_len - max_sh0);
            let last = shift_end >= total_end_shift;
            if last { shift_end = total_end_shift; }
            if score_without_shift < best_score {
                if let Some(best_shift) = Multiplier::<MULTIPLIER>::best_in_range(shift_end, without_shift, used_values) {
                    let new_score = score_without_shift + best_shift as usize * keys.len();
                    if new_score < best_score {
                        best_total_shift = shift_sum + best_shift;
                        best_score = new_score;
                    }
                }
            }
            if last { break; }
            score_without_shift += shift_end as usize * keys.len();
            for (_, sh0) in without_shift.iter_mut() {
                *sh0 += shift_end;
                if *sh0 >= slice_len {
                    *sh0 -= slice_len;
                    score_without_shift -= slice_len as usize;
                }
            }
            total_end_shift -= shift_end;
            shift_sum += shift_end;
        }
        if best_total_shift == u16::MAX {
            0
        } else {
            for key in keys {
                used_values.add(conf.slice_begin(*key) + ((*key as usize).wrapping_add(best_total_shift as usize)&conf.slice_len_minus_one as usize));
            }
            best_total_shift / MULTIPLIER as u16 + 1 
        }
    }
}

/// ShiftSeedWrapped with given number of bits per shift.
#[derive(Clone, Copy)]
pub struct ShiftSeedWrapped<const MULTIPLIER: u8>(pub u8);

impl<const MULTIPLIER: u8> SeedChooser for ShiftSeedWrapped<MULTIPLIER> {
    type UsedValues = UsedValueSet;

    fn conf(self, output_range: usize, input_size: usize, bits_per_seed: u8, bucket_size_100: u16, preferred_slice_len: u16) -> Conf {
        let max_shift = self.extra_shift(bits_per_seed);
        let slice_len = match output_range.saturating_sub(max_shift as usize) {
            n @ 0..64 => (n/2+1).next_power_of_two() as u16,
            64..1300 => 64,
            1300..1750 => 128,
            1750..7500 => 256,
            7500..150000 => 512,
            150000..250000 => 1024,
            _ => 2048,
        }.min(if preferred_slice_len != 0 { preferred_slice_len } else { 1024 });    // TODO tune 1024
        Conf::new(output_range, input_size, bucket_size_100, slice_len, max_shift)
    }

    #[inline(always)] fn f(self, primary_code: u64, seed: u16, conf: &Conf) -> usize {
        conf.slice_begin(primary_code) +
            ((mix_key_seed(primary_code, (seed>>self.0) + 1)
             + MULTIPLIER as u16 * seed) & conf.slice_len_minus_one) as usize
    }

    #[inline]
    fn best_seed(self, used_values: &mut Self::UsedValues, keys: &[u64], conf: &Conf, bits_per_seed: u8) -> u16 {
        //TODO check; what with seed=0, shift=0?
        let slice_len = conf.slice_len();
        let mut best_score = usize::MAX;
        let mut best_total_shift = u16::MAX;
        let mut best_seed = u16::MAX;
        let mut without_shift_arrayvec: arrayvec::ArrayVec::<(usize, u16), 16>;
        let mut without_shift_box: Box<[(usize, u16)]>;
        let without_shift: &mut [(usize, u16)] = if keys.len() > 16 {
            without_shift_box = keys.iter().map(|_| (0, 0)).collect();
            &mut without_shift_box
        } else {
            without_shift_arrayvec = keys.iter().map(|_| (0, 0)).collect();
            &mut without_shift_arrayvec
        };

        for seed in 0..1<<(bits_per_seed - self.0) {
            for ((slice_begin, in_slice), key) in without_shift.iter_mut().zip(keys) {
                *slice_begin = conf.slice_begin(*key);
                *in_slice = mix_key_seed(*key, seed+1).wrapping_add((MULTIPLIER as u16*seed) << self.0);
                if seed == 0 { *in_slice = in_slice.wrapping_add(MULTIPLIER as u16); }   // minimal shift for seed = 0 is 1
                *in_slice &= conf.slice_len_minus_one;
            }
            let mut score_without_shift: usize = without_shift.iter().map(|(sb, is)| *sb + *is as usize).sum();
            let mut total_end_shift = (1u16 << self.0) * MULTIPLIER as u16;
            if seed == 0 { total_end_shift -= MULTIPLIER as u16; }
            let mut shift_sum = 0; //if seed == 0 { MULTIPLIER as u16 } else { 0 };
            loop {  // while total_last_shift > 0
                let max_sh0 = without_shift.iter().map(|(_, sh0)| *sh0).max().unwrap();
                let mut shift_end = Multiplier::<MULTIPLIER>::multiple_rounded_up(slice_len - max_sh0);
                let last = shift_end >= total_end_shift;
                if last { shift_end = total_end_shift; }
                if score_without_shift < best_score {
                    if let Some(best_shift) = Multiplier::<MULTIPLIER>::best_in_range(shift_end, without_shift, used_values) {
                        let new_score = score_without_shift + best_shift as usize * keys.len();
                        if new_score < best_score {
                            best_total_shift = shift_sum + best_shift;
                            if seed == 0 { best_total_shift += MULTIPLIER as u16; }
                            best_seed = seed;
                            best_score = new_score;
                        }
                    }
                }
                if last { break; }
                score_without_shift += shift_end as usize * keys.len();
                for (_, sh0) in without_shift.iter_mut() {
                    *sh0 += shift_end;
                    if *sh0 >= slice_len {
                        *sh0 -= slice_len;
                        score_without_shift -= slice_len as usize;
                    }
                }
                total_end_shift -= shift_end;
                shift_sum += shift_end;
            }
        }
        if best_total_shift == u16::MAX {
            0
        } else {
            let result = (best_seed << self.0) | (best_total_shift / MULTIPLIER as u16);
            for key in keys {
                used_values.add(conf.slice_begin(*key) +
                    ((mix_key_seed(*key, best_seed + 1)
                    + MULTIPLIER as u16 * result) & conf.slice_len_minus_one) as usize
                );
            }
            result
        }
    }
}

/*pub struct ShiftSeedWrapped<const BITS_PER_SEED: u8, const MULTIPLIER: u8>;

impl<const BITS_PER_SEED: u8, const MULTIPLIER: u8> SeedChooser for ShiftSeedWrapped<BITS_PER_SEED, MULTIPLIER> {
    #[inline(always)] fn f<SS: SeedSize>(primary_code: u64, seed: u16, conf: &Conf<SS>) -> usize {
        let shift  = (seed >> BITS_PER_SEED) * MULTIPLIER as u16;
        let seed = (seed & ((1<<BITS_PER_SEED)-1)) + 1;
        conf.slice_begin(primary_code) + conf.in_slice_seed_shift(primary_code, seed, shift)
    }

    #[inline(always)]
    fn best_seed<SS: SeedSize>(used_values: &mut UsedValues, keys: &[u64], conf: &Conf<SS>) -> u16 {
        let mut best_seed = 0;
        let mut best_value = usize::MAX;
        if keys.len() <= SMALL_BUCKET_LIMIT {
            best_seed_small::<Self, _>(&mut best_value, &mut best_seed, used_values, keys, conf)
        } else {
            best_seed_big::<Self, _>(&mut best_value, &mut best_seed, used_values, keys, conf)
        };
        if best_seed != 0 { // can assign seed to the bucket
            for key in keys {
                used_values.add(Self::f(*key, best_seed, conf));
            }
        };
        best_seed
    }
}*/