//! Collecting and reporting building and querying statistics.

use std::io::Write;

/// Trait for collecting (and summarizing or reporting) events during construction of a minimal perfect hashing function.
pub trait BuildStatsCollector {
    /// Called once at each level to indicate sizes of input and level.
    #[inline(always)] fn level(&mut self, _input_size: usize, _level_size: usize) {}

    /// Called once at the end of the building process. Default implementation calls `level(0, 0)`.
    #[inline(always)] fn end(&mut self) { self.level(0, 0); }
}

/// Ignores all events and does nothing.
impl BuildStatsCollector for () {
    #[inline(always)] fn end(&mut self) {}
}

/// Report events occurred during building a minimal perfect hashing function to the wrapped writer.
pub struct BuildStatsPrinter<W: Write = std::io::Stdout>(W);

impl BuildStatsPrinter<std::io::Stdout> {
    /// Report events occurred during building a minimal perfect hashing function to the standard output.
    pub fn stdout() -> Self { Self(std::io::stdout()) }
}

impl<W: Write> BuildStatsCollector for BuildStatsPrinter<W> {
    fn level(&mut self, input_size: usize, level_size: usize) {
        writeln!(self.0, "{} {}", input_size, level_size).unwrap();
    }
}

/// Trait for collecting (and summarizing or reporting) events during querying of a minimal perfect hashing function.
pub trait AccessStatsCollector {
    /// Lookup algorithm calls this method to report that a value has been just found at given level (counting from 0).
    /// The single lookup can call `found_on_level` few times if it finds the fragments on value at different levels.
    #[inline(always)] fn found_on_level(&mut self, _level_nr: u32) {}

    /// Lookup algorithm calls this method to report that a value has not been found and reports number of level searched (counting from 0).
    #[inline(always)]  fn fail_on_level(&mut self, _level_nr: u32) {}
}

/// Ignores all events and does nothing.
impl AccessStatsCollector for () {}

/// Increases own value by the number of levels visited, regardless of the result of the search.
impl AccessStatsCollector for u32 {
    #[inline(always)] fn found_on_level(&mut self, level_nr: u32) { *self += level_nr + 1; }
    #[inline(always)] fn fail_on_level(&mut self, level_nr: u32) { *self += level_nr + 1; }
}

/// Increases own value by the number of levels visited, regardless of the result of the search.
impl AccessStatsCollector for u64 {
    #[inline(always)] fn found_on_level(&mut self, level_nr: u32) { *self += level_nr as u64 + 1; }
    #[inline(always)] fn fail_on_level(&mut self, level_nr: u32) { *self += level_nr as u64 + 1; }
}
