use bitm::{BitAccess, BitVec, ArrayWithRankSelect101111, CombinedSampling};

struct LevelBuilder {
    upper_bit: Box<[u64]>,
    upper_index: usize,
    lower_bits: Box<[u64]>,
    lower_zero_index: usize,
    lower_one_index: usize,
    upper_bit_mask: u64,
    bits_per_value: u8
}

impl LevelBuilder {
    fn new(number_of_zeros: usize, total_len: usize, index_of_bit_to_extract: u8) -> Self {
        Self {
            upper_bit: Box::with_zeroed_bits(total_len),
            upper_index: 0,
            lower_bits: Box::with_zeroed_bits(total_len * (index_of_bit_to_extract-1) as usize),
            lower_zero_index: 0,
            lower_one_index: number_of_zeros,
            upper_bit_mask: 1<<index_of_bit_to_extract,
            bits_per_value: index_of_bit_to_extract
        }
    }

    fn push(&mut self, value: u64) {
        let is_one = value & self.upper_bit_mask != 0;
        self.upper_bit.init_successive_bit(&mut self.upper_index, is_one);
        self.lower_bits.init_successive_bits(
            &mut if is_one { self.lower_one_index } else { self.lower_zero_index },
            value & (self.upper_bit_mask-1), self.bits_per_value);
    }
}

type ArrayWithRank = ArrayWithRankSelect101111::<CombinedSampling, CombinedSampling>;

struct WaveletMatrixLevel {
    /// Bits.
    bits: ArrayWithRank,

    /// Number of zero bits.
    zeros: usize
}

impl WaveletMatrixLevel {
    fn new(level: Box::<[u64]>, zeros: usize) -> Self {
        //let (bits, number_of_ones) = ArrayWithRank::build(level);
        //Self { bits, zeros: level_len - number_of_ones }
        Self { bits: level.into(), zeros }
    }
}

pub struct WaveletMatrix {
    levels: Box<[WaveletMatrixLevel]>
}

impl WaveletMatrix {

    pub fn from_fn<I, F>(content: F, content_len: usize, bits_per_value: u8) -> Self
        where I: IntoIterator<Item = u64>, F: Fn() -> I
    {
        assert!(bits_per_value > 0 && bits_per_value <= 64);
        let mut levels = Vec::with_capacity(bits_per_value as usize);
        if bits_per_value == 1 {
            let mut level = Box::with_zeroed_bits(content_len);
            for (i, e) in content().into_iter().enumerate() {
                level.init_bit(i, e != 0);
            }
            levels.push(WaveletMatrixLevel::new(level, content_len));
            return Self { levels: levels.into_boxed_slice() };
        }
        let mut number_of_zeros = [0; 64];
        for mut e in content() {
            for b in 0..bits_per_value {
                number_of_zeros[b as usize] += (e & 1) as usize;
                e >>= 1;
            }
        }
        let mut current_bit = bits_per_value - 1;
        let mut rest = {
            let mut level = LevelBuilder::new(
                number_of_zeros[current_bit as usize], content_len, current_bit);
            for e in content() { level.push(e); }
            levels.push(WaveletMatrixLevel::new(level.upper_bit, number_of_zeros[current_bit as usize]));
            level.lower_bits
        };
        current_bit -= 1;
        while current_bit >= 1 {
            let mut level = LevelBuilder::new(
                number_of_zeros[current_bit as usize], content_len, current_bit);
            for index in (0..content_len).step_by(current_bit as usize) {
                level.push(rest.get_bits(index, current_bit));
            }
            rest = level.lower_bits;
            levels.push(WaveletMatrixLevel::new(level.upper_bit, number_of_zeros[current_bit as usize]));
        }
        levels.push(WaveletMatrixLevel::new(rest, number_of_zeros[0]));
        levels.reverse();
        Self { levels: levels.into_boxed_slice() }
    }

}