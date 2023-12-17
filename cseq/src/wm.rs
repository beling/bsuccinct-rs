use bitm::{BitAccess, BitVec, ArrayWithRankSelect101111, CombinedSampling, ceiling_div, n_lowest_bits};

struct BitVecBuilder {
    content: Vec<u64>,
    bit_nr: u8
}

impl BitVecBuilder {
    fn with_capacity(number_of_bits: usize) -> Self {
        BitVecBuilder {
            content: Vec::with_capacity(ceiling_div(number_of_bits, 64)),
            bit_nr: 63
        }
    }

    fn push(&mut self, bit: bool) {
        if self.bit_nr == 63 {
            self.bit_nr = 0;
            self.content.push(bit as u64);
        } else {
            self.bit_nr += 1;
            *self.content.last_mut().unwrap() |= (bit as u64) << self.bit_nr;
        }
    }

    fn finish(self) -> Box<[u64]> { self.content.into_boxed_slice() }
}

struct SortedCompactVecBuilder {
    content: Box<[u64]>,
    zero_index: usize,
    one_index: usize,
    value_mask: u64,
    bits_per_value: u8
}

impl SortedCompactVecBuilder {
    fn new(number_of_zeros: usize, total_len: usize, bits_per_value: u8) -> Self {
        Self {
            content: Box::with_zeroed_bits(total_len * bits_per_value as usize),
            zero_index: 0,
            one_index: number_of_zeros,
            value_mask: n_lowest_bits(bits_per_value),
            bits_per_value
        }
    }

    fn push(&mut self, mut value: u64, is_one: bool) {
        value &= self.value_mask;
        let mut index = if is_one { &mut self.one_index } else { &mut self.zero_index };
        self.content.init_bits(*index, value, self.bits_per_value);
        *index += self.bits_per_value as usize;
    }

    fn finish(self) -> Box<[u64]> { self.content }
}

type ArrayWithRank = ArrayWithRankSelect101111::<CombinedSampling, CombinedSampling>;

struct WaveletMatrixLevel {
    /// Bits.
    bits: ArrayWithRank,

    /// Number of zero bits.
    zeros: usize
}

impl WaveletMatrixLevel {
    fn new(level: Box::<[u64]>, level_len: usize) -> Self {
        let (bits, number_of_ones) = ArrayWithRank::build(level);
        Self { bits, zeros: level_len - number_of_ones }
    }
}

struct WaveletMatrix {
    levels: Box<[WaveletMatrixLevel]>
}

impl WaveletMatrix {

    pub fn from_fn<I, F>(content: F, content_len: usize, mut bits_per_value: u8) -> Self
        where I: IntoIterator<Item = u64>, F: Fn() -> I
    {
        assert!(bits_per_value > 0 && bits_per_value < 64);
        let mut levels = Vec::with_capacity(bits_per_value as usize);
        let mut level = BitVecBuilder::with_capacity(content_len);
        if bits_per_value == 1 {
            for e in content() { level.push(e != 0); }
            levels.push(WaveletMatrixLevel::new(level.finish(), content_len));
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
        let mut rest = Option::<Box::<[u64]>>::default();
        while current_bit >= 1 {
            let mut level = BitVecBuilder::with_capacity(content_len);
            let level_mask = 1<<current_bit;
            let mut new_rest = SortedCompactVecBuilder::new(
                number_of_zeros[current_bit as usize], content_len, current_bit-1);
            let mut push = |e| {
                let is_one = e & level_mask != 0;
                level.push(is_one);
                new_rest.push(e, is_one);
            };
            if current_bit == bits_per_value - 1 { // first level
                for e in content() { push(e); }
                // TODO here we can drop content
            } else {
                let prev_rest = rest.unwrap();
                for index in (0..content_len).step_by(current_bit as usize) {
                    push(prev_rest.get_bits(index, current_bit));
                }
            }
            rest = Some(new_rest.finish());
            levels.push(WaveletMatrixLevel::new(level.finish(), content_len));
            current_bit -= 1;
        }
        levels.push(WaveletMatrixLevel::new(rest.unwrap(), content_len));
        levels.reverse();
        Self { levels: levels.into_boxed_slice() }
    }

}