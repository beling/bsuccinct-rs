//! Shift-based seed choosers with wrapping (PHast+).

use std::io;

use binout::{AsIs, Serializer};

use crate::phast::{SeedChooserCore, Weights, conf::{Core, mix_key_seed}, cyclic::{CyclicSet, UsedValueSet}, seed_chooser::SeedChooserConf};
use super::SeedChooser;


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


#[derive(Clone, Copy)]
pub struct ShiftWrappedCore<const MULTIPLIER: u8 = 1>;

impl<const MULTIPLIER: u8> SeedChooserCore for ShiftWrappedCore<MULTIPLIER> {
    #[inline(always)] fn f<C: Core>(&self, primary_code: u64, seed: u16, conf: &C) -> usize {
        conf.slice_begin(primary_code) + ((primary_code as u16).wrapping_add(seed.wrapping_mul(MULTIPLIER as u16)) & conf.slice_len_minus_one()) as usize
        //conf.slice_begin(primary_code) + ((primary_code as usize).wrapping_add(seed as usize*MULTIPLIER as usize) & conf.slice_len_minus_one as usize)
    }

    #[inline(always)] fn read(_input: &mut dyn io::Read) -> io::Result<Self> { Ok(Self) }
}

/// [`SeedChooser`] to build (1-)perfect functions called *PHast+ with wrapping*.
/// 
/// Can be used with any function type: [`Function`](crate::phast::Function), [`Function2`](crate::phast::Function2), [`Perfect`](crate::phast::Perfect).
/// 
/// It chooses best seed using only shifting with wrapping,
/// which leads to quite small size and quite fast construction.
/// 
/// `MULTIPLIER` should be 1, 2, or 3.
/// Typically, increasing `MULTIPLIER` reduces size but slows down construction.
/// `MULTIPLIER=1` works very well with large `bits_per_seed` (10+),
/// larger values (2 and 3) works well with `bits_per_seed=8`.
#[derive(Clone, Copy)]
pub struct ShiftOnlyWrapped<const MULTIPLIER: u8 = 1>;

// TODO all weights have been found for WINDOW_SIZE = 256

/// Bucket-evaluator weights of [`ShiftOnlyWrapped`] with `MULTIPLIER` = 1.
#[cfg(not(feature = "W256"))]
fn shift_only_wrapped_bucket_evaluator_m1(bits_per_seed: u8, slice_len: u16) -> [i32; 7] {
    match (bits_per_seed, slice_len) {  // most found for 100M keys, some for 500M
        (_, ..=64) => [-306080, 391840, 414504, 427036, 436212, 440596, 450648],   // 8, 4.1, 64
        (_, ..=128) => [-323488, 361968, 402564, 423756, 439840, 449160, 472476], // 8, 4.1, 128
        (..=6, ..=256) => [-306528, 238804, 359756, 412460, 444160, 471624, 1134608], // 6, 3.0, slice=256
        (..=6, ..=512) => [-408684, 120780, 383508, 491948, 555920, 608692, 824220],  // 6, 3.0, slice=512
        (_, ..=256) => [-337700, 324660, 387804, 424260, 448548, 469684, 489236],  // 7, 3.5, slice=256
        (7, ..=512) => [-277084, 244608, 407080, 479476, 529816, 564944, 593092],  // 7, 3.5, slice=512
        (8, ..=512) => [0, 597336, 699064, 759472, 799052, 832928, 857084], // 8, 4.1, slice=512, 2.74%
        (8, ..=1024) => [0, 761036, 1059972, 1193884, 1248784, 1323876, 1370600],  // 8, 4.1, slice=1024, 2.94%; 500M
        (9, ..=512) => [0, 565112, 677012, 738548, 782876, 807472, 843804],  // 9, 5.1, slice=512, 3.39%
        (9, ..=1024) => [0, 483400, 714516, 816624, 891288, 947828, 981616],  // 9, 5.1, slice=1024, 2.54%
        (..=9, ..=2048) => [0, 56040, 760344, 987800, 1120676, 1239200, 1307672],  // 9, 5.1, slice=2048, 3.28%; 500M
        (10, ..=512) => [0, 422544, 552012, 613996, 657104, 689028, 706220],    // 10, 5.7, slice=512, 4.86%
        (10, ..=1024) => [0, 74136, 282132, 383852, 472984, 514776, 552724], // 10, 5.7, slice=1024, 2.19%
        (10, ..=2048) => [0, 61496, 493424, 688660, 809880, 917916, 976032],   // 10, 5.7, slice=2048, 1.78%; 500M
        (11, ..=512) => [0, 524960, 625364, 694140, 732004, 768616, 783208],    // 11, 6.3, slice=512, 7.02%
        (11, ..=1024) => [0, 114488, 384728, 506720, 569376, 638364, 665888],    // 11, 6.3, slice=1024, 3.48%
        (11, ..=2048) => [0, 108840, 465944, 659572, 799440, 914380, 968016],  // 11, 6.3, slice=2048, 1.40%
        (11, _) => [0, 244, 306248, 696400, 899160, 1148348, 1240836],  // 11, 6.3, slice=4096, 1.33%; 500M
        (_, ..=512) => [0, 571304, 681396, 754936, 793280, 829968, 842472], // 12, 6.8, slice=512, 9.14%
        (_, ..=1024) => [0, 138024, 473436, 594596, 673788, 735160, 759964], // 12, 6.8, slice=1024, 5.21%
        (_, ..=2048) => [0, 76444, 577304, 783696, 909632, 1037324, 1086836],   // 12, 6.8, slice=2048, 2.42%
        (_, _) => [-15752, 152520, 158356, 209244, 385312, 588056, 688772], // 12, 6.8, 4096
    }
}

/// Bucket-evaluator weights of [`ShiftOnlyWrapped`] with `MULTIPLIER` = 1.
#[cfg(feature = "W256")]
fn shift_only_wrapped_bucket_evaluator_m1(bits_per_seed: u8, slice_len: u16) -> [i32; 7] {
    match (bits_per_seed, slice_len) {
        (_, ..=64) => [-306080, 391840, 414504, 427036, 436212, 440596, 450648],   // 8, 4.1, 64
        (_, ..=128) => [-323488, 361968, 402564, 423756, 439840, 449160, 472476], // 8, 4.1, 128
        (..=6, ..=256) => [-306528, 238804, 359756, 412460, 444160, 471624, 1134608], // 6, 3.0, slice=256
        (..=6, ..=512) => [-408684, 120780, 383508, 491948, 555920, 608692, 824220],  // 6, 3.0, slice=512
        (_, ..=256) => [-337700, 324660, 387804, 424260, 448548, 469684, 489236],  // 7, 3.5, slice=256
        (7, ..=512) => [-277084, 244608, 407080, 479476, 529816, 564944, 593092],  // 7, 3.5, slice=512
        (8, ..=512) => [-267612, 327104, 428616, 489416, 528132, 562564, 586336], // 4.1, slice=512
        (8, ..=1024) => [-202664, 223908, 464516, 581784, 656688, 720516, 768480],  // 4.1, slice=1024
        (_, ..=512) => [-183380, 366760, 488900, 552676, 596640, 622824, 659028],  // 5.1, slice=512
        (9, ..=1024) => [-206780, 272760, 485872, 585924, 656328, 712216, 745952],  // 5.1, slice=1024
        (..=9, ..=2048) => [-13460, 49200, 340452, 553672, 680348, 790672, 862616],  // 9, 5.1, slice=2048
        (10, ..=1024) => [-16044, 60180, 426232, 451376, 533220, 582492, 619964], // 5.7, slice=1024
        (..=10, ..=2048) => [-13204, 49796, 333292, 559696, 677292, 792420, 848748],   // 10, 5.7, slice=2048
        (11, ..=1024) => [-6096, 95712, 460112, 613412, 749480, 764300, 791444],    // 6.3, slice=1024 USELESS
        (11, ..=2048) => [-7108, 91152, 424632, 558528, 696572, 803100, 859188],  // 6.3, slice=2048
        (11, _) => [-19696, 76464, 89576, 238856, 442672, 617928, 725616],
        (_, ..=1024) => [-8760, 121572, 458348, 565884, 648412, 710408, 735148], // 12, 6.8, slice=1024 USELESS
        (_, ..=2048) => [-9420, 64396, 455948, 615472, 735648, 853944, 907588],   // 12, 6.8, slice=2048 USELESS
        (_, _) => [-15752, 152520, 118356, 209244, 385312, 588056, 688772], // 12, 6.8, 4096
    }
}

/// Bucket-evaluator weights of [`ShiftOnlyWrapped`] with `MULTIPLIER` = 2.
#[cfg(not(feature = "W256"))]
fn shift_only_wrapped_bucket_evaluator_m2(bits_per_seed: u8, slice_len: u16) -> [i32; 7] {
    match (bits_per_seed, slice_len) {  // most found for 100M keys, some for 500M
        (_, ..=64) => [-314344, 393672, 415296, 426128, 434156, 439924, 444252],  // 8, 4.1, 64
        (_, ..=128) => [-342136, 362372, 401044, 421448, 434912, 445316, 452460], // 8, 4.1, 128
        (..=6, ..=256) => [-453236, 282636, 370876, 412820, 447136, 468872, 485580], // 6, 3.0, slice=256
        (..=6, ..=512) => [-453748, 145916, 350960, 438864, 499172, 548048, 838112], // 6, 3.0, slice=512
        (_, ..=256) => [-332432, 307220, 375556, 418296, 447676, 470804, 548800],  // 7, 3.5, slice=256
        (7, ..=512) => [-45456, 287404, 400952, 467952, 514928, 554624, 581100],  // 7, 3.5, slice=512
        (8, ..=512) => [0, 603044, 710784, 776372, 829552, 859348, 926092], // 4.1, slice=512, 2.59%
        (8, ..=1024) => [0, 566996, 763884, 870716, 950176, 1013988, 1056860],  // 4.1, slice=1024, 1.90%; 500M
        (..=8, ..=2048) => [-13780, 44896, 340516, 552020, 707176, 837916, 936232], // 8, 4.1, slice=2048
        (9, ..=512) => [0, 484544, 615720, 682208, 723740, 759644, 777828],  // 9, 5.1, slice=512, 5.23%
        (9, ..=1024) => [0, 473964, 672448, 784752, 865300, 930072, 965500],  // 9, 5.1, slice=1024, 2.17%
        (9, ..=2048) => [0, 58900, 521596, 737164, 875552, 994300, 1060292],  // 9, 5.1, slice=2048, 2.14%; 500M
        (10, ..=512) => [0, 438552, 585024, 653236, 689476, 725576, 741592], // 10, 5.7, slice=512, 7.89%
        (10, ..=1024) => [0, 88056, 348888, 467736, 545884, 610684, 641648], // 10, 5.7, slice=1024, 3.94%
        (10, ..=2048) => [0, 65660, 445684, 653068, 796424, 919148, 978684],   // 10, 5.7, slice=2048, 1.46%; 500M
        (11, ..=512) => [0, 520216, 636176, 710940, 746608, 783012, 796256],    // 11, 6.3, slice=512, 10.84%
        (11, ..=1024) => [0, 105380, 497596, 628860, 705764, 771400, 798084],    // 11, 6.3, slice=1024, 6.43%
        (11, ..=2048) => [0, 49776, 575300, 792808, 930812, 1057528, 1110924],  // 11, 6.3, slice=2048, 3.15%
        (..=11, _) => [0, 968, 1816, 314220, 624880, 857980, 964520],   // 11, 6.3, slice=4096, 1.10%; 500M
        (_, ..=512) => [0, 511624, 631696, 716948, 751764, 787984, 800360], // 12, 6.8, slice=512, 13.32%
        (_, ..=1024) => [0, 142152, 437992, 579816, 657920, 723040, 746040], // 12, 6.8, slice=1024, 8.70%
        (_, ..=2048) => [0, 2220, 656084, 894360, 1053948, 1182548, 1227792],   // 12, 6.8, slice=2048, 5.05%
        (_, ..=4096) => [0, 1776, 111712, 236092, 661460, 864020, 956408],    // 12, 6.8, slice=4096, 2.49%
        (_, _) => [0, 44420, 70232, 193760, 646796, 1047760, 1250836]    // 12, 6.8, slice=8192, 0.74%; 500M
    }
}

/// Bucket-evaluator weights of [`ShiftOnlyWrapped`] with `MULTIPLIER` = 2.
#[cfg(feature = "W256")]
fn shift_only_wrapped_bucket_evaluator_m2(bits_per_seed: u8, slice_len: u16) -> [i32; 7] {
    match (bits_per_seed, slice_len) {
        (_, ..=64) => [-314344, 393672, 415296, 426128, 434156, 439924, 444252],  // 8, 4.1, 64
        (_, ..=128) => [-342136, 362372, 401044, 421448, 434912, 445316, 452460], // 8, 4.1, 128
        (..=6, ..=256) => [-453236, 282636, 370876, 412820, 447136, 468872, 485580], // 6, 3.0, slice=256
        (..=6, ..=512) => [-453748, 145916, 350960, 438864, 499172, 548048, 838112], // 6, 3.0, slice=512
        (_, ..=256) => [-332432, 307220, 375556, 418296, 447676, 470804, 548800],  // 7, 3.5, slice=256
        (7, ..=512) => [-45456, 287404, 400952, 467952, 514928, 554624, 581100],  // 7, 3.5, slice=512
        (8, ..=512) => [-271052, 312532, 417956, 485856, 533568, 563784, 620428], // 4.1, slice=512
        (8, ..=1024) => [-200548, 263616, 447128, 559560, 636116, 703688, 747980],  // 4.1, slice=1024
        (..=8, ..=2048) => [-13780, 44896, 340516, 552020, 707176, 837916, 936232], // 8, 4.1, slice=2048
        (_, ..=512) => [-183380, 366760, 488900, 552676, 596640, 622824, 659028],  // 5.1, slice=512
        (9, ..=1024) => [-198768, 282148, 462828, 572804, 652864, 715924, 753792],  // 5.1, slice=1024
        (..=9, ..=2048) => [-14056, 48008, 343520, 547396, 684508, 790796, 863148],  // 9, 5.1, slice=2048
        (10, ..=1024) => [-17532, 67132, 331468, 450216, 527724, 593124, 624052], // 5.7, slice=1024 USELESS
        (..=10, ..=2048) => [-13544, 52204, 330100, 534064, 676016, 793780, 858072],   // 10, 5.7, slice=2048
        (11, ..=1024) => [-6248, 99184, 491312, 622888, 696556, 765680, 792340],    // 6.3, slice=1024 USELESS
        (11, ..=2048) => [-8972, 84172, 332584, 546396, 688744, 802852, 861192],  // 6.3, slice=2048 USELESS
        (11, _) => [-16180, 35856, 37448, 84512, 347420, 546732, 666560],   // 11, 6.3, slice=4096
        (_, ..=1024) => [-8976, 131108, 428784, 568204, 645696, 711052, 733900], // 12, 6.8, slice=1024 USELESS
        (_, ..=2048) => [-11232, 64104, 361384, 600824, 742032, 859852, 911548],   // 12, 6.8, slice=2048 USELESS
        (_, _ /*..=4096*/) => [-16176, 28656, 40632, 88384, 370252, 571656, 684492],    // 12, 6.8, slice=4096  USELESS?? pure performance
        //(_, _) => [-19396, 49484, 77680, 109348, 114240, 205204, 413712]    // 12, 6.8, slice=8192, TODO optimize
    }
}

/// Bucket-evaluator weights of [`ShiftOnlyWrapped`] with `MULTIPLIER` = 3.
#[cfg(not(feature = "W256"))]
fn shift_only_wrapped_bucket_evaluator_m3(bits_per_seed: u8, slice_len: u16) -> [i32; 7] {
    match (bits_per_seed, slice_len) { // most found for 100M keys, some for 500M
        (_, ..=64) => [-325368, 390952, 412772, 425220, 434096, 439504, 449528],  // 8, 4.1, 64
        (_, ..=128) => [-331532, 357000, 396984, 420120, 435932, 444896, 468232], // 8, 4.1, 128
        (..=6, ..=256) => [-573680, 281456, 359176, 401724, 431112, 455368, 1014172], // 6, 3.0, slice=256
        (..=6, ..=512) => [-475624, 165804, 332708, 418280, 478080, 527152, 790172], // 6, 3.0, slice=512
        (_, ..=256) => [-331312, 308768, 378840, 420972, 450864, 475072, 544900],  // 7, 3.5, slice=256
        (7, ..=512) => [-46160, 274320, 392872, 461480, 514428, 556472, 583328],  // 7, 3.5, slice=512
        (8, ..=512) => [0, 255924, 371856, 441428, 492232, 521384, 591692], // 8, 4.1, slice=512, 2.37%
        (8, ..=1024) => [0, 463728, 662200, 779160, 866056, 934136, 977968],  // 4.1, slice=1024, 1.46%, 500M
        (..=8, ..=2048) => [0, 911344, 1300136, 1501532, 1718284, 1781392, 1873240], // 8, 4.1, slice=2048, 1.98%, 500M
        (9, ..=512) => [0, 248684, 366692, 438876, 469116, 498324, 534736], // 9, 5.1, slice=512, 3.40%
        (9, ..=1024) => [0, 411800, 615064, 733228, 812000, 876076, 912608],  // 5.1, slice=1024, 2.06%
        (9, ..=2048) => [0, 62280, 486576, 711280, 859520, 988644, 1056616],  // 9, 5.1, slice=2048, 1.68%, 500M
        (9, ..=4096) => [0, 24356, 704108, 1101208, 1399836, 1588124, 1719224],  // 9, 5.1, slice=4096, 2.48%, 500M
        (10, ..=512) => [0, 195924, 309252, 377468, 411208, 451120, 467608], // 10, 5.7, slice=512, 4.86%
        (10, ..=1024) => [0, 130432, 331488, 444040, 514624, 571476, 609052], // 10, 5.7, slice=1024, 2.19%
        (10, ..=2048) => [0, 57424, 438944, 652792, 795328, 917576, 977384],   // 10, 5.7, slice=2048, 1.38%, 500M
        (10, ..=4096) => [0, 195640, 199936, 698360, 979820, 1180916, 1300792],   // 10, 5.7, slice=4096, 1.32%, 500M
        (11, ..=512) => [0, 273844, 378980, 469476, 484544, 491440, 510660],    // 11, 6.3, slice=512, 7.29%
        (11, ..=1024) => [0, 104520, 364452, 473060, 548676, 612252, 639932],    // 11, 6.3, slice=1024, 3.48%
        (11, ..=2048) => [-8916, 84832, 354216, 550572, 679620, 800300, 854984],  // 11, 6.3, slice=2048 USELESS
        (11, _) => [-13068, 100164, 97300, 163144, 402112, 620500, 731288],  // 11, 6.3, slice=4096, 500M
        (_, ..=1024) => [-8824, 134512, 443604, 572588, 644912, 710236, 735176], // 12, 6.8, slice=1024 USELESS
        (_, ..=2048) => [-10660, 65008, 392192, 598076, 733948, 859836, 909388],   // 12, 6.8, slice=2048 USELESS
        (_, _) => [-13424, 104296, 105112, 178768, 378988, 573704, 674396],  // 12, 6.8, slice=4096  USELESS
    }
}

/// Bucket-evaluator weights of [`ShiftOnlyWrapped`] with `MULTIPLIER` = 3.
#[cfg(feature = "W256")]
fn shift_only_wrapped_bucket_evaluator_m3(bits_per_seed: u8, slice_len: u16) -> [i32; 7] {
    match (bits_per_seed, slice_len) { // multiplier=3, almost the same result as for multiplier=2 weights
        (_, ..=64) => [-325368, 390952, 412772, 425220, 434096, 439504, 449528],  // 8, 4.1, 64
        (_, ..=128) => [-331532, 357000, 396984, 420120, 435932, 444896, 468232], // 8, 4.1, 128
        (..=6, ..=256) => [-573680, 281456, 359176, 401724, 431112, 455368, 1014172], // 6, 3.0, slice=256
        (..=6, ..=512) => [-475624, 165804, 332708, 418280, 478080, 527152, 790172], // 6, 3.0, slice=512
        (_, ..=256) => [-331312, 308768, 378840, 420972, 450864, 475072, 544900],  // 7, 3.5, slice=256
        (7, ..=512) => [-46160, 274320, 392872, 461480, 514428, 556472, 583328],  // 7, 3.5, slice=512
        (_, ..=512) => [100400, 357444, 468452, 539020, 589476, 618424, 689512], // 4.1, slice=512
        (8, ..=1024) => [-202596, 255168, 440056, 557068, 645140, 706376, 753220],  // 4.1, slice=1024
        (..=8, ..=2048) => [-13708, 41552, 361880, 567580, 717652, 834304, 930212], // 8, 4.1, slice=2048
        (9, ..=1024) => [-167028, 241116, 452276, 573868, 651568, 716364, 752556],  // 5.1, slice=1024
        (..=9, ..=2048) => [-15012, 47360, 310808, 530784, 678564, 802748, 875056],  // 9, 5.1, slice=2048
        (10, ..=1024) => [-9576, 118560, 327684, 434928, 504916, 564408, 601828], // 5.7, slice=1024 USELESS
        (..=10, ..=2048) => [-13668, 54256, 324832, 532140, 674024, 792456, 857528],   // 10, 5.7, slice=2048
        (11, ..=1024) => [-6220, 103928, 506868, 622844, 696808, 765432, 792988],    // 6.3, slice=1024 USELESS
        (11, ..=2048) => [-8916, 84832, 354216, 550572, 679620, 800300, 854984],  // 6.3, slice=2048 USELESS
        (11, _) => [-13068, 100164, 97300, 163144, 402112, 620500, 731288],  // 11, 6.3, slice=4096
        (_, ..=1024) => [-8824, 134512, 443604, 572588, 644912, 710236, 735176], // 12, 6.8, slice=1024 USELESS
        (_, ..=2048) => [-10660, 65008, 392192, 598076, 733948, 859836, 909388],   // 12, 6.8, slice=2048 USELESS
        (_, _) => [-13424, 104296, 105112, 178768, 378988, 573704, 674396],  // 12, 6.8, slice=4096  USELESS
    }
}

impl<const MULTIPLIER: u8> SeedChooserConf for ShiftOnlyWrapped<MULTIPLIER> {

    type SeedChooser = Self;

    type BucketEvaluator = Weights;

    type Core = ShiftWrappedCore<MULTIPLIER>;

    type UsedValues = UsedValueSet;
    
    #[inline(always)] fn seed_chooser(&self, _bits_per_seed: u8, _slice_len: u16, _loading_factor_1000: u16) -> Self::SeedChooser {
        *self
    }

    fn bucket_evaluator(&self, bits_per_seed: u8, slice_len: u16, _loading_factor_1000: u16) -> Weights {
        Weights(match MULTIPLIER {
            1 => shift_only_wrapped_bucket_evaluator_m1(bits_per_seed, slice_len),
            2 => shift_only_wrapped_bucket_evaluator_m2(bits_per_seed, slice_len),
            _ => shift_only_wrapped_bucket_evaluator_m3(bits_per_seed, slice_len)
        })
    }

    #[inline(always)] fn core(&self) -> Self::Core { ShiftWrappedCore::<MULTIPLIER> }

    #[inline] fn empty_used_values(&self) -> Self::UsedValues { Default::default() }

    #[inline(always)] fn add_used(&self, used_values: &mut Self::UsedValues, value: usize) { used_values.add(value); }

    #[inline(always)] fn clear_used(&self, used_values: &mut Self::UsedValues, value: usize) { used_values.remove(value); }

    //type UsedValues = UsedValueSetLarge;
    //const FUNCTION2_THRESHOLD: usize = 4096*2;

    #[allow(clippy::match_overlapping_arm)] // ranges overlap on purpose: the first matching arm has the priority
    fn slice_len(&self, output_range: usize, bits_per_seed: u8, preferred_slice_len: u16) -> u16 {
        match output_range.saturating_sub(self.extra_shift(bits_per_seed) as usize) {
            n @ 0..8192 => (n/2+1).next_power_of_two() as u16,
            /*64..1300 => 64,
            1300..1750 => 128,
            1750..7500 => 256,
            7500..150000 => 512,
            150000..250000 => 1024,*/
            //_ => 2048,
            _ => /* 2* */ 8192,
        }.min(if preferred_slice_len != 0 { preferred_slice_len } else { match MULTIPLIER {
            1 => match bits_per_seed {
                ..=5 => 256,
                ..=7 => 512,   // or 6 => 256 for smaller size
                ..=9 => 1024,   // or 8 => 512 for smaller size
                ..=11 => 2048,   // or 10 => 1024 for smaller size(?)
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
        }})
    }
}

impl<const MULTIPLIER: u8> SeedChooser for ShiftOnlyWrapped<MULTIPLIER> {

    #[inline]
    fn best_seed<C: Core>(&self, used_values: &mut Self::UsedValues, keys: &[u64], core: &C, bits_per_seed: u8, _bucket_nr: usize, _first_bucket_in_window: usize) -> u16 {
        let mut without_shift_arrayvec: arrayvec::ArrayVec::<(usize, u16), 16>;
        let mut without_shift_box: Box<[(usize, u16)]>;
        let without_shift: &mut [(usize, u16)] = if keys.len() > 16 {   // we add MULTIPLIER to key as shift=0 is invalid (reserved for bumping)
            without_shift_box = keys.iter().map(|key| (core.slice_begin(*key), (*key as u16).wrapping_add(MULTIPLIER as u16) & core.slice_len_minus_one())).collect();
            &mut without_shift_box
        } else {
            without_shift_arrayvec = keys.iter().map(|key| (core.slice_begin(*key), (*key as u16).wrapping_add(MULTIPLIER as u16) & core.slice_len_minus_one())).collect();
            &mut without_shift_arrayvec
        };

        let slice_len = core.slice_len();
        let mut score_without_shift: usize = 1<<20; // this is not a real score as we only need relative scores
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
            let best_plus_multiplier = best_total_shift as usize + MULTIPLIER as usize;
            for key in keys {
                used_values.add(core.slice_begin(*key) + ((*key as usize).wrapping_add(best_plus_multiplier)&core.slice_len_minus_one() as usize));
            }
            (best_plus_multiplier / MULTIPLIER as usize) as u16
        }
    }
}


#[derive(Clone, Copy)]
pub struct ShiftSeedCore<const MULTIPLIER: u8>(pub u8);

impl<const MULTIPLIER: u8> SeedChooserCore for ShiftSeedCore<MULTIPLIER> {
    #[inline(always)] fn f<C: Core>(&self, primary_code: u64, seed: u16, core: &C) -> usize {
        core.slice_begin(primary_code) +
            ((mix_key_seed(primary_code, (seed>>self.0) + 1)
             + MULTIPLIER as u16 * seed) & core.slice_len_minus_one()) as usize
    }

    /// Writes `self` to the `output`.
    fn write(&self, output: &mut dyn io::Write) -> io::Result<()> { 
        AsIs::write(output, self.0)
    }

    /// Returns number of bytes which `write` will write.
    fn write_bytes(&self) -> usize { AsIs::size(self.0) }

    /// Read `Self` from the `input`.
    fn read(input: &mut dyn io::Read) -> io::Result<Self> {
        Ok(Self(AsIs::read(input)?)) 
    }
}

/// [`SeedChooser`] to build (1-)perfect functions which uses both shifting with wrapping and regular hashing.
/// The parameter points the number of bits of seed used for regular hashing.
/// Increasing it reduces size but slows down construction.
/// 
/// Can be used with any function type: [`Function`](crate::phast::Function), [`Function2`](crate::phast::Function2), [`Perfect`](crate::phast::Perfect).
/// 
/// It chooses best seed using both shifting with wrapping and hashing,
/// which leads to small size and medium speed constructions,
/// but quite slow evaluation.
/// 
/// `MULTIPLIER` should be 1, 2, or 3.
/// Typically, increasing `MULTIPLIER` reduces size but slows down construction.
#[derive(Clone, Copy)]
pub struct ShiftSeedWrapped<const MULTIPLIER: u8>(pub u8);

impl<const MULTIPLIER: u8> SeedChooserConf for ShiftSeedWrapped<MULTIPLIER> {

    type SeedChooser = Self;

    type BucketEvaluator = Weights;

    type Core = ShiftSeedCore<MULTIPLIER>;

    type UsedValues = UsedValueSet;

    #[inline(always)] fn core(&self) -> Self::Core { ShiftSeedCore::<MULTIPLIER>(self.0) }
    
    #[inline(always)] fn seed_chooser(&self, _bits_per_seed: u8, _slice_len: u16, _loading_factor_1000: u16) -> Self::SeedChooser {
        *self
    }

    #[inline] fn empty_used_values(&self) -> Self::UsedValues { Default::default() }

    #[inline(always)] fn add_used(&self, used_values: &mut Self::UsedValues, value: usize) { used_values.add(value); }

    #[inline(always)] fn clear_used(&self, used_values: &mut Self::UsedValues, value: usize) { used_values.remove(value); }

    #[inline(always)] fn slice_len(&self, output_range: usize, bits_per_seed: u8, preferred_slice_len: u16) -> u16 {
        match output_range.saturating_sub(self.extra_shift(bits_per_seed) as usize) {
                    n @ 0..64 => (n/2+1).next_power_of_two() as u16,
                    64..1300 => 64,
                    1300..1750 => 128,
                    1750..7500 => 256,
                    7500..150000 => 512,
                    150000..250000 => 1024,
                    _ => 2048,
        }.min(if preferred_slice_len != 0 { preferred_slice_len } else { 1024 })    // TODO tune 1024
    }

    #[inline] fn bucket_evaluator(&self, bits_per_seed: u8, slice_len: u16, _loading_factor_1000: u16) -> Self::BucketEvaluator {
        Weights::new(bits_per_seed, slice_len)
    }
}

impl<const MULTIPLIER: u8> SeedChooser for ShiftSeedWrapped<MULTIPLIER> {


    #[inline]
    fn best_seed<C: Core>(&self, used_values: &mut Self::UsedValues, keys: &[u64], conf: &C, bits_per_seed: u8, _bucket_nr: usize, _first_bucket_in_window: usize) -> u16 {
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
                *in_slice &= conf.slice_len_minus_one();
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
                    + MULTIPLIER as u16 * result) & conf.slice_len_minus_one()) as usize
                );
            }
            result
        }
    }
    
    
}
