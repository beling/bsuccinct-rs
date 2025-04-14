use std::time;

pub trait BuildStats: Copy {
    #[inline(always)] fn pre_hash(&mut self) {}
    #[inline(always)] fn pre_sort(&mut self) {}
    #[inline(always)] fn pre_seeding(&mut self) {}
    #[inline(always)] fn pre_keys_removing(&mut self) {}
    #[inline(always)] fn post_keys_removing(&mut self) {}
}

impl BuildStats for () {}

#[derive(Clone, Copy)]
pub struct BuildProgressRaport {
    timer: time::Instant,
    hash: time::Duration,
    sort: time::Duration,
    seeding: time::Duration,
    removing: time::Duration
}

impl Default for BuildProgressRaport {
    fn default() -> Self {
        Self { timer: time::Instant::now(), hash: Default::default(), sort: Default::default(), seeding: Default::default(), removing: Default::default() }
    }
}

impl BuildStats for BuildProgressRaport {
    fn pre_hash(&mut self) {
        print!("Calculating primary hashes... ");
        self.timer = time::Instant::now();
    }

    fn pre_sort(&mut self) {
        self.hash = self.timer.elapsed();
        println!("DONE in {:#.2?}", self.hash);
        print!("Sorting...");
        self.timer = time::Instant::now();
    }

    fn pre_seeding(&mut self) {
        self.sort = self.timer.elapsed();
        println!("DONE in {:#.2?}", self.sort);
        print!("Calculating seeds...");
        self.timer = time::Instant::now();
    }

    fn pre_keys_removing(&mut self) {
        self.seeding = self.timer.elapsed();
        println!("DONE in {:#.2?}", self.seeding);
        print!("Removing assigned keys...");
        self.timer = time::Instant::now();
    }

    fn post_keys_removing(&mut self) {
        self.removing = self.timer.elapsed();
        println!("DONE in {:#.2?}", self.removing);
        let total = self.hash + self.sort + self.seeding + self.removing;
        println!("{total:#.2?} total with {:.0}/{:.0}/{:.0}/{:.0} percentage shares",
            self.hash.div_duration_f64(total) * 100.0,
            self.sort.div_duration_f64(total) * 100.0,
            self.seeding.div_duration_f64(total) * 100.0,
            self.removing.div_duration_f64(total) * 100.0,
        )
    }
}