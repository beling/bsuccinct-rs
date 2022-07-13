use std::io::Write;

pub trait BuildStatsCollector {
    #[inline(always)] fn value_on_level(&mut self, _level_nr: u32) {}   // TODO remove?
    #[inline(always)] fn level(&mut self, _input_size: usize, _level_size: usize) {}
    #[inline(always)] fn end(&mut self) { self.level(0, 0); }
}

impl BuildStatsCollector for () {
    #[inline(always)] fn end(&mut self) {}
}

pub struct BuidStatsPrinter<W: Write = std::io::Stdout> {
    writer: W,
}

impl BuidStatsPrinter<std::io::Stdout> {
    pub fn stdout() -> Self {
        Self { writer: std::io::stdout() }
    }
}

impl<W: Write> BuidStatsPrinter<W> {
    pub fn new(writer: W) -> Self {
        Self { writer }
    }
}

impl<W: Write> BuildStatsCollector for BuidStatsPrinter<W> {
    fn level(&mut self, input_size: usize, level_size: usize) {
        writeln!(self.writer, "{} {}", input_size, level_size).unwrap();
    }
}

pub trait AccessStatsCollector {
    /// Lookup algorithm calls this method to report that a value has been just found at given level (counting from 0).
    /// The single lookup can call found_on_level few times if it finds the fragments on value at different levels.
    #[inline(always)] fn found_on_level(&mut self, _level_nr: u32) {}

    /// Lookup algorithm calls this method to report that a value has not been found and reports number of level searched (counting from 0).
    #[inline(always)]  fn fail_on_level(&mut self, _level_nr: u32) {}
}

impl AccessStatsCollector for () {}

impl AccessStatsCollector for u32 {
    #[inline(always)] fn found_on_level(&mut self, level_nr: u32) { *self += level_nr + 1; }
    #[inline(always)] fn fail_on_level(&mut self, level_nr: u32) { *self += level_nr + 1; }
}

impl AccessStatsCollector for u64 {
    #[inline(always)] fn found_on_level(&mut self, level_nr: u32) { *self += level_nr as u64 + 1; }
    #[inline(always)] fn fail_on_level(&mut self, level_nr: u32) { *self += level_nr as u64 + 1; }
}
