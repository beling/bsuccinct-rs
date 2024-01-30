use crate::{BitsPerFragment, Coding, DecodingResult, TreeDegree};

/// Decoder that decodes a value for given code, consuming one codeword fragment at a time.
///
/// Time complexity of decoding the whole code is:
/// - pessimistic: *O(length of the longest code)*
/// - expected: *O(log(number of values, i.e. length of coding.values))*
/// - optimistic: *O(1)*
///
/// Memory complexity: *O(1)*
pub struct Decoder<'huff, ValueType, D = BitsPerFragment> {
    coding: &'huff Coding<ValueType, D>,
    /// shift+fragment is a current position (node number, counting from the left) at current level.
    shift: u32,
    /// Number of leafs at all previous levels.
    first_leaf_nr: u32,
    /// Number of the current level.
    level: u32,
    /// Current level size = number of: internal nodes + leaves.
    level_size: u32
}

impl<'huff, ValueType, D: TreeDegree> Decoder<'huff, ValueType, D> {
    /// Constructs decoder for given `coding`.
    pub fn new(coding: &'huff Coding<ValueType, D>) -> Self {
        Self {
            coding,
            shift: 0,
            first_leaf_nr: 0,
            level: 0,
            level_size: coding.degree.as_u32(),
        }
    }

    /// Resets `self` to initial state and makes it ready to decode next value.
    pub fn reset(&mut self) {
        self.shift = 0;
        self.first_leaf_nr = 0;
        self.level = 0;
        self.level_size = self.coding.degree.as_u32();
    }

    /// Returns the number of fragments consumed since construction or last reset.
    #[inline(always)] pub fn consumed_fragments(&self) -> u32 { self.level }

    /// Consumes a `fragment` of the codeword and returns:
    /// - a value if the given `fragment` finishes the valid codeword;
    /// - an [`DecodingResult::Incomplete`] if the codeword is incomplete and the next fragment is needed;
    /// - or [`DecodingResult::Invalid`] if the codeword is invalid (possible only for bits per fragment > 1).
    ///
    /// Result is undefined if `fragment` exceeds `tree_degree`.
    pub fn consume(&mut self, fragment: u32) -> DecodingResult<&'huff ValueType> {
        self.shift += fragment;
        let internal_nodes_count = self.internal_nodes_count();
        return if self.shift < internal_nodes_count {    // internal node, go level down
            self.shift = self.coding.degree * self.shift;
            self.first_leaf_nr += self.level_size - internal_nodes_count;    // increase by number of leafs at current level
            self.level_size = self.coding.degree * internal_nodes_count;
            self.level += 1;
            DecodingResult::Incomplete
        } else {    // leaf, return value or Invalid
            self.coding.values.get((self.first_leaf_nr + self.shift - internal_nodes_count) as usize).into()
            //self.coding.values.get((self.first_leaf_nr + self.level_size + self.shift) as usize).into()
        }
    }

    /// Consumes a `fragment` of the codeword and returns:
    /// - a value if the given `fragment` finishes the valid codeword;
    /// - an [`DecodingResult::Incomplete`] if the codeword is incomplete and the next fragment is needed;
    /// - or [`DecodingResult::Invalid`] if the codeword is invalid (possible only for `degree` greater than 2)
    ///     or `fragment` is not less than `degree`.
    #[inline(always)] pub fn consume_checked(&mut self, fragment: u32) -> DecodingResult<&'huff ValueType> {
        if fragment < self.coding.degree.as_u32() {
            self.consume(fragment)
        } else {
            DecodingResult::Invalid
        }
    }

    /// Tries to decode and return a single value from the `fragments` iterator,
    /// consuming as many fragments as needed.
    /// 
    /// Returns [`DecodingResult::Incomplete`] if the iterator exhausted before the value was decoded
    /// ([`Self::consumed_fragments`] enables checking if the iterator yielded any fragment before exhausting).
    /// Returns [`DecodingResult::Invalid`] if obtained invalid codeword (possible only for `degree` greater than 2).
    pub fn decode<I: Iterator<Item = u32>>(&mut self, fragments: &mut I) -> DecodingResult<&'huff ValueType> {
        loop {
            let fragment = match fragments.next() {
                Some(fragment) => fragment,
                None => return DecodingResult::Incomplete
            };
            let result = self.consume(fragment);
            match result {
                DecodingResult::Incomplete => {},
                _ => { return result; }
            }
        }
    }

    /// Tries to decode and return a single value from the `fragments` iterator,
    /// consuming as many fragments as needed.
    /// If successful, it [resets](Self::reset) `self` to be ready to decode the next value.
    /// 
    /// Returns [`DecodingResult::Incomplete`] if the iterator exhausted before the value was decoded
    /// ([`Self::consumed_fragments`] enables checking if the iterator yielded any fragment before exhausting).
    /// Returns [`DecodingResult::Invalid`] if obtained invalid codeword (possible only for `degree` greater than 2).
    pub fn decode_next<F: Into<u32>, I: Iterator<Item = F>>(&mut self, fragments: &mut I) -> DecodingResult<&'huff ValueType> {
        loop {
            let fragment = match fragments.next() {
                Some(fragment) => fragment,
                None => return DecodingResult::Incomplete
            };
            let result = self.consume(fragment.into());
            match result {
                DecodingResult::Value(_) => {
                    self.reset();
                    return result;
                },
                DecodingResult::Invalid => { return result; }
                DecodingResult::Incomplete => {},
            }
        }
    }

    /*pub fn decode_next<I: Iterator<Item = u32>>(&mut self, fragments: &mut I) -> DecodingResult<&'huff ValueType> {
        let result = self.decode(fragments);
        match result {
            DecodingResult::Value(_) => {
                self.reset();
                result
            },
            _ => { result }
        }
    }*/

    /// Returns number of internal (i.e. non-leafs) nodes at the current level of the tree.
    #[inline(always)] fn internal_nodes_count(&self) -> u32 {
        self.coding.internal_nodes_count[self.level as usize]
    }
}
