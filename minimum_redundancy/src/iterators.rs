use std::iter::FusedIterator;
use crate::{Code, Coding, TreeDegree};

/// Iterator over the levels of the huffman tree.
///
/// For each level of the tree, it exposes the tuple that consists of:
/// - values assigned to the leafs at the current level,
/// - number of internal nodes at the level, which equals the bits of codeword assigned to the first leaf at the level,
/// - index of the level in the tree, which equals to the length of the codewords assigned to leafs at the level.
#[derive(Copy, Clone)]
pub struct LevelIterator<'coding, ValueType, D> {
    /// Huffman tree to iterate over.
    coding: &'coding Coding<ValueType, D>,
    /// Index of the last value exposed.
    last_value_index: usize,
    /// Size of the whole current level, sum of numbers of: internal nodes, leafs, unused indices (only at the last level)
    level_size: u32,
    /// Index of level of the tree, which is equal to the length of the codewords assigned to leafs at this level.
    level: u32
}

impl<'coding, ValueType, D: TreeDegree> LevelIterator<'coding, ValueType, D> {
    /// Returns iterator over levels of `coding`.
    pub fn new(coding: &'coding Coding<ValueType, D>) -> Self {
        Self {
            coding,
            level_size: coding.degree.as_u32(),
            last_value_index: 0,
            level: 0
        }
    }
}

impl<'coding, ValueType, D: TreeDegree> FusedIterator for LevelIterator<'coding, ValueType, D> {}

impl<'coding, ValueType, D: TreeDegree> ExactSizeIterator for LevelIterator<'coding, ValueType, D> {
    fn len(&self) -> usize {
        self.coding.internal_nodes_count.len() - self.level as usize
    }
}

impl<'coding, ValueType, D: TreeDegree> Iterator for LevelIterator<'coding, ValueType, D> {
    type Item = (&'coding [ValueType], u32, u32);

    fn next(&mut self) -> Option<Self::Item> {
        (self.last_value_index != self.coding.values.len()).then(|| {
            let value_index = self.last_value_index;
            let internal_nodes = self.coding.internal_nodes_count[self.level as usize];
            self.level += 1;
            let leaves_count = self.level_size - internal_nodes;
            self.last_value_index = (value_index + leaves_count as usize).min(self.coding.values.len());
            self.level_size = self.coding.degree * internal_nodes;
            (&self.coding.values[value_index..self.last_value_index], internal_nodes, self.level)
        })
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}

/// Iterator over value-codeword pairs.
#[derive(Copy, Clone)]
pub struct CodesIterator<'coding, ValueType, D> {
    /// Iterator over levels.
    level_iterator: LevelIterator<'coding, ValueType, D>,
    /// Index in values that points the value about to be exposed.
    value_index: usize,
    /// Content of the codeword about to be exposed (the codeword length equals `self.level_iterator.level`).
    bits: u32,
}

impl<'coding, ValueType, D: TreeDegree> CodesIterator<'coding, ValueType, D> {
    pub fn new(coding: &'coding Coding<ValueType, D>) -> Self {
        Self {
            level_iterator: LevelIterator::new(coding),
            value_index: 0,
            bits: 0
        }
    }
}

impl<'coding, ValueType, D: TreeDegree> FusedIterator for CodesIterator<'coding, ValueType, D> {}

impl<'coding, ValueType, D: TreeDegree> ExactSizeIterator for CodesIterator<'coding, ValueType, D> {
    fn len(&self) -> usize {
        self.level_iterator.coding.values.len() - self.value_index
    }
}

impl<'coding, ValueType, D: TreeDegree> Iterator for CodesIterator<'coding, ValueType, D> {
    type Item = (&'coding ValueType, Code);

    fn next(&mut self) -> Option<Self::Item> {
        while self.value_index == self.level_iterator.last_value_index {
            let (_, first_code_bits, _) = self.level_iterator.next()?;
            self.bits = first_code_bits;
        }
        let result = (&self.level_iterator.coding.values[self.value_index],
                          Code{ content: self.bits, len: self.level_iterator.level });
        self.value_index += 1;
        self.bits += 1;
        Some(result)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.len();
        (len, Some(len))
    }
}
