use bitm::ceiling_div;
use std::mem::MaybeUninit;
use fsum::FSum;
use std::fmt;
use std::fmt::Formatter;
use super::kvset::KVSet;

/// Chooses the size of level for the given sequence of retained values.
pub trait LevelSizer {

    /// Returns number of 64-bit segments to use for given sequence of retained `values`.
    fn size_segments_for_values<VIt, F>(&self, _values: F, values_len: usize, _bits_per_value: u8) -> usize
        where VIt: IntoIterator<Item = u64>, F: FnMut() -> VIt
    {
        self.max_size_segments(values_len)
    }

    /// Returns number of 64-bit segments to use for given sequence of retained `values`.
    fn size_segments<K, KV: KVSet<K>>(&self, kv: &KV) -> usize
    {
        self.max_size_segments(kv.kv_len())
    }


    /// Returns maximal number of segment that can be returned by `size_segments` for level of size `max_level_size` or less.
    fn max_size_segments(&self, max_level_size: usize) -> usize;
}

/// Choose level size as a percent of the input size.
#[derive(Copy, Clone)]
pub struct ProportionalLevelSize {
    pub percent: u16
}

impl ProportionalLevelSize {
    pub fn with_percent(percent: u16) -> Self { Self{percent} }
}

impl Default for ProportionalLevelSize {
    fn default() -> Self { Self::with_percent(90) } // 80 is a bit better than 90 but slower
}

impl LevelSizer for ProportionalLevelSize {
    fn max_size_segments(&self, max_level_size: usize) -> usize {
        ceiling_div(max_level_size*self.percent as usize, 64*100)
    }
}

impl fmt::Display for ProportionalLevelSize {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}percent", self.percent)
    }
}

/// Chooses optimal level size considering distribution of incidence of values.
#[derive(Default, Copy, Clone)]
pub struct OptimalLevelSize;

/// Removes zeros from `count` and returns its prefix without zeros.
fn remove_zeros(counts: &mut [u32]) -> &[u32] {
    let mut counts_len = 0usize;
    for i in 0usize..counts.len() {
        if counts[i] != 0 {
            counts[counts_len] = counts[i];
            counts_len += 1;
        }
    }
    &counts[0..counts_len]
}

/// For given distribution of incidence of values `counts` and `input_size` (sum of counts),
/// returns probabilities of k positive collisions, for all k = 0, 1, ..., 15.
pub(crate) fn positive_collisions_prob(counts: &mut [u32], input_size: usize) -> [f64; 16] {
    let counts = remove_zeros(counts);
    let mut array: [MaybeUninit<f64>; 16] = unsafe { MaybeUninit::uninit().assume_init() };
    for i in 0usize..array.len() {   // k = i + 1 = 1, 2, ...
        let mut r = FSum::new();
        for c in counts {
            //if *c > i as u32 {
            //    r += (*c as f64 / input_size as f64).powi(i as i32+1);
            //}
            if *c > i as u32 {
                r += (0..=i)
                    .map(|v| (*c - v as u32) as f64 / (input_size - v) as f64)
                    .fold(1.0, |a, b| a * b);
            }
        }
        array[i] = MaybeUninit::new(r.value());
    }
    unsafe { std::mem::transmute::<_, [f64; 16]>(array) }
}

impl OptimalLevelSize {
    fn size_segments_for_dist(counts: &mut [u32], input_size: usize, bits_per_fragment: u8) -> usize {
        let mut result = ceiling_div(input_size, 64);
        if result == 1 { return 1; }
        let positive_collisions_p = positive_collisions_prob(counts, input_size);
        let mut result_eval = f64::MAX;
        while result >= 1 {
            let mut numerator = FSum::with_value(1.0625);
            /*#[cfg(feature = "simple_rank")] let mut numerator = FSum::with_value(1.0625);
            #[cfg(not(feature = ""))] let mut numerator = FSum::with_value(1.03125);*/
            numerator += 1.0 / result as f64;   // = 64bit / (level size * 64bit)
            let mut denominator = FSum::new();

            let lambda = input_size as f64 / (result * 64) as f64;
            let mut lambda_to_power_k = lambda;
            let mut k_factorial = 1u64;
            for i in 0usize..16 {
                let k = i as u32 + 1;
                k_factorial *= k as u64;
                let pk = positive_collisions_p[i] * lambda_to_power_k * (-lambda).exp() / k_factorial as f64;
                lambda_to_power_k *= lambda;

                numerator += pk * bits_per_fragment as f64;
                denominator += pk * k as f64;
            }
            let new_result_eval = numerator.value() / denominator.value();
            if new_result_eval >= result_eval {  // impossible in the first iteration
                return result + 1;
            }
            result_eval = new_result_eval;
            result -= 1;
        }
        1
    }
    //Licznik = (suma po k = 1... z pk(k)) * bits_per_fragment + 1
    //Mianownik = suma po k = 1 ... z k*pk(k)
    //pk(k) = positive_collisions_p(k) * d.pmf(k)
    //positive_collisions_p(k) = suma po v = [prawdopdobieństwa (udziały) wystąpień różnych wartości] z v**k
    //    (positive_collisions_p to prawdopobieństwo pozytywnej kolizji dla k elementów trafiających w ten sam indeks)
    //d.pmf(k) = prawdopobieństwo k sukcesów według rozkładu
    //  poisson(licza fragmentów do zapisania, wielkość wejścia / wielkość tablicy, liczba wpisów)
}

/*impl LevelSizeChooser for OptimalLevelSize {
    fn size_segments<C: Coding>(&self, coding: &C, values: &[C::Codeword], value_rev_indices: &[u8]) -> usize {
        let mut counts = [0u32; 256];
        for (c, ri) in values.iter().zip(value_rev_indices.iter()) {
            counts[coding.rev_fragment_of(*c, *ri) as usize] += 1;
        }
        Self::size_segments_for_dist(
            &mut counts[0..=coding.max_fragment_value() as usize],
            values.len(),
            coding.bits_per_fragment()
        )
    }

    fn max_size_segments(&self, max_level_size: usize) -> usize {
        ceiling_div(max_level_size, 64)
    }
}*/

impl LevelSizer for OptimalLevelSize {
    fn size_segments_for_values<VIt, F>(&self, mut values: F, values_len: usize, bits_per_value: u8) -> usize
        where VIt: IntoIterator<Item = u64>, F: FnMut() -> VIt
    {
        let mut counts = [0u32; 256];   // TODO support bits_per_value > 8
        for v in values() { counts[v as usize] += 1; }
        Self::size_segments_for_dist(
            &mut counts[0..(1usize<<bits_per_value)],
            values_len,
            bits_per_value
        )
    }

    fn size_segments<K, KV: KVSet<K>>(&self, kv: &KV) -> usize
    {
        let mut counts = [0u32; 256];   // TODO support bits_per_value > 8
        kv.for_each_key_value(|_, v| counts[v as usize] += 1);
        let bits_per_value = kv.bits_per_value();
        Self::size_segments_for_dist(
            &mut counts[0..(1usize<<bits_per_value)],
            kv.kv_len(),
            bits_per_value
        )
    }

    fn max_size_segments(&self, max_level_size: usize) -> usize {
        ceiling_div(max_level_size, 64)
    }
}

impl fmt::Display for OptimalLevelSize {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "optimal")
    }
}


#[derive(Copy, Clone)]
pub struct OptimalGroupedLevelSize {
    pub divider: u8
}

impl Default for OptimalGroupedLevelSize {
    fn default() -> Self {
        Self { divider: 1 }
    }
}

impl OptimalGroupedLevelSize {
    pub fn with_divider(divider: u8) -> Self {
        Self { divider: divider.max(1) }
    }
}

impl LevelSizer for OptimalGroupedLevelSize {
    fn size_segments_for_values<VIt, F>(&self, mut values: F, values_len: usize, bits_per_value: u8) -> usize
    where VIt: IntoIterator<Item = u64>, F: FnMut() -> VIt
    {
        let divider = self.divider as usize;
        let max_value = (1usize<<bits_per_value) - 1;
        (0..divider).map(|delta| {
            let mut counts = [0u32; 256];   // TODO support for bits_per_value > 8
            for v in values() { counts[(v as usize + delta) / divider] += 1; }
            OptimalLevelSize::size_segments_for_dist(
                &mut counts[0 ..= (max_value + delta) / divider],
                values_len,
                bits_per_value  // this must be unchanged as it is used to calculate memory used by a value
            )
        }).min().unwrap()
    }

    fn size_segments<K, KV: KVSet<K>>(&self, kv: &KV) -> usize
    {
        let bits_per_value = kv.bits_per_value();
        let divider = self.divider as usize;
        let max_value = (1usize<<bits_per_value) - 1;
        (0..divider).map(|delta| {
            let mut counts = [0u32; 256];   // TODO support for bits_per_value > 8
            kv.for_each_key_value(|_, v| counts[(v as usize + delta) / divider] += 1);
            OptimalLevelSize::size_segments_for_dist(
                &mut counts[0 ..= (max_value + delta) / divider],
                kv.kv_len(),
                bits_per_value  // this must be unchanged as it is used to calculate memory used by a value
            )
        }).min().unwrap()
    }

    fn max_size_segments(&self, max_level_size: usize) -> usize {
        ceiling_div(max_level_size, 64)
    }
}

impl fmt::Display for OptimalGroupedLevelSize {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "optimal_grouped{}", self.divider)
    }
}

/// Resize level obtained from another chooser.
#[derive(Copy, Clone)]
pub struct ResizedLevel<LSC> {
    pub level_size_chooser: LSC,
    pub percent: u16
}

impl<LSC> ResizedLevel<LSC> {
    #[inline(always)] pub fn new(percent: u16, level_size_chooser: LSC) -> Self {
        Self { level_size_chooser, percent }
    }

    #[inline(always)] fn resized(&self, size: usize) -> usize {
        ceiling_div(size * self.percent as usize, 100)
    }
}

impl<LSC: LevelSizer> LevelSizer for ResizedLevel<LSC> {
    #[inline] fn size_segments_for_values<VIt, F>(&self, values: F, values_len: usize, bits_per_value: u8) -> usize
    where VIt: IntoIterator<Item = u64>, F: FnMut() -> VIt
    {
        self.resized(self.level_size_chooser.size_segments_for_values(values, values_len, bits_per_value))
    }

    fn size_segments<K, KV: KVSet<K>>(&self, kv: &KV) -> usize
    {
        self.resized(self.level_size_chooser.size_segments(kv))
    }

    #[inline] fn max_size_segments(&self, max_level_size: usize) -> usize {
        self.resized(max_level_size)
    }
}