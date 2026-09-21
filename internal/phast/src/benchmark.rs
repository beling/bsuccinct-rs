use std::time::{Duration, Instant};

use butils::UnitPrefix;

use crate::Conf;

#[derive(Default)]
pub struct Result {
    /// Total size
    pub size_bytes: usize,

    /// Total building time
    pub build_time: Duration,

    /// Total query time
    pub evaluation_time: Duration,

    /// Total number of bumped keys
    pub bumped_keys: usize,

    /// Number of builds without bumping
    pub bumpless_builds: u32,

    /// Total output range, sum of output ranges of tries
    pub range: usize,

    /// Number of levels or tries to build
    pub levels: usize,
}

impl std::ops::AddAssign for Result {
    fn add_assign(&mut self, rhs: Self) {
        self.size_bytes += rhs.size_bytes;
        self.build_time += rhs.build_time;
        self.evaluation_time += rhs.evaluation_time;
        self.bumped_keys += rhs.bumped_keys;
        self.bumpless_builds += rhs.bumpless_builds;
        self.range += rhs.range;
        self.levels += rhs.levels;
    }
}

/// Estimates bits per element for an Elias-Fano non-decreasing sequence in `0..output_range`.
/// `keys_to_map` must be positive and may be fractional when averaging multiple tries.
/// Metadata, select indexes and word padding are omitted.
///
/// With `l` low bits, each element costs `l + 1 + ceil(output_range / 2^l) / keys_to_map` bits.
/// This counts one zero per high-bit interval, including the optional trailing zero.
/// The low-bit width cannot be negative, even when repeated values make the element
/// count exceed the output range.
fn elias_fano_cost(keys_to_map: f64, output_range: f64) -> f64 {
    let low_bits = (output_range / keys_to_map).log2().floor().max(0.0);
    low_bits + 1.0 + (output_range / 2f64.powf(low_bits)).ceil() / keys_to_map
    // simpler formula almost as accurate as above: low_bits + 1.0 + output_range as f64 / (keys_to_map * 2f64.powf(low_bits))
}

/// Estimates the number of keys that remain bumped (unassigned) after the output range of a function
/// is expanded from `used_range` to `final_range`, which must not be smaller than `used_range`.
/// The so far unused part of the range is assumed to assign keys with the same effectiveness
/// as the part used so far, i.e. to assign `assigned`/`used_range` keys per unit of the range.
/// As it can take over only the keys bumped so far, the result is never negative.
/// Ranges and key numbers are totals over all tries; `used_range` must be positive.
fn final_bumped_keys(bumped: f64, assigned: f64, used_range: f64, final_range: f64) -> f64 {
    (bumped - (final_range - used_range) * assigned / used_range).max(0.0)
}

impl Result {

    #[inline(never)]
    pub fn print(&self, tries: u32, key_num: u32, evals_per_try: u32, k: u16) {
        let minimum_range = key_num.div_ceil(k as u32);
        let total_keys = tries as usize * key_num as usize;
        
        let bits_per_key = (8*self.size_bytes) as f64 / total_keys as f64;
        print!("{bits_per_key:.3} bits/key");

        let minimum_range_x_tries = minimum_range as usize * tries as usize;
        let bumped_share = self.bumped_keys as f64 / total_keys as f64;
        
        let mut bits_per_key_final = bits_per_key;  // virtual, final value after using remaining range with same efficiency as before
        let mut bumped_keys_final = self.bumped_keys as f64;
        let mut bumped_share_final = bumped_share;
        if self.range < minimum_range_x_tries {   // overloading
            bits_per_key_final *= minimum_range_x_tries as f64 / self.range as f64; // we need extra space for storing seeds of unused range
            // the unused range is assumed to assign keys with the same efficiency as the used one,
            // so it takes over part of the keys bumped so far
            bumped_keys_final = final_bumped_keys(
                self.bumped_keys as f64,
                (total_keys - self.bumped_keys) as f64,
                self.range as f64,
                minimum_range_x_tries as f64,
            );
            bumped_share_final = bumped_keys_final / total_keys as f64;
        }
        let mut repair_cost_per_key = 0.0;
        if bumped_keys_final != 0.0 {   // adds cost of repairing bumped keys
            repair_cost_per_key += 
                (elias_fano_cost(bumped_keys_final / tries as f64, minimum_range as f64)
                + 2.0) * bumped_share_final;    // 2.0 bits/key is a cost of building MPHF for bumped keys
        }
        if self.range > minimum_range_x_tries { // adds cost of shrinking output range to minimal one
            // here *_final has same values as normal counterparts
            let total_keys_to_map = (self.range - minimum_range_x_tries) as f64;
            repair_cost_per_key += (elias_fano_cost(total_keys_to_map / tries as f64, minimum_range as f64)
                + if k > 1 { 2.0 } else { 0.0 }) * total_keys_to_map / total_keys as f64;
                // if k>1 we assume that we build MPHF for keys with values >minimum_range from scratch, using 2.0 bits/key
        }
        if repair_cost_per_key != 0.0 { print!(" (≈{:.3} MPHF)", bits_per_key_final + repair_cost_per_key) }
        if self.bumped_keys != 0 || self.range != minimum_range_x_tries { // α = number of mapped keys / number of slots
            print!(", α={:.1}%", (100 * (total_keys - self.bumped_keys as usize)) as f64 / (self.range * k as usize) as f64);
        }
        if self.bumped_keys != 0 {
            print!(", {:.2}%", bumped_share * 100.0);
            if bumped_share != bumped_share_final { print!(" ({:.2}%)", bumped_share_final * 100.0) }
            print!(" bumped");
        }
        if tries > 1 && self.bumpless_builds != tries {
            print!(", {}/{tries}={:.0}% bumpless", self.bumpless_builds, 100.0 * self.bumpless_builds as f64 / tries as f64)
        }
        if self.range > minimum_range_x_tries {
            print!(", {:.2}% over the minimum range", ((self.range - minimum_range_x_tries) * 100) as f64 / minimum_range_x_tries as f64)
        } /*else if self.range < minimum_range_x_tries {
            print!(", {:.2}% under the minimum range", ((minimum_range_x_tries - self.range) * 100) as f64 / minimum_range_x_tries as f64)
        }*/
        if tries == 1 { print!(", {} levels", self.levels) } else { print!(", {:.2} levels", self.levels as f64 / tries as f64); }
        print!(", {:#.2?} build", self.build_time / tries as u32);
        if evals_per_try != 0 {
            print!(", {:.2?}ns/key evaluation", self.evaluation_time.as_secs_f64().as_nanos() / (total_keys * evals_per_try as usize) as f64)
        }
        println!();
    }

    #[inline(never)]
    pub fn print_avg_csv(&self, conf: &Conf) {
        conf.print_csv();
        let tries = conf.tries();
        let total_keys = tries as f64 * conf.keys_num as f64;
        let minimum_range = conf.minimum_range() as usize * tries as usize;
        println!(" {tries} {} {} {} {} {}",
            (8*self.size_bytes) as f64 / total_keys,
            (self.bumped_keys * 100) as f64 / total_keys,
            ((self.range - minimum_range) * 100) as f64 / minimum_range as f64,
            (self.build_time.as_secs_f64() / total_keys).as_nanos(),
            (self.evaluation_time.as_secs_f64() / (total_keys * conf.evaluations as f64)).as_nanos()
        );
    }

    /*pub fn print_csv(&self, try_nr: u32, conf: &Conf) {
        conf.print_csv();
        let keys = conf.keys_num as f64;
        let minimum_range = conf.minimum_range() as usize;
        print!(", {try_nr}, {:.3}, {:.2}, {:.2}, {:.2}, {:.2}",
            (8*self.size_bytes) as f64 / keys,
            (self.bumped_keys * 100) as f64 / keys,
            ((self.range - minimum_range) * 100) as f64 / minimum_range as f64,
            (self.build_time.as_secs_f64() / keys).as_nanos(),
            (self.evaluation_time.as_secs_f64() / keys).as_nanos()
        );
    }*/

    #[inline(never)]
    pub fn print_try(&self, try_nr: u32, conf: &Conf) {
        if conf.csv || conf.less { return; }
        if conf.many_tries() { print!("{try_nr}: "); }
        self.print(1, conf.keys_num, conf.evaluations, conf.k);
    }

    #[inline(never)]
    pub fn print_avg(&self, conf: &Conf) {
        if conf.csv { self.print_avg_csv(conf); return; }
        if !conf.many_tries() { return; }
        print!("Average: ");
        self.print(conf.tries(), conf.keys_num, conf.evaluations, conf.k);
    }
}

pub fn benchmark<R, F: FnOnce() -> R>(f: F) -> (R, Duration) {
    let start_moment = Instant::now();
    let r = f();
    let time = start_moment.elapsed();
    (r, time)
}

/// Tests for the estimates used in benchmark output.
#[cfg(test)]
mod tests {
    use super::{elias_fano_cost, final_bumped_keys};

    /// Sparse sequences include both low bits and the unary high-bit vector.
    #[test]
    fn elias_fano_sparse() {
        assert_eq!(elias_fano_cost(4.0, 64.0), 6.0);
        assert_eq!(elias_fano_cost(4.0, 48.0), 5.5);
    }

    /// A partially filled high-bit interval still needs a whole unary zero.
    #[test]
    fn elias_fano_rounds_up_high_intervals() {
        assert_eq!(elias_fano_cost(4.0, 49.0), 5.75);
    }

    /// Equal element count and range need no low bits.
    #[test]
    fn elias_fano_equal_count_and_range() {
        assert_eq!(elias_fano_cost(8.0, 8.0), 2.0);
    }

    /// Repeated values can make the element count exceed the output range.
    #[test]
    fn elias_fano_dense() {
        assert_eq!(elias_fano_cost(16.0, 4.0), 1.25);
        // Counts averaged over multiple tries need not be integers.
        assert_eq!(elias_fano_cost(2.5, 1.0), 1.4);
    }

    /// The reported k=1000 case must add a positive repair cost to the base size.
    #[test]
    fn elias_fano_large_k_repair() {
        let key_num = 1_000_000.0;
        let bumped_keys = 61_500.0;
        let repair_cost = (elias_fano_cost(bumped_keys, 1000.0) + 2.0) * bumped_keys / key_num;
        assert!((repair_cost - 0.1855).abs() < 1e-12);
    }

    /// The unused part of the range takes over bumped keys with the effectiveness of the used one.
    #[test]
    fn final_bumped_keys_expansion() {
        // k=1, n=1000: 600 keys assigned in 800 values; 200 more values take over 150 keys.
        assert_eq!(final_bumped_keys(200.0, 600.0, 800.0, 1000.0), 50.0);
        // Full effectiveness of the used range (alpha=100%): all bumped keys are taken over.
        assert_eq!(final_bumped_keys(1.0, 4.0, 2.0, 3.0), 0.0); // k=2, n=5
        assert_eq!(final_bumped_keys(100.0, 900.0, 900.0, 1000.0), 0.0); // k=1, n=1000
        // Nothing to expand: all bumped keys stay bumped.
        assert_eq!(final_bumped_keys(200.0, 600.0, 1000.0, 1000.0), 200.0);
    }
}
