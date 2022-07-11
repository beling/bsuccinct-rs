#![macro_use]

/// Writes primitive (integer) to `output` (which implements `std::io::Write`);
/// in little-endian bytes order.
///
/// # Example
///
/// ```
/// use binout::write_int;
///
/// let mut output = Vec::new();
/// write_int!(&mut output, 1u32);
/// assert_eq!(output, vec![1, 0, 0, 0]);
/// ```
#[macro_export]
macro_rules! write_int {
    ($output:expr, $what:expr) => {
     ::std::io::Write::write_all($output, &$what.to_le_bytes())
    }
}

/// Reads primitive (integer) from `input` (which implements `std::io::Read`);
/// in little-endian bytes order, returning `std::io::Result`.
///
/// # Example
///
/// ```
/// use binout::read_int;
///
/// let input = [1u8, 0u8, 0u8, 0u8];
/// assert_eq!(read_int!(&mut &input[..], u32).unwrap(), 1u32);
/// ```
#[macro_export]
macro_rules! read_int {
    ($input:expr, $what:ty) => {{
        let mut buff = [0u8; ::std::mem::size_of::<$what>()];
        let result = ::std::io::Read::read_exact($input, &mut buff);
        result.map(|()| <$what>::from_le_bytes(buff))
    }}
}

/// Returns byte whose 7 least significant bits are copied from `v` and the most significant bit is `1`.
#[inline(always)] fn m(v: u32) -> u8 {
    v as u8 | (1 << 7)
}

/// Writes `val` into `output` in *VByte* format.
pub fn vbyte_write<W: std::io::Write + ?Sized>(output: &mut W, val: u32) -> std::io::Result<()> {
    if val < (1 << 7) {
        output.write_all(&[val as u8])
    } else if val < (1 << 14) {
        output.write_all(&[m(val), (val >> 7) as u8])
    } else if val < (1 << 21) {
        output.write_all(&[m(val), m(val >> 7), (val >> 14) as u8])
    } else if val < (1 << 28) {
        output.write_all(&[m(val), m(val >> 7), m(val >> 14), (val >> 21) as u8])
    } else {
        output.write_all(&[m(val), m(val >> 7), m(val >> 14), m(val >> 21), (val >> 28) as u8])
    }
}

/// Returns number of bytes occupied by `val` in *VByte* format. Result is in the range *[1, 5]*.
///
/// # Example
///
/// ```
/// use binout::vbyte_len;
///
/// assert_eq!(vbyte_len(0), 1);
/// assert_eq!(vbyte_len(127), 1);
/// assert_eq!(vbyte_len(128), 2);
/// ```
pub fn vbyte_len(val: u32) -> u8 {
    if val < (1 << 7) { 1 }
    else if val < (1 << 14) { 2 }
    else if val < (1 << 21) { 3 }
    else if val < (1 << 28) { 4 } else { 5 }
}

/// Returns the value read from `input` and decoded from *VByte* format.
pub fn vbyte_read<R: std::io::Read + ?Sized>(input: &mut R) -> std::io::Result<u32> {
    let mut read: u8 = 0;
    let mut result = 0;
    for shift in [0, 7, 14, 21, 28] {
        input.read_exact(std::slice::from_mut(&mut read))?;
        result |= ((read & 0x7F) as u32) << shift;
        if read < 128 { return Ok(result) }
    }
    Err(std::io::ErrorKind::InvalidData.into())   // too many bytes greater than 128
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_vbyte(value: u32) {
        let mut buff = Vec::new();
        assert!(vbyte_write(&mut buff, value).is_ok());
        assert_eq!(buff.len(), vbyte_len(value) as usize);
        assert_eq!(vbyte_read(&mut &buff[..]).unwrap(), value)
    }

    #[test] fn vbyte() {
        test_vbyte(0);
        test_vbyte(127);
        test_vbyte(128);
        test_vbyte(256);
        test_vbyte(2256);
        test_vbyte(32256);
        test_vbyte(8912310);
        test_vbyte(2_000_000_000);
        test_vbyte(4_000_000_000);
        test_vbyte(u32::MAX);
    }
}