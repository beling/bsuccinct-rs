/// L1 block covering 2^20=1048576 L2 blocks, which means 2^32 bits of universe.
struct L1Block {
    /// The number of bit ones preceding this block in the universe.
    rank: u64,
    /// Index of the first bit of this block in the universe.
    begin_index: usize,
}

impl L1Block {
    /// Number of universe bits per L1 block; 2^32.
    const COVERED_UNIVERSE_BITS: usize = 1<<32;
}

/// L2 block covering 64 L3 blocks, which means 64*64=4096 bits of the universe.
/// 
/// Each L3 block covers 64 bits of the universe and uses 7 bits to store the number of one bits it contains.
struct L2Block {
    /// Number of bit ones preceding this block in the enclosing L1 block.
    rank: u32,
    /// Relative (to the beginning of the enclosing L1 block) index of the first universe bit of this block
    begin_index: u32,
    /// 64 sizes of consecutive L3 blocks contained in this block, each stored on 7 bits
    l3_sizes: [u64; 7],
}

/// Returns 7 the least significant bits of `block`.
#[inline(always)] fn lo7(blocks: u64) -> u8 { (blocks & 127) as u8 }

/// Reads 7 bit size of L3 blocks and removes it from `block`.
/// Increase `l3_begin` by the size of the L3 block read.
fn move7(blocks: &mut u64, l3_begin: &mut usize) {
    let result = lo7(*blocks);
    *blocks >>= 7;
    //TODO bit_index += size[result]
}

fn get7(mut blocks: u64, index: &mut usize, l3_begin: &mut usize) -> Option<u8> {
    if *index < 1 * 64 { return Some(lo7(blocks)); } else { move7(&mut blocks, l3_begin) }
    if *index < 2 * 64 { return Some(lo7(blocks)); } else { move7(&mut blocks, l3_begin) }
    if *index < 3 * 64 { return Some(lo7(blocks)); } else { move7(&mut blocks, l3_begin) }
    if *index < 4 * 64 { return Some(lo7(blocks)); } else { move7(&mut blocks, l3_begin) }
    if *index < 5 * 64 { return Some(lo7(blocks)); } else { move7(&mut blocks, l3_begin) }
    if *index < 6 * 64 { return Some(lo7(blocks)); } else { move7(&mut blocks, l3_begin) }
    if *index < 7 * 64 { return Some(lo7(blocks)); } else { move7(&mut blocks, l3_begin) }
    if *index < 8 * 64 { return Some(lo7(blocks)); } else { move7(&mut blocks, l3_begin) }
    if *index < 9 * 64 { return Some(lo7(blocks)); } else { move7(&mut blocks, l3_begin) }
    *index -= 9 * 64;
    None
}

impl L2Block {
    /// Number of universe bits per L3 block.
    const BITS_PER_L3: usize = 64;

    /// Number of L3 block covered by one L2 block.
    const COVERED_L3_BLOCKS: usize = 64;

    /// Number of universe bits per L2 block.
    const COVERED_UNIVERSE_BITS: usize = Self::COVERED_L3_BLOCKS * Self::BITS_PER_L3;

    #[inline(always)] fn get_sizes(&self, index: usize) -> u64 {
        self.l3_sizes[index] << index | self.l3_sizes[index-1] >> (64-index)
    }

    /// Gets size of L3 block with given bit `index` within `self`
    /// and decreases `index` to point bit in decoded L3 block.
    /// Increases `l3_begin` to show the first bit of encoded L3 block in the `content`.
    fn get_size(&self, index: &mut usize, l3_begin: &mut usize) -> u8 {
        if let Some(s) = get7(self.l3_sizes[0], index, l3_begin) { return s; }
        if let Some(s) = get7(self.get_sizes(1), index, l3_begin) { return s; }
        if let Some(s) = get7(self.get_sizes(2), index, l3_begin) { return s; }
        if let Some(s) = get7(self.get_sizes(3), index, l3_begin) { return s; }
        if let Some(s) = get7(self.get_sizes(4), index, l3_begin) { return s; }
        if let Some(s) = get7(self.get_sizes(5), index, l3_begin) { return s; }
        if let Some(s) = get7(self.get_sizes(6), index, l3_begin) { return s; }
        debug_assert!(*index < 64);
        (self.l3_sizes[6] >> (64-7)) as u8
    }
}

/// Enumerative coding/compressed bitmap.
pub struct ECBitMap {
    l1: Box<[L1Block]>,
    l2: Box<[L2Block]>,
    content: Box<[u64]>
}

impl ECBitMap {
    pub fn get(&self, mut index: usize) -> Option<bool> {
        let l2 = self.l2.get(index / L2Block::COVERED_UNIVERSE_BITS)?;
        let mut bit_index = l2.begin_index as usize +
            //unsafe {self.l1.get_unchecked(index / L1Block::COVERED_UNIVERSE_BITS)}.begin_index    // safe as corresponding l2 block exists
            self.l1[index / L1Block::COVERED_UNIVERSE_BITS].begin_index;
        index %= L2Block::COVERED_UNIVERSE_BITS;
        let size = l2.get_size(index, &mut bit_index);

    }
}