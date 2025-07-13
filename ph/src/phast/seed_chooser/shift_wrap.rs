use crate::phast::{conf::{mix_key_seed, Conf}, cyclic::{GenericUsedValue, UsedValueSet}, seed_chooser::shift::Multiplier, Weights};
use super::SeedChooser;

#[derive(Clone, Copy)]
pub struct ShiftOnlyWrapped<const MULTIPLIER: u8>;

fn shift_only_wrapped_bucket_evaluator_m1(bits_per_seed: u8, slice_len: u16) -> [i32; 7] {
    match (bits_per_seed, slice_len) {
        (_, ..=64) => [-76520, 97960, 103626, 106759, 109053, 110149, 112662],   // 8, 4.1, 64
        (_, ..=128) => [-80872, 90492, 100641, 105939, 109960, 112290, 118119], // 8, 4.1, 128
        (..=6, ..=256) => [-76632, 59701, 89939, 103115, 111040, 117906, 283652], // 6, 3.0, slice=256
        (..=6, ..=512) => [-102171, 30195, 95877, 122987, 138980, 152173, 206055],  // 6, 3.0, slice=512
        (_, ..=256) => [-84425, 81165, 96951, 106065, 112137, 117421, 122309],  // 7, 3.5, slice=256
        (7, ..=512) => [-69271, 61152, 101770, 119869, 132454, 141236, 148273],  // 7, 3.5, slice=512
        (8, ..=512) => [-66903, 81776, 107154, 122354, 132033, 140641, 146584], // 4.1, slice=512
        (8, ..=1024) => [-50666, 55977, 116129, 145446, 164172, 180129, 192120],  // 4.1, slice=1024
        (_, ..=512) => [-45845, 91690, 122225, 138169, 149160, 155706, 164757],  // 5.1, slice=512
        (9, ..=1024) => [-51695, 68190, 121468, 146481, 164082, 178054, 186488],  // 5.1, slice=1024
        (..=9, ..=2048) => [-3365, 12300, 85113, 138418, 170087, 197668, 215654],  // 9, 5.1, slice=2048
        (10, ..=1024) => [-4011, 15045, 106558, 112844, 133305, 145623, 154991], // 5.7, slice=1024
        (..=10, ..=2048) => [-3301, 12449, 83323, 139924, 169323, 198105, 212187],   // 10, 5.7, slice=2048
        (11, ..=1024) => [-1524, 23928, 115028, 153353, 187370, 191075, 197861],    // 6.3, slice=1024 USELESS
        (11, ..=2048) => [-1777, 22788, 106158, 139632, 174143, 200775, 214797],  // 6.3, slice=2048
        (11, _) => [-4924, 19116, 22394, 59714, 110668, 154482, 181404],
        (_, ..=1024) => [-2190, 30393, 114587, 141471, 162103, 177602, 183787], // 12, 6.8, slice=1024 USELESS
        (_, ..=2048) => [-2355, 16099, 113987, 153868, 183912, 213486, 226897],   // 12, 6.8, slice=2048 USELESS
        (_, _) => [-3938, 38130, 29589, 52311, 96328, 147014, 172193], // 12, 6.8, 4096
    }
}

fn shift_only_wrapped_bucket_evaluator_m2(bits_per_seed: u8, slice_len: u16) -> [i32; 7] {
    match (bits_per_seed, slice_len) {
        (_, ..=64) => [-78586, 98418, 103824, 106532, 108539, 109981, 111063],  // 8, 4.1, 64
        (_, ..=128) => [-85534, 90593, 100261, 105362, 108728, 111329, 113115], // 8, 4.1, 128
        (..=6, ..=256) => [-113309, 70659, 92719, 103205, 111784, 117218, 121395], // 6, 3.0, slice=256
        (..=6, ..=512) => [-113437, 36479, 87740, 109716, 124793, 137012, 209528], // 6, 3.0, slice=512
        (_, ..=256) => [-83108, 76805, 93889, 104574, 111919, 117701, 137200],  // 7, 3.5, slice=256
        (7, ..=512) => [-11364, 71851, 100238, 116988, 128732, 138656, 145275],  // 7, 3.5, slice=512
        (8, ..=512) => [-67763, 78133, 104489, 121464, 133392, 140946, 155107], // 4.1, slice=512
        (8, ..=1024) => [-50137, 65904, 111782, 139890, 159029, 175922, 186995],  // 4.1, slice=1024
        (..=8, ..=2048) => [-3445, 11224, 85129, 138005, 176794, 209479, 234058], // 8, 4.1, slice=2048
        (_, ..=512) => [-45845, 91690, 122225, 138169, 149160, 155706, 164757],  // 5.1, slice=512
        (9, ..=1024) => [-49692, 70537, 115707, 143201, 163216, 178981, 188448],  // 5.1, slice=1024
        (..=9, ..=2048) => [-3514, 12002, 85880, 136849, 171127, 197699, 215787],  // 9, 5.1, slice=2048
        (10, ..=1024) => [-4383, 16783, 82867, 112554, 131931, 148281, 156013], // 5.7, slice=1024 USELESS
        (..=10, ..=2048) => [-3386, 13051, 82525, 133516, 169004, 198445, 214518],   // 10, 5.7, slice=2048
        (11, ..=1024) => [-1562, 24796, 122828, 155722, 174139, 191420, 198085],    // 6.3, slice=1024 USELESS
        (11, ..=2048) => [-2243, 21043, 83146, 136599, 172186, 200713, 215298],  // 6.3, slice=2048 USELESS
        (11, _) => [-4045, 8964, 9362, 21128, 86855, 136683, 166640],   // 11, 6.3, slice=4096
        (_, ..=1024) => [-2244, 32777, 107196, 142051, 161424, 177763, 183475], // 12, 6.8, slice=1024 USELESS
        (_, ..=2048) => [-2808, 16026, 90346, 150206, 185508, 214963, 227887],   // 12, 6.8, slice=2048 USELESS
        (_, _ /*..=4096*/) => [-4044, 7164, 10158, 22096, 92563, 142914, 171123],    // 12, 6.8, slice=4096  USELESS?? pure performance
        //(_, _) => [-4849, 12371, 19420, 27337, 28560, 51301, 103428]    // 12, 6.8, slice=8192, TODO optimize
    }
}

fn shift_only_wrapped_bucket_evaluator_m3(bits_per_seed: u8, slice_len: u16) -> [i32; 7] {
    match (bits_per_seed, slice_len) { // multiplier=3, almost the same result as for multiplier=2 weights
        (_, ..=64) => [-81342, 97738, 103193, 106305, 108524, 109876, 112382],  // 8, 4.1, 64
        (_, ..=128) => [-82883, 89250, 99246, 105030, 108983, 111224, 117058], // 8, 4.1, 128
        (..=6, ..=256) => [-143420, 70364, 89794, 100431, 107778, 113842, 253543], // 6, 3.0, slice=256
        (..=6, ..=512) => [-118906, 41451, 83177, 104570, 119520, 131788, 197543], // 6, 3.0, slice=512
        (_, ..=256) => [-82828, 77192, 94710, 105243, 112716, 118768, 136225],  // 7, 3.5, slice=256
        (7, ..=512) => [-11540, 68580, 98218, 115370, 128607, 139118, 145832],  // 7, 3.5, slice=512
        (_, ..=512) => [25100, 89361, 117113, 134755, 147369, 154606, 172378], // 4.1, slice=512
        (8, ..=1024) => [-50649, 63792, 110014, 139267, 161285, 176594, 188305],  // 4.1, slice=1024
        (..=8, ..=2048) => [-3427, 10388, 90470, 141895, 179413, 208576, 232553], // 8, 4.1, slice=2048
        (9, ..=1024) => [-41757, 60279, 113069, 143467, 162892, 179091, 188139],  // 5.1, slice=1024
        (..=9, ..=2048) => [-3753, 11840, 77702, 132696, 169641, 200687, 218764],  // 9, 5.1, slice=2048
        (10, ..=1024) => [-2394, 29640, 81921, 108732, 126229, 141102, 150457], // 5.7, slice=1024 USELESS
        (..=10, ..=2048) => [-3417, 13564, 81208, 133035, 168506, 198114, 214382],   // 10, 5.7, slice=2048
        (11, ..=1024) => [-1555, 25982, 126717, 155711, 174202, 191358, 198247],    // 6.3, slice=1024 USELESS
        (11, ..=2048) => [-2229, 21208, 88554, 137643, 169905, 200075, 213746],  // 6.3, slice=2048 USELESS
        (11, _) => [-3267, 25041, 24325, 40786, 100528, 155125, 182822],  // 11, 6.3, slice=4096
        (_, ..=1024) => [-2206, 33628, 110901, 143147, 161228, 177559, 183794], // 12, 6.8, slice=1024 USELESS
        (_, ..=2048) => [-2665, 16252, 98048, 149519, 183487, 214959, 227347],   // 12, 6.8, slice=2048 USELESS
        (_, _) => [-3356, 26074, 26278, 44692, 94747, 143426, 168599],  // 12, 6.8, slice=4096  USELESS
    }
}

impl<const MULTIPLIER: u8> SeedChooser for ShiftOnlyWrapped<MULTIPLIER> {

    type UsedValues = UsedValueSet;

    //type UsedValues = UsedValueSetLarge;
    //const FUNCTION2_THRESHOLD: usize = 4096*2;

    fn bucket_evaluator(&self, bits_per_seed: u8, slice_len: u16) -> Weights {
        Weights(match MULTIPLIER {
            1 => shift_only_wrapped_bucket_evaluator_m1(bits_per_seed, slice_len),
            2 => shift_only_wrapped_bucket_evaluator_m2(bits_per_seed, slice_len),
            _ => shift_only_wrapped_bucket_evaluator_m3(bits_per_seed, slice_len)
        })
    }

    fn conf(self, output_range: usize, input_size: usize, bits_per_seed: u8, bucket_size_100: u16, preferred_slice_len: u16) -> Conf {
        let max_shift = self.extra_shift(bits_per_seed);
        let slice_len = match output_range.saturating_sub(max_shift as usize) {
            n @ 0..4096 => (n/2+1).next_power_of_two() as u16,
            /*64..1300 => 64,
            1300..1750 => 128,
            1750..7500 => 256,
            7500..150000 => 512,
            150000..250000 => 1024,*/
            //_ => 2048,
            _ => /* 2* */ 4096,
        }.min(if preferred_slice_len != 0 { preferred_slice_len } else { match MULTIPLIER {
            1 => match bits_per_seed {
                ..=5 => 256,
                ..=7 => 512,   // or 6 => 256 for smaller size
                ..=9 => 1024,   // or 8 => 512 for smaller size
                ..=11 => 2048,   // or 10 => 1024 for smaller size
                _ => 4096,
                //_ => 2*4096
            },
            2 => match bits_per_seed {
                ..=5 => 256,
                ..=7 => 512,
                8 => 1024,
                ..=10 => 2048,   // or 9 => 1024 for smaller size
                _ => 4096   // only 11, do not use 12
                //_ => 2*4096
            },
            _ => match bits_per_seed {
                ..=4 => 256,
                ..=7 => 512,
                8 => 1024,
                ..=10 => 2048,  // or (for MULTIPLIER=3) 10 => 4096 for faster construction
                _ => 4096
            },
        }});        
        Conf::new(output_range, input_size, bucket_size_100, slice_len, max_shift)
    }

    #[inline(always)] fn f(self, primary_code: u64, seed: u16, conf: &Conf) -> usize {
        conf.slice_begin(primary_code) + ((primary_code as usize).wrapping_add((seed-1) as usize*MULTIPLIER as usize) & conf.slice_len_minus_one as usize)
    }

    /*#[inline(always)] fn f_slice<SS: SeedSize>(primary_code: u64, slice_begin: usize, seed: u16, conf: &Conf<SS>) -> usize {
        slice_begin + ((primary_code as usize).wrapping_add((seed-1) as usize*MULTIPLIER as usize) & conf.slice_len_minus_one as usize)
    }*/

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