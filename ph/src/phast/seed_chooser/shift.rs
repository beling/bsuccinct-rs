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
    
    #[inline(always)] fn seed_chooser(&self, _bits_per_seed: u8, _slice_len: u16, _loading_factor_1000: u16) -> Self::SeedChooser {
        *self
    }

    type Core = ShiftCore;

    #[inline(always)] fn core(&self) -> Self::Core { ShiftCore }

    /// Returns the bucket-evaluator weights tuned for [`ShiftOnly`]
    /// and given `bits_per_seed` and `slice_len`.
    fn bucket_evaluator(&self, bits_per_seed: u8, slice_len: u16, _loading_factor_1000: u16) -> Weights {
        #[cfg(not(feature = "W256"))]    // W = 512
        return Weights(
            if slice_len <= 256 { match (bits_per_seed, slice_len) {
                (..=6, ..=128) => [0, 665916, 718276, 741340, 758508, 769344, 1579680],   // 6, 3.0, 128, 7.86%
                (..=6, _) => [0, 535928, 707496, 768588, 807888, 833952, 1474736], // 6, 3.0, slice=256, 7.51%
                (_, ..=128) => [0, 923292, 965844, 992700, 1010024, 1024844, 1438672], // 7, 3.5, slice=128, 7.37%
                (..=7, _) => [0, 670952, 740560, 775876, 799576, 816124, 827636],  // 7, 3.5, slice=256, 4.92%
                (_, _) => [0, 680600, 742468, 773428, 795708, 813900, 824212],   // 8, 4.1, slice=256, 4.76%
            }} else { match (bits_per_seed, slice_len) {
                (..=7, ..=512) => [0, 554816, 819420, 909192, 960376, 999692, 1025980],  // 7, 3.5, slice=512, 5.31%
                (..=7, ..=1024) => [0, 914924, 1537628, 1768980, 1904136, 1981996, 2057972],  // 7, 3.5, slice=1024, 5.76%
                (8, ..=512) => [0, 594612, 713304, 767064, 803724, 835948, 855288], // 8, 4.1, slice=512, 3.32%
                (8, ..=1024) => [0, 952100, 1423216, 1575840, 1661176, 1734184, 1777776],  // 8, 4.1, slice=1024, 4.12%
                (..=8, ..=2048) => [0, 212, 1053292, 1528276, 1647252, 1829556, 1957968],  // 8, 4.1, slice=2048, 5.26%
                (9, ..=512) => [0, 560660, 787124, 864428, 906420, 945944, 960956], // 9, 5.1, slice=512, 4.12%
                (9, ..=1024) => [0, 444600, 759172, 873708, 945292, 998448, 1031512],  // 5.1, slice=1024, 3.27%
                (9, ..=2048) => [0, 728, 984716, 1284620, 1429080, 1562948, 1630060],  // 5.1, slice=2048, 4.43%
                (10, ..=512) => [0, 94292, 1013844, 1177704, 1217460, 1263864, 1264752], // 5.7, slice=512, 5.69%
                (10, ..=1024) => [0, 65148, 624792, 788112, 850732, 923184, 924736], // 5.7, slice=1024, 3.53%
                (10, ..=2048) => [0, 56760, 732436, 942508, 1063932, 1164892, 1221828],   // 5.7, slice=2048, 2.51%
                (11, ..=512) => [0, 219544, 779080, 1981260, 2089796, 2121044, 2128056],    // 6.3, slice=512, 7.05%
                (11, ..=1024) => [0, 88048, 506500, 785364, 919696, 922832, 923228],    // 6.3, slice=1024, 5.47%
                (11, ..=2048) => [0, 872, 1109868, 1421680, 1487420, 1612232, 1614256],  // 6.3, slice=2048, 3.26%
                (11, ..=4096) => [0, 12920, 644196, 1299280, 1345104, 1546348, 1648072], // 6.3, slice=4096, 2.09%
                (..=11, _) => [0, 30644, 153536, 695696, 1211644, 1540704, 6142376],    // 11, 6.3, slice=8192, 3.91%
                (_, ..=512) => [0, 333664, 657192, 2300232, 2350012, 2452336, 2456760], // 12, 6.8, slice=512, 7.90%
                (_, ..=1024) => [0, 58184, 64152, 1708924, 1828084, 1913724, 1917552], // 12, 6.8, slice=1024, 6.55%
                (_, ..=2048) => [0, 56516, 292220, 789964, 890284, 1013576, 1013932],   // 12, 6.8, slice=2048, 4.82%
                (_, ..=4096) => [0, 7088, 69800, 623868, 767832, 1051484, 1052020],  // 12, 6.8, slice=4096, 2.80%
                (_, _) => [0, 43936, 340208, 972124, 1374116, 1676744, 6631912],   // 12, 6.8, slice=8192, 1.66%
            }
        });

        #[cfg(feature = "W256")]
        return Weights(
            if slice_len <= 256 { match (bits_per_seed, slice_len) {
                (..=6, ..=128) => [-393756, 272160, 324520, 347584, 364752, 375588, 1185924],   // 6, 3.0, 128
                (..=6, _) => [-327920, 202080, 363268, 427588, 465888, 495748, 1149120], // 6, 3.0, slice=256
                (_, ..=128) => [-692652, 235668, 295704, 333692, 352888, 368672, 827032], // 8, 4.1, slice=128
                (..=7, _) => [-343908, 326124, 395348, 430344, 453332, 470840, 482624],  // 7, 3.5, slice=256
                (_, _) => [-343148, 336432, 398212, 429164, 451436, 469508, 479860],   // 8, 4.1, slice=256
            }} else { match (bits_per_seed, slice_len) {
                (..=7, ..=512) => [-383336, 153996, 412140, 499024, 550412, 591356, 621792],  // 7, 3.5, slice=512
                (_, ..=512) => [-272548, 322064, 440756, 494516, 531176, 563400, 582740], // 8, 4.1, slice=512
                (..=8, ..=1024) => [-199104, 114440, 482056, 619904, 709312, 773996, 819744],  // 8, 4.1, slice=1024
                (..=8, ..=2048) => [-56056, -47704, 254792, 579508, 776224, 1414372, 1441352],  // 8, 4.1, slice=2048
                (9, ..=1024) => [-241756, 196828, 487400, 596724, 666852, 716724, 751260],  // 5.1, slice=1024
                (9, ..=2048) => [192672, 193312, 529772, 791184, 938172, 1041432, 1116656],  // 5.1, slice=2048
                (10, ..=1024) => [-19036, 39720, 351696, 500328, 573232, 661840, 660380], // 5.7, slice=1024
                (10, ..=2048) => [-13676, 32168, 395440, 581716, 705732, 794152, 857764],   // 5.7, slice=2048
                (_, ..=1024) => [-6240, 102220, 385292, 627164, 758752, 805260, 795312],    // 6.3, slice=1024
                (11, ..=2048) => [-1176, 9200, 647824, 909672, 1113328, 1378148, 1370904],  // 6.3, slice=2048
                (11, ..=4096) => [-10696, 76776, 149240, 445712, 669772, 821700, 945876], // 6.3, slice=4096
                (_, ..=2048) => [-7656, 43892, 280900, 692488, 963520, 1223000, 1173280],   // 12, 6.8, slice=2048
                (_, ..=4096) => [-10604, -1788, 64424, 654720, 895820, 1415252, 1357084],  // 12, 6.8, slice=4096
                (_, _) => [-17236, -1948, 86648, 104380, 333480, 628252, 2175372],   // 12, 6.8, slice=8192
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

