//! Tools to deal with codes.

/// `Code` represents a code which consists of a number of `bits_per_fragment`-bits fragments.
/// It is also an iterator over the code fragments.
#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub struct Code {
    /// Concatenated fragments of the codeword. The lowest bits contain the first fragment.
    /// It stores only few last fragments, and can represent the code with length > 32 bits, with zeroed few first fragments.
    pub bits: u32,
    /// Number of fragments.
    pub fragments: u32,
    /// Size of a single code fragment, in bits.
    pub bits_per_fragment: u8   // TODO remove
}

/// Returns the `fragment_nr`-th `bits_per_fragment`-bit fragment of `bits`.
pub fn get_u32_fragment(bits: u32, fragment_nr: u32, bits_per_fragment: u8) -> u32 {
    bits.checked_shr(bits_per_fragment as u32 * fragment_nr).map_or(0, |v| v & ((1u32 << bits_per_fragment as u32) - 1))
    //(bits >> (bits_per_fragment as u32 * fragment_nr as u32)) & ((1u32 << bits_per_fragment as u32) - 1)
}

impl Code {
    /// Constructs an empty code that uses `bits_per_fragment`-bits per fragment.
    pub fn new(bits_per_fragment: u8) -> Self {
        Self { bits: 0, fragments: 0, bits_per_fragment }
    }

    /// Appends the `fragment` (that must be less than 2 to the power of `self.bits_per_fragment`) to the end of `self`.
    pub fn push(&mut self, fragment: u32) {
        self.bits <<= self.bits_per_fragment;
        self.bits |= fragment;
        self.fragments += 1;
    }

    /// Gets `fragment_nr`-th fragment from the end.
    #[inline] pub fn get_r(&self, fragment_nr: u32) -> u32 {
        get_u32_fragment(self.bits, fragment_nr, self.bits_per_fragment)
        //(self.bits >> (self.bits_per_fragment as u32 * fragment_nr as u32)) & ((1u32 << self.bits_per_fragment as u32) - 1)
    }

    /// Gets `fragment_nr`-th fragment.
    #[inline] pub fn get(&self, fragment_nr: u32) -> u32 {
        self.get_r(self.fragments - fragment_nr - 1)
    }

    /// Extracts and returns first, remaining code fragment.
    pub fn extract_first(&mut self) -> u32 {
        let mask = (1u32 << self.bits_per_fragment as u32) - 1;
        self.fragments -= 1;
        let shift = self.bits_per_fragment as u32 * self.fragments as u32;
        let result = (self.bits >> shift) & mask;
        self.bits ^= result << shift;
        return result;
    }

    /// Returns whether `self` consists of zero fragments.
    #[inline] pub fn is_empty(&self) -> bool { self.fragments == 0 }
}

impl ExactSizeIterator for Code {
    /// Returns the length of `self` in fragments.
    fn len(&self) -> usize { self.fragments as usize }
}

impl Iterator for Code {
    type Item = u32;

    /// Extracts and returns the first fragment of `self` or returns `None` of `self` is empty.
    fn next(&mut self) -> Option<u32> {
        (self.fragments != 0).then(|| self.extract_first())
        //if self.fragments == 0 { None } else { Some(self.extract_first()) }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (self.fragments as usize, Some(self.fragments as usize))
    }
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn code() {
        let mut code = Code { bits: 0b_11_10_01, fragments: 3, bits_per_fragment: 2 };
        assert_eq!(code.get_r(0), 0b01);
        assert_eq!(code.get_r(1), 0b10);
        assert_eq!(code.get_r(2), 0b11);
        assert_eq!(code.get(0), 0b11);
        assert_eq!(code.get(2), 0b01);
        assert_eq!(code.fragments, 3);
        assert_eq!(code.extract_first(), 0b11);
        assert_eq!(code, Code { bits: 0b_10_01, fragments: 2, bits_per_fragment: 2 });
        assert_eq!(code.extract_first(), 0b10);
        assert_eq!(code, Code { bits: 0b_01, fragments: 1, bits_per_fragment: 2 });
        assert_eq!(code.extract_first(), 0b01);
        assert_eq!(code, Code { bits: 0, fragments: 0, bits_per_fragment: 2 });
    }
}