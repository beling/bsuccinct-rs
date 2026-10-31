//! [`ShiftOnly`] – the PHast+ seed chooser without wrapping.

use std::io;

use crate::phast::{SeedChooserCore, Weights, conf::Core, cyclic::{CyclicSet, UsedValueSetLarge}, seed_chooser::SeedChooserConf};
use super::SeedChooser;

/// Returns whether two (or more) keys share the same value
/// (i.e. whether the bucket would collide with itself).
/// The result is shift-independent.
/// The slice can be sorted in place.
#[inline] fn self_collide(without_shift: &mut [usize]) -> bool {
    without_shift.sort_unstable();  // maybe it is better to postpone self-collision test?
    for i in 1..without_shift.len() {
        if without_shift[i-1] == without_shift[i] { // self-collision?
            return true;
        }
    }
    false
}   //TODO try not to sort if without_shift is small

/// Returns the values that the `keys` would be assigned with shift equal to `0`
/// (i.e. by the seed-independent part of the mapping).
#[inline] fn shifts0<'k, 'c, C: Core>(keys: &'k [u64], conf: &'c C) -> impl Iterator<Item = usize> + use<'k, 'c, C> {
    keys.iter().map(|key| conf.f_shift0(*key))
}

/// Returns the bits of `used_values` (starting from the shifted values) occupied by the given `shift`,
/// merged (OR-ed) into `excluded`; a `1` bit means that the value is already taken.
#[inline] fn occupy_sum<const UVS: usize>(mut excluded: u64, used_values: &CyclicSet<UVS>, without_shift: &[usize], shift: u16) -> u64 {
    for first in without_shift.iter() {
        excluded |= used_values.get64(*first + shift as usize);
    }
    excluded
}

/// Marks the values (shifted by `total_shift`) as used.
#[inline] fn mark_used<const UVS: usize>(used_values: &mut CyclicSet<UVS>, without_shift: &[usize], total_shift: u16) {
    for first in without_shift {
        used_values.add(*first + total_shift as usize);
    }
}

/// [`SeedChooserCore`] of PHast+ without wrapping ([`ShiftOnly`]):
/// the seed is used only as a shift added to the seed-independent value of a key.
#[derive(Clone, Copy)]
pub struct ShiftCore;

impl SeedChooserCore for ShiftCore {
    const FUNCTION2_THRESHOLD: usize = 8192;

    /// Returns the value of the function for given `key` and `seed`
    /// (i.e. the value with the shift component equal to `seed-1`).
    #[inline(always)] fn f<C: Core>(&self, key: u64, seed: u16, conf: &C) -> usize {
        conf.f_shift0(key) + (seed-1) as usize
    }

    /// Returns the maximal shift that the chooser can add to a value over the slice length.
    #[inline(always)] fn extra_shift(&self, bits_per_seed: u8) -> u16 {
        (1 << bits_per_seed) - 2
    }

    /// Read `Self` from the `input`.
    #[inline(always)] fn read(_input: &mut dyn io::Read) -> io::Result<Self> { Ok(Self) }
}

/// [`SeedChooser`] to build (1-)perfect functions called *PHast+ without wrapping*.
/// 
/// Must be used with [`Function2`](crate::phast::Function2).
/// 
/// It chooses best seed using only shifting without wrapping,
/// which leads to very fast construction but the cost of bigger size.
#[derive(Clone, Copy, Default)]
pub struct ShiftOnly;


impl SeedChooserConf for ShiftOnly {

    type SeedChooser = Self;

    type BucketEvaluator = Weights;

    type UsedValues = UsedValueSetLarge;
    
    #[inline(always)] fn seed_chooser(&self, _bits_per_seed: u8, _slice_len: u16) -> Self::SeedChooser {
        *self
    }

    type Core = ShiftCore;

    #[inline(always)] fn core(&self) -> Self::Core { ShiftCore }

    /// Returns the bucket-evaluator weights tuned for [`ShiftOnly`]
    /// and given `bits_per_seed` and `slice_len`.
    fn bucket_evaluator(&self, bits_per_seed: u8, slice_len: u16) -> Weights {
        #[cfg(not(feature = "W256"))]    // W = 512
        return Weights(
            if slice_len <= 256 { match (bits_per_seed, slice_len) {
                (..=6, ..=128) => [0, 166479, 179569, 185335, 189627, 192336, 394920],   // 6, 3.0, 128, 7.86%
                (..=6, _) => [0, 133982, 176874, 192147, 201972, 208488, 368684], // 6, 3.0, slice=256, 7.51%
                (_, ..=128) => [-173163, 58917, 73926, 83423, 88222, 92168, 206758], // 8, 4.1, slice=128
                (..=7, _) => [0, 167738, 185140, 193969, 199894, 204031, 206909],  // 7, 3.5, slice=256, 4.92%
                (_, _) => [0, 170150, 185617, 193357, 198927, 203475, 206053],   // 8, 4.1, slice=256, 4.76%
            }} else { match (bits_per_seed, slice_len) {
                (..=7, ..=512) => [0, 138704, 204855, 227298, 240094, 249923, 256495],  // 7, 3.5, slice=512, 5.31%
                (..=7, ..=1024) => [0, 228731, 384407, 442245, 476034, 495499, 514493],  // 7, 3.5, slice=1024, 5.76%
                (8, ..=512) => [0, 148653, 178326, 191766, 200931, 208987, 213822], // 8, 4.1, slice=512, 3.32%
                (8, ..=1024) => [0, 238025, 355804, 393960, 415294, 433546, 444444],  // 8, 4.1, slice=1024, 4.12%
                (..=8, ..=2048) => [0, 53, 263323, 382069, 411813, 457389, 489492],  // 8, 4.1, slice=2048, 5.26%
                (9, ..=512) => [0, 140165, 196781, 216107, 226605, 236486, 240239], // 9, 5.1, slice=512, 4.12%
                (9, ..=1024) => [0, 111150, 189793, 218427, 236323, 249612, 257878],  // 5.1, slice=1024, 3.27%
                (9, ..=2048) => [0, 182, 246179, 321155, 357270, 390737, 407515],  // 5.1, slice=2048, 4.43%
                (10, ..=512) => [0, 23573, 253461, 294426, 304365, 315966, 316188], // 5.7, slice=512, 5.69%
                (10, ..=1024) => [0, 16287, 156198, 197028, 212683, 230796, 231184], // 5.7, slice=1024, 3.53%
                (10, ..=2048) => [0, 14190, 183109, 235627, 265983, 291223, 305457],   // 5.7, slice=2048, 2.51%
                (11, ..=512) => [0, 54886, 194770, 495315, 522449, 530261, 532014],    // 6.3, slice=512, 7.05%
                (11, ..=1024) => [0, 22012, 126625, 196341, 229924, 230708, 230807],    // 6.3, slice=1024, 5.47%
                (11, ..=2048) => [0, 218, 277467, 355420, 371855, 403058, 403564],  // 6.3, slice=2048, 3.26%
                (11, ..=4096) => [0, 3230, 161049, 324820, 336276, 386587, 412018], // 6.3, slice=4096, 2.09%
                (..=11, _) => [0, 7661, 38384, 173924, 302911, 385176, 1535594],    // 11, 6.3, slice=8192, 3.91%
                (_, ..=512) => [0, 83416, 164298, 575058, 587503, 613084, 614190], // 12, 6.8, slice=512, 7.90%
                (_, ..=1024) => [0, 14546, 16038, 427231, 457021, 478431, 479388], // 12, 6.8, slice=1024, 6.55%
                (_, ..=2048) => [0, 14129, 73055, 197491, 222571, 253394, 253483],   // 12, 6.8, slice=2048, 4.82%
                (_, ..=4096) => [0, 1772, 17450, 155967, 191958, 262871, 263005],  // 12, 6.8, slice=4096, 2.80%
                (_, _) => [0, 10984, 85052, 243031, 343529, 419186, 1657978],   // 12, 6.8, slice=8192, 1.66%
            }
        });

        #[cfg(feature = "W256")]
        return Weights(
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
        });
    }   // TODO these are tuned for WINDOW_SIZE=256

    #[inline] fn empty_used_values(&self) -> Self::UsedValues { Default::default() }

    #[inline(always)] fn add_used(&self, used_values: &mut Self::UsedValues, value: usize) { used_values.add(value); }

    #[inline(always)] fn clear_used(&self, used_values: &mut Self::UsedValues, value: usize) { used_values.remove(value); }

    #[allow(clippy::match_overlapping_arm)] // ranges overlap on purpose: the first matching arm has the priority
    #[inline(always)] fn slice_len(&self, output_range: usize, bits_per_seed: u8, preferred_slice_len: u16) -> u16 {
        match output_range.saturating_sub(self.extra_shift(bits_per_seed) as usize) {
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
        })
    }
}

impl SeedChooser for ShiftOnly {

    /// Returns the lowest shift (increased by one) that does not make the bucket collide with the values used
    /// by other processed buckets, or `0` (bumping) if there is no such shift.
    #[inline]
    fn best_seed<C: Core>(&self, used_values: &mut Self::UsedValues, keys: &[u64], conf: &C, bits_per_seed: u8, _bucket_nr: usize, _first_bucket_in_window: usize) -> u16 {
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
            let used = occupy_sum(0, used_values, without_shift, shift);
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

