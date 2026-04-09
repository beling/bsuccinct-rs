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

    /// Total output range, sum of output ranges of tries
    pub range: usize,
}

impl std::ops::AddAssign for Result {
    fn add_assign(&mut self, rhs: Self) {
        self.size_bytes += rhs.size_bytes;
        self.build_time += rhs.build_time;
        self.evaluation_time += rhs.evaluation_time;
        self.bumped_keys += rhs.bumped_keys;
        self.range += rhs.range;
    }
}

/// Returns the cost (in bits per mapped key) of mapping `keys_to_map` keys
/// to given `output_range` by Elias-Fano.
fn elias_fano_cost(keys_to_map: f64, output_range: u32) -> f64 {
    (output_range as f64 / keys_to_map).log2()/*.ceil()*/ + 2.0
}

impl Result {

    #[inline(never)]
    pub fn print(&self, tries: u32, key_num: u32, evals_per_try: u32, minimum_range: u32, k: u16) {
        let total_keys = tries as usize * key_num as usize;
        
        let bits_per_key = (8*self.size_bytes) as f64 / total_keys as f64;
        print!("{bits_per_key:.3} bits/key");
        let bumped_share = self.bumped_keys as f64 / total_keys as f64;
        let mut repair_cost_per_key = 0.0;
        if self.bumped_keys != 0 {
            repair_cost_per_key += 
                (elias_fano_cost(self.bumped_keys as f64 / tries as f64, minimum_range)
                + 2.0) * bumped_share;    // 2.0 bits/key is a cost of building MPHF for bumped keys
        }
        let minimum_range_x_tries = minimum_range as usize * tries as usize;
        if self.range != minimum_range_x_tries {
            let total_keys_to_map = (self.range - minimum_range_x_tries) as f64;
            repair_cost_per_key += (elias_fano_cost(total_keys_to_map / tries as f64, minimum_range)
                + if k > 1 { 2.0 } else { 0.0 }) * total_keys_to_map / total_keys as f64;
                // if k>1 we assume that we build MPHF for keys with values >minimum_range from scratch, using 2.0 bits/key
        }
        if repair_cost_per_key != 0.0 { print!(" (≈{:.3} MPHF)", bits_per_key + repair_cost_per_key) }
        if self.bumped_keys != 0 {
            let bumped_percent = bumped_share * 100.0;
            print!(", {:.2}% bumped, α={:.1}%", bumped_percent, 100.0-bumped_percent);
        }
        if self.range != minimum_range_x_tries {
            print!(", {:.2}% over the minimum range", ((self.range - minimum_range_x_tries) * 100) as f64 / minimum_range_x_tries as f64)
        }
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
        self.print(1, conf.keys_num, conf.evaluations, conf.minimum_range(), conf.k);
    }

    #[inline(never)]
    pub fn print_avg(&self, conf: &Conf) {
        if conf.csv { self.print_avg_csv(conf); return; }
        if !conf.many_tries() { return; }
        print!("Average: ");
        self.print(conf.tries(), conf.keys_num, conf.evaluations, conf.minimum_range(), conf.k);
    }
}

pub fn benchmark<R, F: FnOnce() -> R>(f: F) -> (R, Duration) {
    let start_moment = Instant::now();
    let r = f();
    let time = start_moment.elapsed();
    (r, time)
}