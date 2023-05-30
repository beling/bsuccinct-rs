use bitm::{BitAccess, BitVec, n_lowest_bits};

/// Solves value collisions during construction of BBMap.
pub trait CollisionSolver {
    /// Returns true if `index` is under collision and should not be farther processed.
    fn is_under_collision(&self, index: usize) -> bool;

    /// Try to assign value (`bits_per_fragment` bits of `fragment`) to the given `index` which is not under collision.
    fn process_fragment(&mut self, index: usize, fragment: u8, bits_per_fragment: u8);

    /// Array that shows indices which have assigned values and are not under collision.
    fn to_collision_array(self) -> Box<[u64]>;

    fn construct_value_array(number_of_values: usize, bits_per_fragment: u8) -> Box<[u64]> {
        Box::<[u64]>::with_zeroed_bits(number_of_values*bits_per_fragment as usize)
    }

    /// Set `index`-th value in final `output` (which is an array of `bits_per_fragment` bits values) to `fragment`.
    #[inline(always)] fn set_value(output: &mut [u64], index: usize, fragment: u8, bits_per_fragment: u8) {
        output.init_fragment(index, fragment as u64, bits_per_fragment);
    }
}

/// Builds `CollisionSolver`.
pub trait CollisionSolverBuilder {
    /// Type of collision solver that is build by `self`.
    type CollisionSolver: CollisionSolver;

    /// Constructs `CollisionSolver` for given number of values (64*`level_size_segments`) and `bits_per_fragment`.
    /// The solver supports indices in range [0, 64*`level_size_segments`) and values of the size of `bits_per_fragment` bits.
    fn new(&self, level_size_segments: u32, bits_per_fragment: u8) -> Self::CollisionSolver;

    /// Gets whether the `new` method, with the current parameters, returns the collision solver that is lossless.
    fn is_lossless(&self) -> bool;
}

/// Shows that the builder always produces the collision solver that is lossless and thus can be used with compressed BBmap.
pub trait IsLossless: CollisionSolverBuilder {} // TODO: maybe check only in runtime by is_lossless method


/// BBMap collision solver that permits assigning only one value (few equal values) to each index.
pub struct LoMemAcceptEqualsSolver {
    /// Which indices are under collision.
    collided: Box<[u64]>,
    /// Fragments assigned to indices.
    fragments: Box<[u64]>,
    /// Which indices have assigned values and are not under collision.
    current_array: Box<[u64]>
}

impl LoMemAcceptEqualsSolver {
    pub(crate) fn new(level_size_segments: u32, bits_per_fragment: u8) -> Self {
        Self {
            collided: Box::<[u64]>::with_zeroed_64bit_segments(level_size_segments as usize),
            fragments: Box::<[u64]>::with_zeroed_64bit_segments(level_size_segments as usize * bits_per_fragment as usize),
            current_array: Box::<[u64]>::with_zeroed_64bit_segments(level_size_segments as usize)
        }
    }
}

impl CollisionSolver for LoMemAcceptEqualsSolver {
    #[inline(always)] fn is_under_collision(&self, index: usize) -> bool {
        self.collided.get_bit(index)
    }

    fn process_fragment(&mut self, index: usize, fragment: u8, bits_per_fragment: u8) {
        if !self.current_array.get_bit(index) { // empty:
            self.current_array.set_bit(index);
            self.fragments.init_fragment(index, fragment as _, bits_per_fragment);
        } else if /*fragments[a_index]*/ self.fragments.get_fragment(index, bits_per_fragment) as u8 != fragment {
            self.collided.set_bit(index);
            self.current_array.clear_bit(index);
        }
    }

    fn to_collision_array(self) -> Box<[u64]> {
        self.current_array
    }
}

#[derive(Default, Copy, Clone)]
pub struct LoMemAcceptEquals;

impl CollisionSolverBuilder for LoMemAcceptEquals {
    type CollisionSolver = LoMemAcceptEqualsSolver;

    #[inline(always)] fn new(&self, level_size_segments: u32, bits_per_fragment: u8) -> Self::CollisionSolver {
        Self::CollisionSolver::new(level_size_segments, bits_per_fragment)
    }

    #[inline(always)] fn is_lossless(&self) -> bool { true }
}

impl IsLossless for LoMemAcceptEquals {}


/// BBMap collision solver that permits assigning only one value (few equal values) to each index.
pub struct AcceptEqualsSolver {
    /// Which indices are under collision.
    collided: Box<[u64]>,
    /// Fragments assigned to indices (uses 1 byte / value).
    fragments: Box<[u8]>,
    /// Which indices have assigned values and are not under collision.
    current_array: Box<[u64]>
}

impl AcceptEqualsSolver {
    fn new(level_size_segments: u32, _bits_per_fragment: u8) -> Self {
        Self {
            collided: Box::<[u64]>::with_zeroed_64bit_segments(level_size_segments as usize),
            fragments: vec![0u8; level_size_segments as usize * 64].into_boxed_slice(),
            current_array: Box::<[u64]>::with_zeroed_64bit_segments(level_size_segments as usize)
        }
    }
}

impl CollisionSolver for AcceptEqualsSolver {
    #[inline(always)] fn is_under_collision(&self, index: usize) -> bool {
        self.collided.get_bit(index)
    }

    fn process_fragment(&mut self, index: usize, fragment: u8, _bits_per_fragment: u8) {
        if !self.current_array.get_bit(index) { // empty:
            self.current_array.set_bit(index);
            self.fragments[index] = fragment;
        } else if self.fragments[index] != fragment {
            self.collided.set_bit(index);
            self.current_array.clear_bit(index);
        }
    }

    fn to_collision_array(self) -> Box<[u64]> {
        self.current_array
    }
}

#[derive(Default, Copy, Clone)]
pub struct AcceptEquals;

impl CollisionSolverBuilder for AcceptEquals {
    type CollisionSolver = AcceptEqualsSolver;

    #[inline(always)] fn new(&self, level_size_segments: u32, bits_per_fragment: u8) -> Self::CollisionSolver {
        Self::CollisionSolver::new(level_size_segments, bits_per_fragment)
    }

    #[inline(always)] fn is_lossless(&self) -> bool { true }
}

impl IsLossless for AcceptEquals {}

#[derive(Copy, Clone)]
struct LimitedDifferenceCell {
    /// total difference of added values over minimal value
    total_difference: u16,
    /// minimal value (lowest bit) and number of fragments
    minimum_and_count: u16
}

impl LimitedDifferenceCell {
    /// total_difference=0, minimum=value_mask, count=0
    #[inline(always)] fn new(value_mask: u16) -> Self {
        Self { total_difference: 0, minimum_and_count: value_mask }
    }

    #[inline(always)] fn minimum(&self, value_mask: u16) -> u8 {
        (self.minimum_and_count & value_mask) as u8
    }

    #[inline(always)] fn set_minimum(&mut self, new_value: u8, value_mask: u16) {
        self.minimum_and_count &= !value_mask;
        self.minimum_and_count |= new_value as u16;
    }

    #[inline(always)] fn inc_count(&mut self, bits_per_value: u8) {
        self.minimum_and_count = self.minimum_and_count.checked_add(1 << bits_per_value).unwrap();
    }

    #[inline(always)] fn get_count(&self, bits_per_value: u8) -> u16 {
        self.minimum_and_count >> bits_per_value
    }
}

pub struct AcceptLimitedAverageDifferenceSolver {
    cells: Box<[LimitedDifferenceCell]>,
    bits_per_value: u8,
    value_mask: u16,
    max_difference_per_value: u8
}

impl AcceptLimitedAverageDifferenceSolver {
    pub fn new(level_size_segments: u32, bits_per_value: u8, max_difference_per_value: u8) -> Self {
        let value_mask = n_lowest_bits(bits_per_value as _) as u16;
        Self {
            cells: vec![LimitedDifferenceCell::new(value_mask); level_size_segments as usize*64].into_boxed_slice(),
            bits_per_value,
            value_mask,
            max_difference_per_value
        }
    }
}

impl CollisionSolver for AcceptLimitedAverageDifferenceSolver {
    #[inline(always)] fn is_under_collision(&self, _index: usize) -> bool { false }

    fn process_fragment(&mut self, index: usize, fragment: u8, _bits_per_fragment: u8) {
        let c = &mut self.cells[index];
        let m = c.minimum(self.value_mask);
        if fragment < m {
            c.total_difference = c.total_difference.checked_add(c.get_count(self.bits_per_value) * (m - fragment) as u16).unwrap();
            c.set_minimum(fragment, self.value_mask);
        } else {
            c.total_difference = c.total_difference.checked_add((fragment - m) as u16).unwrap(); // (fragment - m) can be 0 here
        }
        c.inc_count(self.bits_per_value);
    }

    fn to_collision_array(self) -> Box<[u64]> {
        let mut result = Box::<[u64]>::with_zeroed_64bit_segments(self.cells.len() / 64);
        for (index, cell) in self.cells.into_iter().enumerate() {
            let d = cell.get_count(self.bits_per_value);
            if d != 0 && cell.total_difference as u32 <= d as u32 * self.max_difference_per_value as u32 {
                result.set_bit(index);
            }
        }
        result
    }

    fn construct_value_array(number_of_values: usize, bits_per_fragment: u8) -> Box<[u64]> {
        Box::<[u64]>::with_filled_bits(number_of_values*bits_per_fragment as usize)
    }

    fn set_value(output: &mut [u64], index: usize, fragment: u8, bits_per_fragment: u8) {
        let fragment = fragment as u64;
        output.conditionally_change_fragment(| old| if fragment < old { Some(fragment) } else {None}, index, bits_per_fragment);
    }
}

/// Collision solver that uses minimal value in the set and accepts limited average difference
/// between values of the set members and this minimum.
#[derive(Copy, Clone)]
pub struct AcceptLimitedAverageDifference {
    /// Maximal average difference accepted.
    max_difference_per_value: u8
}

impl AcceptLimitedAverageDifference {
    pub fn new(max_difference_per_value: u8) -> Self {
        Self { max_difference_per_value }
    }
}

impl CollisionSolverBuilder for AcceptLimitedAverageDifference {
    type CollisionSolver = AcceptLimitedAverageDifferenceSolver;

    #[inline(always)] fn new(&self, level_size_segments: u32, bits_per_fragment: u8) -> Self::CollisionSolver {
        Self::CollisionSolver::new(level_size_segments, bits_per_fragment, self.max_difference_per_value)
    }

    #[inline(always)] fn is_lossless(&self) -> bool { self.max_difference_per_value == 0 }
}


pub struct CountPositiveCollisions {
    count_and_fragments: Box<[u16]>
}

impl CountPositiveCollisions {
    pub fn new(level_size: usize) -> Self {
        CountPositiveCollisions {
            count_and_fragments: vec![0; level_size].into_boxed_slice()
        }
    }

    pub fn consider(count_and_fragment: &mut u16, fragment: u16, bits_per_fragment: u8) {
        if *count_and_fragment == 0 {  // empty?
            *count_and_fragment = (1u16 << bits_per_fragment) | fragment;
        } else if *count_and_fragment & ((1u16 << bits_per_fragment) - 1) == fragment {   // the same fragment again
            if let Some(v) = count_and_fragment.checked_add(1 << bits_per_fragment) {
                *count_and_fragment = v;
            }
        } else {    // collision:
            *count_and_fragment = u16::MAX;
        }
    }

    /// Returns number of positive collision in given `entry`.
    #[inline] pub fn positive_collisions_in_entry(entry: u16, bits_per_fragment: u8) -> u16 {
        if entry == u16::MAX {  // collision
            0
        } else {
            entry >> bits_per_fragment
        }
    }

    /// Returns number of positive collision at given `index`.
    #[inline] pub fn count(&self, index: usize, bits_per_fragment: u8) -> u16 {
        Self::positive_collisions_in_entry(self.count_and_fragments[index], bits_per_fragment)
    }

    pub fn len(&self) -> usize { self.count_and_fragments.len() }

    /// Counts total number of positive collision in each group (chunk) of successive `values_per_group` indices.
    pub fn positive_collisions_of_groups(&self, values_per_group: u8, bits_per_fragment: u8) -> Box<[u8]> {
        self.count_and_fragments
            .chunks(values_per_group as usize)
            .map(|v|
                v.iter()
                    .map(|e| Self::positive_collisions_in_entry(*e, bits_per_fragment))
                    .fold(0u8, |sum, x| sum.saturating_add(x.min(u8::MAX as _) as u8))
            ).collect()
    }
}

impl CollisionSolver for CountPositiveCollisions {
    fn is_under_collision(&self, index: usize) -> bool {
        self.count_and_fragments[index] == u16::MAX
    }

    fn process_fragment(&mut self, index: usize, fragment: u8, bits_per_fragment: u8) {
        Self::consider(&mut self.count_and_fragments[index], fragment as u16, bits_per_fragment);
    }

    fn to_collision_array(self) -> Box<[u64]> {
        todo!()
    }
}