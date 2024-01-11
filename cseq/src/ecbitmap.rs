/// L1 block covering 2^20=1048576 L2 blocks, which means 2^32 bits of universe.
struct L1Block {
    /// The number of bit ones preceding this block in the universe.
    rank: u64,
    /// Index of the first bit of this block in the universe.
    begin_index: usize,
}

impl L1Block {
    /// Number of universe bits per L1 block; 2^32.
    const UNIVERSE_BITS: usize = 1<<32;
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

/// Read 7 least significant bits `block`. Increase `accumulative_bit_size`.
fn consume7(block: &mut u64, accumulative_bit_size: &mut u64) -> u8 {
    let result = (*block & 127) as u8;
    *block >>= 7;
    //TODO accumulative_bit_size += size[result]
    return result;
}

fn get7(mut block: u64, index: u8, accumulative_bit_size: &mut u64) -> Option<u8> {
    let v = consume7(&mut block, accumulative_bit_size);    if index == 0 { return Some(v); }
    let v = consume7(&mut block, accumulative_bit_size);    if index == 1 { return Some(v); }
    let v = consume7(&mut block, accumulative_bit_size);    if index == 2 { return Some(v); }
    let v = consume7(&mut block, accumulative_bit_size);    if index == 3 { return Some(v); }
    let v = consume7(&mut block, accumulative_bit_size);    if index == 4 { return Some(v); }
    let v = consume7(&mut block, accumulative_bit_size);    if index == 5 { return Some(v); }
    let v = consume7(&mut block, accumulative_bit_size);    if index == 6 { return Some(v); }
    let v = consume7(&mut block, accumulative_bit_size);    if index == 7 { return Some(v); }
    let v = consume7(&mut block, accumulative_bit_size);    if index == 8 { return Some(v); }
    None
}

impl L2Block {
    /// Number of universe bits per L3 block.
    const L3_UNIVERSE_BITS: usize = 64;

    /// Number of universe bits per L2 block.
    const UNIVERSE_BITS: usize = 64 * Self::L3_UNIVERSE_BITS;

    fn get(index: u8, accumulative_bit_size: &mut u64) -> u8 {

    }
}

/// Enumerative coding/compressed bitmap.
pub struct ECBitMap {
    l1: Box<[L1Block]>,
    l2: Box<[L2Block]>,
    content: Box<[u64]>
}

impl ECBitMap {
    pub fn get(&self, index: usize) -> Option<usize> {
        let begin =
            self.l1.get(index / L1Block::UNIVERSE_BITS)?.begin_index

        let l2 = self.l2.get(index / L2Block::UNIVERSE_BITS)?;
        let begin = l2.begin_index as usize +
            //unsafe {self.l1.get_unchecked(index / L1Block::UNIVERSE_BITS)}.begin_index
            self.l1[index / L1Block::UNIVERSE_BITS].begin_index +

    }
}