use std::{collections::HashMap, fs::File, io::Write};

use csf::coding::minimum_redundancy::Frequencies;

pub struct Input {
    pub keys: Box<[u32]>,
    pub values: Box<[u32]>,
    pub frequencies: HashMap::<u32, u32>,
    /// Entropy of values.
    pub entropy: f64
}

impl Input {
    pub const HEADER: &'static str = "input_len entropy";

    /*pub fn print_params(&self) {
        print!("{} keys with entropy {:.2}", self.keys.len(), self.entropy);
    }*/

    pub fn print_params_to(&self, file: &mut Option<File>) {
        if let Some(ref mut f) = file {
            write!(f, "{} {}", self.keys.len(), self.entropy).unwrap();
        }
    }
}

impl From<(Box<[u32]>, Box<[u32]>, HashMap<u32, u32>)> for Input {
    fn from((keys, values, frequencies): (Box<[u32]>, Box<[u32]>, HashMap<u32, u32>)) -> Self {
        let entropy = frequencies.entropy();
        Self { keys, values, frequencies, entropy }
    }
}

impl From<(Box<[u32]>, Box<[u32]>)> for Input {
    fn from((keys, values): (Box<[u32]>, Box<[u32]>)) -> Self {
        let frequencies = HashMap::<u32, u32>::with_counted_all(values.iter());
        (keys, values, frequencies).into()
    }
}

impl From<(Box<[u32]>, Box<[u32]>, f64)> for Input {
    fn from((keys, values, entropy): (Box<[u32]>, Box<[u32]>, f64)) -> Self {
        let frequencies = HashMap::<u32, u32>::with_counted_all(values.iter());
        Self { keys, values, frequencies, entropy }
    }
}

// Normalize vector of values_weights by dividing its elements by their sum.
/*pub fn normalize(values_weights: &mut [f64]) {
    let sum = FSum::with_all(values_weights.iter()).value();
    for v in values_weights { *v /= sum; }
}*/

// Construct benchmark data with:
// - length close to len;
// - occurrence of i-th value proportional to i-th cell of normalized_values_weights (but minimum 1);
//     normalized_values_weights must be normalized, i.e. sum of its cells has to be 1.0.
/*pub fn kv_from_normalized(len: usize, normalized_values_weights: &[f64]) -> (Box<[u32]>, Box<[u32]>) {
    //dbg!(normalized_values_weights);
    let mut keys = Vec::with_capacity(len + normalized_values_weights.len());
    let mut values = Vec::with_capacity(len + normalized_values_weights.len());
    let mut key = 0u32;
    let mut value = 0u32;
    for w in normalized_values_weights {
        let l = ((*w * len as f64).round() as u32).max(1);
        keys.extend((0..l).map(|i| key + i));
        values.extend((0..l).map(|_| value));
        key += l;
        value += 1;
    }
    (keys.into_boxed_slice(), values.into_boxed_slice())
}*/

// Construct benchmark data with:
// - length close to len,
// - occurrence of i-th value proportional to i-th cell of values_weights (but minimum 1).
/*pub fn kv(len: usize, values_weights: &mut [f64]) -> (Box<[u32]>, Box<[u32]>) {
    normalize(values_weights);
    kv_from_normalized(len, values_weights)
}*/

// Construct benchmark data with:
// - length close to len,
// - occurrence of i-th value proportional to i-th cell of values_weights (but minimum 1).
/*pub fn kv_intw(len: usize, values_weights: &[usize]) -> (Box<[u32]>, Box<[u32]>) {
    let mut c = values_weights.iter().map(|v| *v as f64).collect::<Box<_>>();
    kv(len, &mut c)
}*/

// Construct benchmark data with:
// - length `len`,
// - number of different values `different_values`,
// - possibly equal occurrence of each value.
/*pub fn kv_equals(len: u32, different_values: u32) -> (Box<[u32]>, Box<[u32]>) {
    ((0..len).collect(), (0..len).map(|v| v%different_values).collect())
}*/

// Construct benchmark data with:
// - length `len`,
// - number of different values `different_values`,
// - occurrence of each value except one is one.
/*pub fn kv_dominated(len: u32, mut different_values: u32) -> (Box<[u32]>, Box<[u32]>) {
    different_values -= 1;
    (
        (0..len).collect(),
        (0..different_values).chain((different_values..len).map(|_| different_values)).collect()
    )
}*/

/// Construct benchmark data with:
/// - length `len`,
/// - number of different values `different_values`,
/// - occurrence of each value except one is `lo_count`.
pub fn kv_dominated_lo(len: u32, mut different_values: u32, lo_count: u32) -> (Box<[u32]>, Box<[u32]>) {
    different_values -= 1;  // without last which dominates
    let non_dominated_count = different_values * lo_count;
    (
        (0..len).collect(),
        (0..non_dominated_count).map(|v| v % different_values)
            .chain((non_dominated_count..len).map(|_| different_values)).collect()
    )
}

pub fn kv_dominated_lo_entropy(len: u32, mut different_values: u32, lo_count: u32) -> f64 {
    different_values -= 1;
    let dominated_count = (len - different_values * lo_count) as f64 / len as f64;
    let lo_count = lo_count as f64 / len as f64;
    return different_values as f64 * -lo_count * lo_count.log2()
        - dominated_count * dominated_count.log2();
}

/*pub fn kv_linear(len: usize, mut different_values: u32, delta: f64) -> (Box<[u32]>, Box<[u32]>) {
    different_values -= 1;
    kv_from_normalized(
        len,
        (0..=different_values).map(|v| (1.0 + delta * ((2*v) as f64/different_values as f64-1.0)) / 2.0).collect::<Box<[f64]>>().as_ref()
    )
}*/



