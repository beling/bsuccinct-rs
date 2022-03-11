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
    /// Current level size = number of: internal nodes + leaves.
    level_size: u32,
    /// Number of the current level.
    level: u32
}

impl<'huff, ValueType, D: TreeDegree> Decoder<'huff, ValueType, D> {
    /// Constructs decoder for given `coding`.
    pub fn new(coding: &'huff Coding<ValueType, D>) -> Self {
        Self {
            coding,
            shift: 0,
            first_leaf_nr: 0,
            level_size: coding.degree.as_u32(),
            level: 0
        }
    }

    /// Consumes a `fragment` of the codeword and returns:
    /// - a value if the given `fragment` finishes the valid codeword;
    /// - an `DecodingResult::Incomplete` if the codeword is incomplete and the next fragment is needed;
    /// - or `DecodingResult::Invalid` if the codeword is invalid (possible only for bits per fragment > 1).
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
    /// - an `DecodingResult::Incomplete` if the codeword is incomplete and the next fragment is needed;
    /// - or `DecodingResult::Invalid` if the codeword is invalid (possible only for `degree` greater than 2)
    ///     or `fragment` is not less than `degree`.
    #[inline(always)] pub fn consume_checked(&mut self, fragment: u32) -> DecodingResult<&'huff ValueType> {
        if fragment < self.coding.degree.as_u32() {
            self.consume(fragment)
        } else {
            DecodingResult::Invalid
        }
    }

    /// Returns number of internal (i.e. non-leafs) nodes at the current level of the tree.
    #[inline(always)] fn internal_nodes_count(&self) -> u32 {
        self.coding.internal_nodes_count[self.level as usize]
    }
}
