use crate::phast::{ComparableF64, ShiftWrappedCore, WINDOW_SIZE, Weights, conf::Core, cyclic::UsedValueSet, seed_chooser::shift_wrap::Multiplier};
use super::SeedChooser;

/// [`SeedChooser`] to build (1-)perfect functions called *PHast+ with wrapping*.
/// This version uses product instead of sum to evaluate seeds.
/// 
/// Can be used with any function type: [`Function`], [`Function2`], [`Perfect`].
/// 
/// It chooses best seed using only shifting with wrapping,
/// which leads to quite small size and quite fast construction.
/// 
/// `MULTIPLIER` should be 1, 2, or 3.
/// Typically, increasing `MULTIPLIER` reduces size but slows down construction.
/// `MULTIPLIER=1` works very well with large `bits_per_seed` (10+),
/// larger values (2 and 3) works well with `bits_per_seed=8`.
#[derive(Clone, Copy)]
pub struct ShiftOnlyProdWrapped<const MULTIPLIER: u8 = 1>;

fn shift_only_wrapped_bucket_evaluator_m1(bits_per_seed: u8, slice_len: u16) -> [i32; 7] {
    if WINDOW_SIZE > 350 {
        match (bits_per_seed, slice_len) { // WINDOW_SIZE = 512
            (_, ..=64) => [-76520, 97960, 103626, 106759, 109053, 110149, 112662],   // 8, 4.1, 64
            (_, ..=128) => [-80872, 90492, 100641, 105939, 109960, 112290, 118119], // 8, 4.1, 128
            (..=6, ..=256) => [-76632, 59701, 89939, 103115, 111040, 117906, 283652], // 6, 3.0, slice=256
            (..=6, ..=512) => [-102171, 30195, 95877, 122987, 138980, 152173, 206055],  // 6, 3.0, slice=512
            (_, ..=256) => [-84425, 81165, 96951, 106065, 112137, 117421, 122309],  // 7, 3.5, slice=256
            (7, ..=512) => [-69271, 61152, 101770, 119869, 132454, 141236, 148273],  // 7, 3.5, slice=512
            (8, ..=512) => [0, 156331, 180744, 194468, 205143, 213101, 219184], // W=512, S=8, 4.1, slice=512, 2.68%
            (8, ..=1024) => [0, 210340, 280754, 310181, 328296, 345579, 357109],  // W=512, S=8, 4.1, slice=1024, 2.89%
            (..=8, ..=2048) => [0, 125973, 310793, 384469, 427093, 459775, 487022],  // W=512, S=8, 4.1, slice=2048, 4.10%
            (_, ..=512) => [0, 117100, 147726, 161856, 173086, 178690, 188372],  // W=512, S=9, 5.1, slice=512, 3.32%
            (9, ..=1024) => [0, 110354, 164251, 188777, 208002, 221262, 229669],  // W=512, S=9, 5.1, slice=1024, 2.40%
            (9, ..=2048) => [0, 6401, 178303, 234605, 268158, 296421, 313789],  // W=512, S=9, 5.1, slice=2048, 3.22%
            //(9, 4096) => [0, 4, 111829, 269183, 344453, 400230, 436928] // W=512, S=9, 5.1, slice=4096, 4.89%
            (10, ..=1024) => [0, 22051, 73837, 97796, 117850, 129314, 140490], // W=512, S=10, 5.7, slice=1024, 2.11%
            (10, ..=2048) => [0, 14607, 109732, 155728, 183216, 209792, 224618],   // W=512, S=10, 5.7, slice=2048, 1.61%
            (..=10, ..=4096) => [0, 3, 128279, 252036, 306197, 357339, 387496],   // W=512, S=10, 5.7, slice=4096, 2.61%
            (11, ..=1024) => [0, 27388, 90240, 120744, 139069, 154392, 161287],    // W=512, S=11, 6.3, slice=1024, 3.25%
            (11, ..=2048) => [0, 24166, 111992, 156652, 186724, 212961, 227787],  // W=512, S=11, 6.3, slice=2048, 1.29%
            (11, ..=4096) => [0, 17, 165005, 253419, 311596, 352199, 377737], // W=512, S=11, 6.3, slice=4096, 1.10%
            (11, _) => [0, 43917, 46586, 89083, 215836, 294678, 346865], // W=512, S=11, 6.3, slice=8192, 2.37%
            (_, ..=1024) => [0, 33929, 111765, 144460, 164900, 181227, 187164], // W=512, S=12, 6.8, slice=1024, 4.97%
            (_, ..=2048) => [0, 10645, 123536, 182729, 216394, 245955, 257909],   // W=512, S=12, 6.8, slice=2048, 2.11%
            (_, ..=4096) => [0, 39386, 41110, 65140, 127482, 175702, 200631], // W=512, S=12, 6.8, slice=4096, 0.73%
            (_, _) => [0, 41245, 56358, 82569, 178801, 256021, 301752], // W=512, S=12, 6.8, slice=8192, 0.68%
        }
    } else {    // WINDOW_SIZE = 256
        match (bits_per_seed, slice_len) {
            (_, ..=64) => [-76520, 97960, 103626, 106759, 109053, 110149, 112662],   // 8, 4.1, 64
            (_, ..=128) => [-80872, 90492, 100641, 105939, 109960, 112290, 118119], // 8, 4.1, 128
            (..=6, ..=256) => [-76632, 59701, 89939, 103115, 111040, 117906, 283652], // 6, 3.0, slice=256
            (..=6, ..=512) => [-102171, 30195, 95877, 122987, 138980, 152173, 206055],  // 6, 3.0, slice=512
            (_, ..=256) => [-84425, 81165, 96951, 106065, 112137, 117421, 122309],  // 7, 3.5, slice=256
            (7, ..=512) => [-69271, 61152, 101770, 119869, 132454, 141236, 148273],  // 7, 3.5, slice=512
            (8, ..=512) => [0, 152990, 178178, 192159, 202472, 210473, 216537], // W=256, S=8, 4.1, slice=512, 2.69%
            (8, ..=1024) => [0, 107693, 166854, 196259, 215558, 230759, 242884],  // W=256, S=8, 4.1, slice=1024, 2.96%
            (..=8, ..=2048) => [0, 14926, 91551, 154251, 199562, 227926, 254852],  // W=256, S=8, 4.1, slice=2048, 4.78%
            (_, ..=512) => [0, 116814, 147363, 161526, 172750, 178294, 187973],  // W=256, S=9, 5.1, slice=512, 3.32%
            (9, ..=1024) => [0, 108631, 162464, 187910, 206788, 219596, 228342],  // W=256, S=9, 5.1, slice=1024, 2.40%
            (9, ..=2048) => [0, 14918, 94707, 146874, 179152, 204695, 223034],  // W=256, S=9, 5.1, slice=2048, 3.37%
            //(..=9, ..=4096) => [0, 931, 2752, 48036, 119910, 173319, 212198],   // W=256, S=9, 5.1, slice=2096, 5.75%   ???
            (10, ..=1024) => [0, 21302, 73361, 97829, 117887, 129369, 140609], // W=256, S=10, 5.7, slice=1024, 2.11%
            (10, ..=2048) => [0, 15440, 89392, 135715, 164014, 189435, 204689],   // W=256, S=10, 5.7, slice=2048, 1.62%
            (..=10, ..=4096) => [0, 20927, 26211, 47734, 118682, 162377, 192219],   // W=256, S=10, 5.7, slice=4096, 3.03%
            (11, ..=1024) => [0, 26273, 90245, 120891, 139228, 154539, 161444],    //  W=256, S=11, 6.3, slice=1024, 3.25%
            (11, ..=2048) => [0, 24477, 86077, 126008, 161790, 184156, 198627],  // W=256, S=11, 6.3, slice=2048, 1.29%
            (11, ..=4096) => [0, 17900, 19784, 40648, 89345, 132760, 159319], // W=256, S=11, 6.3, slice=4096, 1.24%
            (11, _) => [0, 7620, 8868, 9008, 15831, 72569, 124794], // W=256, S=11, 6.3, slice=8192, 3.02%
            (_, ..=1024) => [0, 33822, 111204, 143820, 164716, 180629, 186570], // W=256, S=12, 6.8, slice=1024, 4.97%
            (_, ..=2048) => [0, 17352, 80957, 137198, 170928, 200126, 212449],   // W=256, S=12, 6.8, slice=2048, 2.14%
            (_, ..=4096) => [0, 40496, 30478, 46656, 80691, 115102, 139934], // W=256, S=12, 6.8, slice=4096, 0.74%
            (_, _) => [0, 10454, 22433, 22459, 40862, 42988, 85357]   // W=256, S=12, 6.8, slice=8192, 0.99%
        }
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
        (8, ..=512) => [0, 60452, 88934, 99413, 111898, 118637, 134896], // W=256, S=8, 4.1, slice=512, 2.47%
        (8, ..=1024) => [0, 117295, 162111, 187985, 206775, 222871, 234087],  // W=256, S=8, 4.1, slice=1024, 1.80%
        (..=8, ..=2048) => [0, 14482, 96530, 149748, 186518, 215883, 240418], // W=256, S=8, 4.1, slice=2048, 3.04%
        (_, ..=512) => [0, 115760, 149372, 165530, 176685, 184853, 189537],  // W=256, S=9, 5.1, slice=512, 5.06%
        (9, ..=1024) => [0, 96383, 148175, 174752, 193956, 208465, 217820],  // W=256, S=9, 5.1, slice=1024, 2.01%
        (..=9, ..=2048) => [0, 15459, 92262, 138717, 171918, 198539, 216424],   // W=256, S=9, 5.1, slice=2048, 1.99%
        (..=9, ..=4096) => [0, 3387, 6661, 58238, 124550, 169968, 206798],   // W=256, S=9, 5.1, slice=4096, 3.82%
        (10, ..=1024) => [0, 21217, 87315, 117004, 136428, 152750, 160488], // W=256, S=10, 5.7, slice=1024, 3.69%
        (..=10, ..=2048) => [0, 16093, 81532, 126821, 160179, 187768, 203515],   // W=256, S=10, 5.7, slice=2048, 1.27%
        (..=10, ..=4096) => [0, 12910, 13316, 29688, 94298, 138511, 170268],   // W=256, S=10, 5.7, slice=2048, 1.63%
        (11, ..=1024) => [0, 26238, 118939, 155743, 175223, 191502, 197740],    // W=256, S=11, 6.3, slice=1024, 6.23%
        (11, ..=2048) => [0, 23676, 85767, 144949, 181778, 210304, 224248],  // W=256, S=11, 6.3, slice=2048, 2.90%
        (11, _) => [0, 12764, 13333, 23992, 80634, 126928, 151205],   // W=256, S=11, 6.3, slice=4096, 0.86%
        (_, ..=1024) => [0, 35513, 109131, 146470, 167346, 184750, 190448], // W=256, S=12, 6.8, slice=1024, 8.55%
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

impl<const MULTIPLIER: u8> SeedChooser for ShiftOnlyProdWrapped<MULTIPLIER> {

    type UsedValues = UsedValueSet;

    type Core = ShiftWrappedCore<MULTIPLIER>;

    #[inline] fn empty_used_values(&self) -> Self::UsedValues { Default::default() }

    #[inline(always)] fn add_used(&self, used_values: &mut Self::UsedValues, value: usize) { used_values.add(value); }

    #[inline(always)] fn clear_used(&self, used_values: &mut Self::UsedValues, value: usize) { used_values.remove(value); }

    #[inline(always)] fn core(&self) -> Self::Core { ShiftWrappedCore::<MULTIPLIER> }

    //type UsedValues = UsedValueSetLarge;
    //const FUNCTION2_THRESHOLD: usize = 4096*2;

    fn bucket_evaluator(&self, bits_per_seed: u8, slice_len: u16) -> Weights {
        Weights(match MULTIPLIER {
            1 => shift_only_wrapped_bucket_evaluator_m1(bits_per_seed, slice_len),
            2 => shift_only_wrapped_bucket_evaluator_m2(bits_per_seed, slice_len),
            _ => shift_only_wrapped_bucket_evaluator_m3(bits_per_seed, slice_len)
        })
    }

    #[inline]
    fn best_seed<C: Core>(&self, used_values: &mut Self::UsedValues, keys: &[u64], core: &C, bits_per_seed: u8, bucket_nr: usize, _first_bucket_in_window: usize) -> u16 {
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
        let mut best_score = ComparableF64(f64::MAX);
        let score_to_extract = core.slice_begin_for_bucket(bucket_nr).wrapping_sub(95);
        let mut total_end_shift = ((MULTIPLIER as u16) << bits_per_seed) - MULTIPLIER as u16;
        // note that total_last_shift itself is not allowed
        let mut shift_sum = 0;
        let mut best_total_shift = u16::MAX;
        loop {  // while total_last_shift > 0
            let max_sh0 = without_shift.iter().map(|(_, sh0)| *sh0).max().unwrap();
            let mut shift_end = Multiplier::<MULTIPLIER>::multiple_rounded_up(slice_len - max_sh0);
            let last = shift_end >= total_end_shift;
            if last { shift_end = total_end_shift; }
            if let Some(best_shift) = Multiplier::<MULTIPLIER>::best_in_range(shift_end, without_shift, used_values) {
                let new_score = ComparableF64(without_shift.iter()
                    .map(|(beg, sh0)| (*beg + *sh0 as usize + best_shift as usize).wrapping_sub(score_to_extract) as f64)
                    .product());
                if new_score < best_score {
                    best_total_shift = shift_sum + best_shift;
                    best_score = new_score;
                }
            }
            if last { break; }
            for (_, sh0) in without_shift.iter_mut() {
                *sh0 += shift_end;
                if *sh0 >= slice_len { *sh0 -= slice_len; }
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
