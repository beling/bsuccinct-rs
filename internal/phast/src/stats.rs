use std::time::{Duration, Instant};

#[derive(Default)]
pub struct Stats {
    /// Total size
    pub size_bytes: usize,

    /// Total building time
    pub build_time: Duration,
}

impl Stats {
    
    pub fn add(&mut self, size_bytes: usize, build_time: Duration) {
        self.size_bytes += size_bytes;
        self.build_time += build_time;
    }

    pub fn print(&self, tries: u64, key_num: usize) {
        if tries == 1 { return; }
        let total_keys = tries as usize * key_num;
        println!("Average: {:.3} bits/key, {:#.2?} build",
            (8*self.size_bytes) as f64 / total_keys as f64,
            self.build_time / tries as u32
        );
    }
}

pub fn benchmark<R, F: FnOnce() -> R>(f: F) -> (R, Duration) {
    let start_moment = Instant::now();
    let r = f();
    let time = start_moment.elapsed();
    (r, time)
}