use std::io::BufRead;

//fn test_data_32x<const N: usize>(how_many: usize) -> (Vec<[u32; N]>, Vec<[u32; N]>) { test_data(how_many, Generate32x::<N>::new(5678)) }
pub struct RawLines<B> {
    buf: B,
    separator: u8,
}

impl<B> RawLines<B> {
    pub fn separated_by_newlines(buf: B) -> Self { Self { buf, separator: b'\n' } }
    pub fn separated_by_zeros(buf: B) -> Self { Self { buf, separator: 0 } }
}

impl<B: BufRead> Iterator for RawLines<B> {
    type Item = std::io::Result<Box<[u8]>>;

    fn next(&mut self) -> Option<Self::Item> {
        let mut buf = Vec::new();
        match self.buf.read_until(self.separator, &mut buf) {
            Ok(0) => None,
            Ok(_n) => {
                if buf.last() == Some(&self.separator) {
                    buf.pop();
                    if self.separator == b'\n' && buf.last() == Some(&b'\r') {
                        buf.pop();
                    }
                }
                //buf.shrink_to_fit();
                Some(Ok(buf.into_boxed_slice()))
            }
            Err(e) => Some(Err(e)),
        }
    }
}

pub fn gen_data<I: Iterator>(keys_num: usize, foreign_keys_num: usize, mut generator: I) -> (Vec<I::Item>, Vec<I::Item>) {
    let mut keys = Vec::with_capacity(keys_num);
    keys.extend(generator.by_ref().take(keys_num));
    let mut foreign = Vec::with_capacity(foreign_keys_num);
    foreign.extend(generator.take(foreign_keys_num));
    (keys, foreign)
    //(generator.by_ref().take(keys_num).collect(), generator.take(foreign_keys_num).collect())
}

/*struct Generate32x<const N: usize>(XorShift32);
impl<const N: usize> Generate32x<N> {
    pub fn new(seed: u32) -> Self { Self(XorShift32(seed)) }
}

impl<const N: usize> Iterator for Generate32x<N> {
    type Item = [u32; N];

    fn next(&mut self) -> Option<Self::Item> {
        /*let mut result: [MaybeUninit<u32>; N] = unsafe {
            MaybeUninit::uninit().assume_init()
        };
        for v in &mut result { v.write(self.0.next().unwrap()); }
        Some(unsafe{std::mem::transmute::<_, [u32; N]>(result)})*/
        let mut result = [0u32; N];
        for v in &mut result { *v = self.0.next().unwrap(); }
        Some(result)
    }
}*/