use crate::phast::{conf::mix_key_seed, cyclic::{GenericUsedValue, UsedValueSet}, Params};

use super::conf::Conf;

pub(crate) fn slice_len(output_without_shift_range: usize, bits_per_seed: u8, preferred_slice_len: u16) -> u16 {
    match output_without_shift_range {
            n @ 0..64 => (n/2+1).next_power_of_two() as u16,
            64..1300 => 64,
            1300..1750 => 128,
            1750..7500 => 256,
            7500..150000 => 512,
            _ if bits_per_seed < 6 => if preferred_slice_len == 0 { 512 } else { preferred_slice_len },
            _ => if preferred_slice_len == 0 { 1024 } else { preferred_slice_len }
        }
}

/// Choose best seed in bucket.
pub trait SeedChooser: Copy {
    /// Specifies whether bumping is allowed.
    const BUMPING: bool = true;

    /// The lowest seed that does not indicate bumping.
    const FIRST_SEED: u16 = if Self::BUMPING { 1 } else { 0 };

    type UsedValues: GenericUsedValue;

    /// Returns maximum number of keys mapped to each output value; `k` of `k`-perfect function.
    #[inline(always)] fn k(self) -> u8 { 1 }

    /// Returns output range of minimal (perfect or k-perfect) function for given number of keys.
    #[inline(always)] fn minimal_output_range(self, num_of_keys: usize) -> usize { num_of_keys }

    fn conf(self, output_range: usize, input_size: usize, bits_per_seed: u8, bucket_size_100: u16, preferred_slice_len: u16) -> Conf {
        let max_shift = self.extra_shift(bits_per_seed);
        let slice_len = slice_len(output_range.saturating_sub(max_shift as usize), bits_per_seed.into(), preferred_slice_len);
        Conf::new(output_range, input_size, bucket_size_100, slice_len, max_shift)
    }

    #[inline(always)] fn conf_for_minimal(self, num_of_keys: usize, bits_per_seed: u8, bucket_size_100: u16, preferred_slice_len: u16) -> Conf {
        self.conf(self.minimal_output_range(num_of_keys), num_of_keys, bits_per_seed, bucket_size_100, preferred_slice_len)
    }

    #[inline(always)] fn conf_for_minimal_p<SS: Copy+Into<u8>>(self, num_of_keys: usize, params: &Params<SS>) -> Conf {
        self.conf_for_minimal(num_of_keys, params.seed_size.into(), params.bucket_size100, params.preferred_slice_len)
    }

    /// How much the chooser can add to value over slice length.
    #[inline(always)] fn extra_shift(self, _bits_per_seed: u8) -> u16 { 0 }

    /// Returns function value for given primary code and seed.
    fn f(self, primary_code: u64, seed: u16, conf: &Conf) -> usize;
    
    /// Returns best seed to store in seeds array or `u16::MAX` if `NO_BUMPING` is `true` and there is no feasible seed.
    fn best_seed(self, used_values: &mut Self::UsedValues, keys: &[u64], conf: &Conf, bits_per_seed: u8) -> u16;
}

#[inline(always)]
fn best_seed_big<SC: SeedChooser>(seed_chooser: SC, best_value: &mut usize, best_seed: &mut u16, used_values: &mut UsedValueSet, keys: &[u64], conf: &Conf, seeds_num: u16) {
    let mut values_used_by_seed = Vec::with_capacity(keys.len());
    let simd_keys = keys.len() / 4 * 4;
    //assert!(simd_keys <= keys.len());
    'outer: for seed in SC::FIRST_SEED..seeds_num {    // seed=0 is special = no seed,
        values_used_by_seed.clear();
        for i in (0..simd_keys).step_by(4) {
            let values = [
                seed_chooser.f(keys[i], seed, conf),
                seed_chooser.f(keys[i+1], seed, conf),
                seed_chooser.f(keys[i+2], seed, conf),
                seed_chooser.f(keys[i+3], seed, conf),
            ];
            let contains = [
                used_values.contain(values[0]),
                used_values.contain(values[1]),
                used_values.contain(values[2]),
                used_values.contain(values[3]),
            ];
            if contains.iter().any(|b| *b) { continue 'outer; }
            //if contains[0] || contains[1] || contains[2] || contains[3] { continue 'outer; }
            values_used_by_seed.push(values[0]);
            values_used_by_seed.push(values[1]);
            values_used_by_seed.push(values[2]);
            values_used_by_seed.push(values[3]);
        }
        //assert!(keys.len() - simd_keys < 4);
        for i in simd_keys..keys.len() {
            let value = seed_chooser.f(keys[i], seed, conf);
            if used_values.contain(value) { continue 'outer; }
            values_used_by_seed.push(value);
        }
        let seed_value = values_used_by_seed.iter().sum();
        if seed_value < *best_value {
            values_used_by_seed.sort();
            if values_used_by_seed.windows(2).any(|v| v[0]==v[1]) {
                //SELF_COLLISION_KEYS.fetch_add(keys.len() as u64, std::sync::atomic::Ordering::Relaxed);
                //SELF_COLLISION_BUCKETS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                //if SC::BUMPING { return; }
                continue;
            }
            *best_value = seed_value;
            *best_seed = seed;
        }
    }
}

#[inline(always)]
fn best_seed_small<SC: SeedChooser>(seed_chooser: SC, best_value: &mut usize, best_seed: &mut u16, used_values: &mut UsedValueSet, keys: &[u64], conf: &Conf, seeds_num: u16) {
    assert!(keys.len() <= SMALL_BUCKET_LIMIT);  // seems to speeds up a bit
    let mut values_used_by_seed = arrayvec::ArrayVec::<_, SMALL_BUCKET_LIMIT>::new(); // Vec::with_capacity(keys.len());
    'outer: for seed in SC::FIRST_SEED..seeds_num {    // seed=0 is special = no seed,
        values_used_by_seed.clear();
        for key in keys.iter().copied() {
            let value = seed_chooser.f(key, seed, conf);
            if used_values.contain(value) { continue 'outer; }
            values_used_by_seed.push(value);
        }
        let seed_value = values_used_by_seed.iter().sum();
        if seed_value < *best_value {
            values_used_by_seed.sort_unstable();
            for i in 1..values_used_by_seed.len() {
                if values_used_by_seed[i-1] == values_used_by_seed[i] {
                    //SELF_COLLISION_KEYS.fetch_add(keys.len() as u64, std::sync::atomic::Ordering::Relaxed);
                    //SELF_COLLISION_BUCKETS.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    //if SC::BUMPING { return; }
                    continue 'outer;
                }
            }
            *best_value = seed_value;
            *best_seed = seed;
        }
    }
}

const SMALL_BUCKET_LIMIT: usize = 8;

/// Choose best seed without shift component.
#[derive(Clone, Copy)]
pub struct SeedOnly;

impl SeedChooser for SeedOnly {
    type UsedValues = UsedValueSet;
    
    #[inline(always)] fn f(self, primary_code: u64, seed: u16, conf: &Conf) -> usize {
        conf.f(primary_code, seed)
    }

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
                used_values.add(conf.f(*key, best_seed));
            }
        };
        best_seed
    }
}

/// Choose best seed without shift component.
#[derive(Clone, Copy)]
pub struct SeedOnlyNoBump;

impl SeedChooser for SeedOnlyNoBump {
    const BUMPING: bool = false;
    const FIRST_SEED: u16 = 0;

    type UsedValues = UsedValueSet;

    #[inline(always)] fn f(self, primary_code: u64, seed: u16, conf: &Conf) -> usize {
        conf.f_nobump(primary_code, seed)
    }

    #[inline]
    fn best_seed(self, used_values: &mut Self::UsedValues, keys: &[u64], conf: &Conf, bits_per_seed: u8) -> u16 {
        //let _: [(); Self::FIRST_SEED as usize] = [];
        let mut best_seed = u16::MAX;
        let mut best_value = usize::MAX;
        if keys.len() <= SMALL_BUCKET_LIMIT {
            best_seed_small(self, &mut best_value, &mut best_seed, used_values, keys, conf, 1<<bits_per_seed)
        } else {
            best_seed_big(self, &mut best_value, &mut best_seed, used_values, keys, conf, 1<<bits_per_seed)
        };
        if best_seed != u16::MAX { // can assign seed to the bucket
            for key in keys {
                used_values.add(conf.f_nobump(*key, best_seed));
            }
        };
        best_seed
    }
}

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
pub struct ShiftOnly<const MULTIPLIER: u8, const L: u16 = 1024, const L_LARGE_SEEDS: u16 = 1024>;

//pub static SELF_COLLISION_KEYS: AtomicU64 = AtomicU64::new(0);
//pub static SELF_COLLISION_BUCKETS: AtomicU64 = AtomicU64::new(0);

impl<const MULTIPLIER: u8, const L: u16, const L_LARGE_SEEDS: u16> SeedChooser for ShiftOnly<MULTIPLIER, L, L_LARGE_SEEDS> {
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
            _ => if preferred_slice_len == 0 { 2048 } else { preferred_slice_len },
        }.min(if bits_per_seed <= 8 { L } else { L_LARGE_SEEDS });
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

pub type ShiftOnlyX1 = ShiftOnly<1, 512, 1024>;
pub type ShiftOnlyX2 = ShiftOnly<2>;
pub type ShiftOnlyX3 = ShiftOnly<3>;
pub type ShiftOnlyX4 = ShiftOnly<4>;


#[derive(Clone, Copy)]
pub struct ShiftOnlyWrapped<const MULTIPLIER: u8>;

impl<const MULTIPLIER: u8> SeedChooser for ShiftOnlyWrapped<MULTIPLIER> {
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
        }.min(if preferred_slice_len != 0 { preferred_slice_len } else { match MULTIPLIER {
            1 => match bits_per_seed {
                1..=5 => 256,
                6..=7 => 512,   // or 6 => 256 for smaller size
                8..=9 => 1024,   // or 8 => 512 for smaller size
                _ => 2048   // or 10 => 1024 for smaller size
            },
            2 => match bits_per_seed {
                1..=5 => 256,
                6..=7 => 512,
                8 => 1024,
                _ => 2048
            },
            _ => match bits_per_seed {
                7 => 1024,
                1..=4 => 256,
                5..=7 => 512,
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
pub struct ShiftSeedWrapped<const MULTIPLIER: u8, const L: u16 = 1024, const L_LARGE_SEEDS: u16 = 1024>(pub u8);

impl<const MULTIPLIER: u8, const L: u16, const L_LARGE_SEEDS: u16> SeedChooser for ShiftSeedWrapped<MULTIPLIER, L, L_LARGE_SEEDS> {
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
        }.min(if bits_per_seed <= 8 { L } else { L_LARGE_SEEDS });
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