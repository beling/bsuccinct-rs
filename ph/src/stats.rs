//! Collecting and reporting building and querying statistics
//! of minimal perfect hash functions.

use std::io::Write;

/// Trait for collecting (and summarizing or reporting) events during construction of a minimal perfect hashing function.
pub trait BuildStatsCollector {
    /// Called once at each level to indicate sizes of input and level. Default implementation does nothing.
    #[inline(always)] fn level(&mut self, _input_size: usize, _level_size: usize) {}

    /// Called once at the end of the building process with the number of remaining (unassigned) keys
    /// (`0` when construction has been successful). Default implementation does nothing.
    #[inline(always)] fn end(&mut self, _remaining_keys: usize) {}
}

/// Ignores all events and does nothing.
impl BuildStatsCollector for () {}

/// Reports events that occurred during building a minimal perfect hashing function to the wrapped writer
/// (each event is written as a separate line).
pub struct BuildStatsPrinter<W: Write = std::io::Stdout>(W);

impl BuildStatsPrinter<std::io::Stdout> {
    /// Reports events that occurred during building a minimal perfect hashing function to the standard output.
    pub fn stdout() -> Self { Self(std::io::stdout()) }
}

impl<W: Write> BuildStatsCollector for BuildStatsPrinter<W> {
    fn level(&mut self, input_size: usize, level_size: usize) {
        writeln!(self.0, "{} {}", input_size, level_size).unwrap();
    }

    fn end(&mut self, remaining_keys: usize) {
        writeln!(self.0, "Completed {}. {} keys remaining.", if remaining_keys == 0 { "successfully" } else { "unsuccessfully" }, remaining_keys).unwrap();
    }
}

/// Trait for collecting (and summarizing or reporting) events during querying of a minimal perfect hashing function.
pub trait AccessStatsCollector {
    /// The lookup algorithm calls this method to report that a value has been just found at the given level (counting from 0).
    /// A single lookup may call `found_on_level` a few times, if it finds the fragments of the value at different levels.
    #[inline(always)] fn found_on_level(&mut self, _level_nr: usize) {}

    /// The lookup algorithm calls this method to report that a value has not been found,
    /// together with the number of levels searched (counting from 0).
    #[inline(always)] fn fail_on_level(&mut self, _level_nr: usize) {}
}

/// Ignores all events and does nothing.
impl AccessStatsCollector for () {}

/// Reports the number of levels visited during lookups by increasing its own value
/// by this number, regardless of the lookup result (so `self` grows by at least 1 for each reported event).
impl AccessStatsCollector for usize {
    #[inline(always)] fn found_on_level(&mut self, level_nr: usize) { *self += level_nr + 1; }
    #[inline(always)] fn fail_on_level(&mut self, level_nr: usize) { *self += level_nr + 1; }
}
