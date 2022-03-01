use super::{ceiling_div, n_lowest_bits};

/// The trait that is implemented for the array of `u64` and extends it with methods for
/// accessing and modifying single bits or arbitrary fragments consisted of few (up to 63) bits.
pub trait BitAccess {
    /// Gets bit with given index `bit_nr`.
    fn get_bit(&self, bit_nr: usize) -> bool;

    /// Sets bit with given index `bit_nr` to `1`.
    fn set_bit(&mut self, bit_nr: usize);

    /// Sets bit with given index `bit_nr` to `0`.
    fn clear_bit(&mut self, bit_nr: usize);

    /// Gets bits `[begin, begin+len)`.
    fn get_bits(&self, begin: usize, len: u8) -> u64;

    /// Sets bits `[begin, begin+len)` to the content of `v`.
    fn set_bits(&mut self, begin: usize, v: u64, len: u8);

    /// Gets `v_size` bits with indices in range [`index*v_size`, `index*v_size+v_size`).
    #[inline(always)] fn get_fragment(&self, index: usize, v_size: u8) -> u64 {
        self.get_bits(index * v_size as usize, v_size)
    }

    /// Inits `v_size` bits with indices in range [`index*v_size`, `index*v_size+v_size`) to `v`.
    /// Before init, the bits are assumed to be cleared or already set to `v`.
    #[inline(always)] fn init_fragment(&mut self, index: usize, v: u64, v_size: u8) {
        self.set_fragment(index, v, v_size)
    }

    /// Sets `v_size` bits with indices in range [`index*v_size`, `index*v_size+v_size`) to `v`.
    #[inline(always)] fn set_fragment(&mut self, index: usize, v: u64, v_size: u8) {
        self.set_bits(index * v_size as usize, v, v_size);
    }

    /// Swaps ranges of bits: [`index1*v_size`, `index1*v_size+v_size`) with [`index2*v_size`, `index2*v_size+v_size`).
    fn swap_fragments(&mut self, index1: usize, index2: usize, v_size: u8) {
        // TODO faster implementation
        let v1 = self.get_fragment(index1, v_size);
        self.set_fragment(index1, self.get_fragment(index2, v_size), v_size);
        self.set_fragment(index2, v1, v_size);
    }

    /// Conditionally (if `new_value` does not return `None`) changes
    /// the value `old` stored at bits `[begin, begin+v_size)`
    /// to the one returned by `new_value` (whose argument is `old`).
    /// Returns `old` (the value before change).
    fn conditionally_change_bits<NewValue>(&mut self, new_value: NewValue, begin: usize, v_size: u8) -> u64
        where NewValue: FnOnce(u64) -> Option<u64>
    {
        let old = self.get_bits(begin, v_size);
        if let Some(new) = new_value(old) { self.set_bits(begin, new, v_size); }
        old
    }

    /// Conditionally (if `new_value` does not return `None`) changes
    /// the value `old` stored at bits [`index*v_size`, `index*v_size+v_size`)
    /// to the one returned by `new_value` (whose argument is `old`).
    /// Returns `old` (the value before change).
    #[inline(always)] fn conditionally_change_fragment<NewValue>(&mut self, new_value: NewValue, index: usize, v_size: u8) -> u64
        where NewValue: FnOnce(u64) -> Option<u64>
    {
        self.conditionally_change_bits(new_value, index * v_size as usize, v_size)
    }
}

/// The trait that is implemented for `Box<[u64]>` and extends it with bit-oriented constructors.
pub trait BitVec where Self: Sized {
    fn with_64bit_segments(segments_value: u64, segments_len: usize) -> Self;

    #[inline(always)] fn with_zeroed_64bit_segments(segments_len: usize) -> Self {
        Self::with_64bit_segments(0, segments_len)
    }

    #[inline(always)] fn with_filled_64bit_segments(segments_len: usize) -> Self {
        Self::with_64bit_segments(u64::MAX, segments_len)
    }

    #[inline(always)] fn with_zeroed_bits(bit_len: usize) -> Self {
        Self::with_zeroed_64bit_segments(ceiling_div(bit_len, 64))
    }

    #[inline(always)] fn with_filled_bits(bit_len: usize) -> Self {
        Self::with_filled_64bit_segments(ceiling_div(bit_len, 64))
    }

    //fn with_bit_fragments<V: Into<u64>, I: IntoIterator<Item=V>>(items: I, fragment_count: usize, bits_per_fragment: u8) -> Box<[u64]>
}

impl BitVec for Box<[u64]> {
    #[inline(always)] fn with_64bit_segments(segments_value: u64, segments_len: usize) -> Self {
        vec![segments_value; segments_len].into_boxed_slice()
    }
}

/*#[inline(always)] pub fn bitvec_len_for_bits(bits_len: usize) -> usize { ceiling_div(bits_len, 64) }

#[inline(always)] pub fn bitvec_with_segments_len_and_value(segments_len: usize, segments_value: u64) -> Box<[u64]> {
    vec![segments_value; segments_len].into_boxed_slice()
}
#[inline(always)] pub fn bitvec_with_segments_len_zeroed(segments_len: usize) -> Box<[u64]> {
    bitvec_with_segments_len_and_value(segments_len, 0)
}
#[inline(always)] pub fn bitvec_with_segments_len_filled(segments_len: usize) -> Box<[u64]> {
    bitvec_with_segments_len_and_value(segments_len, u64::MAX)
}
#[inline(always)] pub fn bitvec_with_bits_len_zeroed(bits_len: usize) -> Box<[u64]> {
    bitvec_with_segments_len_zeroed(bitvec_len_for_bits(bits_len))
}
#[inline(always)] pub fn bitvec_with_bits_len_filled(bits_len: usize) -> Box<[u64]> {
    bitvec_with_segments_len_filled(bitvec_len_for_bits(bits_len))
}

pub fn bitvec_with_items<V: Into<u64>, I: IntoIterator<Item=V>>(items: I, fragment_count: usize, bits_per_fragment: u8) -> Box<[u64]> {
    let mut result = bitvec_with_bits_len_zeroed(fragment_count * bits_per_fragment as usize);
    for (i, v) in items.into_iter().enumerate() {
        result.init_fragment(i, v.into(), bits_per_fragment);
    }
    result
}*/

impl BitAccess for [u64] {
    #[inline(always)] fn get_bit(&self, bit_nr: usize) -> bool {
        self[bit_nr / 64] & (1u64 << (bit_nr % 64) as u64) != 0
    }

    #[inline(always)] fn set_bit(&mut self, bit_nr: usize) {
        self[bit_nr / 64] |= 1u64 << (bit_nr % 64) as u64;
    }

    #[inline(always)] fn clear_bit(&mut self, bit_nr: usize) {
        self[bit_nr / 64] &= !((1u64) << (bit_nr % 64) as u64);
    }

    fn get_bits(&self, begin: usize, len: u8) -> u64 {
        let index_segment = begin / 64;
        //data += index_bit / 64;
        let offset = (begin % 64) as u8;
        let w1 = self[index_segment]>>offset;
        let end_bit = offset+len;
        if end_bit > 64 {
            w1 |
                ((self[index_segment+1] & n_lowest_bits(end_bit&0x3F)) // or (end_bit-64) or (end_bit%64)
                    << (64-offset))  // move bits to the left
        } else {
            w1 & n_lowest_bits(len)
        }
    }

    fn set_bits(&mut self, begin: usize, v: u64, len: u8) {
        let index_segment = begin / 64;
        let offset = (begin % 64) as u64;   // the lowest bit to set in index_segment
        let v_mask = n_lowest_bits(len);
        if offset + len as u64 > 64 {
            let shift = 64-offset;
            self[index_segment+1] &= !(v_mask >> shift);
            self[index_segment+1] |= v >> shift;
        }
        self[index_segment] &= !(v_mask << offset);
        self[index_segment] |= v << offset;
    }

    fn init_fragment(&mut self, index: usize, v: u64, v_size: u8) {
        debug_assert!({let f = self.get_fragment(index, v_size); f == 0 || f == v});
        let index_bit = index * v_size as usize;
        let index_segment = index_bit / 64;
        let offset = (index_bit % 64) as u64;   // the lowest bit to init in index_segment
        if offset + v_size as u64 > 64 {
            self[index_segment+1] |= v >> (64-offset);
        }
        self[index_segment] |= v << offset;
    }

    fn conditionally_change_bits<NewValue>(&mut self, new_value: NewValue, begin: usize, v_size: u8) -> u64
        where NewValue: FnOnce(u64) -> Option<u64>
    {
        let index_segment = begin / 64;
        //data += index_bit / 64;
        let offset = (begin % 64) as u64;
        let w1 = self[index_segment]>>offset;
        let end_bit = offset+v_size as u64;
        let v_mask = n_lowest_bits(v_size);
        let r = if end_bit > 64 {
            let shift = 64-offset;
            w1 | ((self[index_segment+1] & (v_mask >> shift)) << shift)
        } else {
            w1 & v_mask
        };
        if let Some(v) = new_value(r) {
            if end_bit > 64 {
                let shift = 64 - offset;
                self[index_segment + 1] &= !(v_mask >> shift);
                self[index_segment + 1] |= v >> shift;
            }
            self[index_segment] &= !(v_mask << offset);
            self[index_segment] |= v << offset;
        }
        r
    }
}

#[cfg(test)]
mod tests {
    // Note this useful idiom: importing names from outer (for mod tests) scope.
    use super::*;

    #[test]
    fn fragments_init_set_swap() {
        let mut b = Box::<[u64]>::with_zeroed_64bit_segments(2);
        assert_eq!(b.as_ref(), [0u64, 0u64]);
        b.init_fragment(1, 0b101, 3);
        assert_eq!(b.get_fragment(1, 3), 0b101);
        assert_eq!(b.get_fragment(0, 3), 0);
        assert_eq!(b.get_fragment(2, 3), 0);
        b.init_fragment(2, 0b10110_10110_10110_10110_10110_10110, 30);
        assert_eq!(b.get_fragment(2, 30), 0b10110_10110_10110_10110_10110_10110);
        assert_eq!(b.get_fragment(1, 30), 0);
        assert_eq!(b.get_fragment(3, 30), 0);
        b.set_fragment(2, 0b11010_11010_11111_00000_11111_10110, 30);
        assert_eq!(b.get_fragment(2, 30), 0b11010_11010_11111_00000_11111_10110);
        assert_eq!(b.get_fragment(1, 30), 0);
        assert_eq!(b.get_fragment(3, 30), 0);
        b.swap_fragments(2, 3, 30);
        assert_eq!(b.get_fragment(3, 30), 0b11010_11010_11111_00000_11111_10110);
        assert_eq!(b.get_fragment(2, 30), 0);
        assert_eq!(b.get_fragment(1, 30), 0);
    }

    #[test]
    fn fragments_conditionally_change() {
        let mut b = Box::<[u64]>::with_zeroed_64bit_segments(2);
        let old = b.conditionally_change_fragment(|old| if 0b101>old {Some(0b101)} else {None}, 1, 3);
        assert_eq!(old, 0);
        assert_eq!(b.get_fragment(1, 3), 0b101);
        assert_eq!(b.get_fragment(0, 3), 0);
        assert_eq!(b.get_fragment(2, 3), 0);
        let bits = 0b10110_10110_10110_10110_10110_10110;
        let old = b.conditionally_change_fragment(|old| if old==bits {Some(bits)} else {None}, 2, 30);
        assert_eq!(old, 0);
        assert_eq!(b.get_fragment(2, 30), 0);
        assert_eq!(b.get_fragment(1, 30), 0);
        assert_eq!(b.get_fragment(3, 30), 0);
        let old = b.conditionally_change_fragment(|old| if old!=bits {Some(bits)} else {None}, 2, 30);
        assert_eq!(old, 0);
        assert_eq!(b.get_fragment(2, 30), bits);
        assert_eq!(b.get_fragment(1, 30), 0);
        assert_eq!(b.get_fragment(3, 30), 0);
        let bits2 = 0b1100_11111_00000_10110_00111_11100;
        let old = b.conditionally_change_fragment(|old| if old!=bits2 {Some(bits2)} else {None}, 2, 30);
        assert_eq!(old, bits);
        assert_eq!(b.get_fragment(2, 30), bits2);
        assert_eq!(b.get_fragment(1, 30), 0);
        assert_eq!(b.get_fragment(3, 30), 0);
    }

    #[test]
    fn bits() {
        let mut b = Box::<[u64]>::with_filled_64bit_segments(2);
        assert_eq!(b.as_ref(), [u64::MAX, u64::MAX]);
        assert!(b.get_bit(3));
        assert!(b.get_bit(73));
        b.clear_bit(73);
        assert!(!b.get_bit(73));
        assert!(b.get_bit(72));
        assert!(b.get_bit(74));
        b.set_bit(73);
        assert!(b.get_bit(73));
    }
}