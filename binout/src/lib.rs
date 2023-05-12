#![doc = include_str!("../README.md")]

/// Trait implemented by each serializer for the following types:
/// `u8`, `u16`, `u32`, `u64`, `usize` (which, for portability, is always serialized the same as `u64`).
pub trait Serializer<T: Copy>: Copy {

    /// Either size of each value in bytes (if each value occupies constant size) or `None`.
    const CONST_SIZE: Option<usize> = None;

    /// Returns number of bytes which `write(output, val)` will write.
    fn size(val: T) -> usize;

    /// Serialize `val` to the given `output`.
    fn write<W: std::io::Write + ?Sized>(output: &mut W, val: T) -> std::io::Result<()>;
    
    /// Deserialize value from the given `input`.
    fn read<R: std::io::Read + ?Sized>(input: &mut R) -> std::io::Result<T>;

    /// Serialize all `values` into the given `output`.
    fn write_all_values<W, InIter>(output: &mut W, values: InIter) -> std::io::Result<()>
        where W: std::io::Write + ?Sized, InIter: IntoIterator<Item = T>
    {
        values.into_iter().try_for_each(|val| Self::write(output, val))
    }

    /// Serialize all `values` into the given `output`.
    #[inline] fn write_all<'a, W, InIter>(output: &mut W, values: InIter) -> std::io::Result<()>
        where W: std::io::Write + ?Sized, InIter: IntoIterator<Item = &'a T>, T: 'a //, InIter::Item: std::borrow::Borrow<T>
    {
        Self::write_all_values(output, values.into_iter().cloned())
    }

    /// Returns number of bytes occupied by encoded content of the array.
    fn array_content_size(array: &[T]) -> usize {
        if let Some(each_element_size) = Self::CONST_SIZE {
            each_element_size * array.len()
        } else {
            array.iter().map(|val| Self::size(*val)).sum()
        }
    }

    /// Returns number of bytes which `write_array(output, array)` will write.
    #[inline] fn array_size(array: &[T]) -> usize {
        VByte::size(array.len()) + Self::array_content_size(array)
    }

    /// Serialize `array` to the given `input`. Size of the `array` is stored in `VByte` format.
    fn write_array<W: std::io::Write + ?Sized>(output: &mut W, array: &[T]) -> std::io::Result<()> {
        VByte::write(output, array.len())?;
        Self::write_all(output, array)
    }

    /// Deserialize `n` values from the given `input`.
    fn read_n<R: std::io::Read + ?Sized>(input: &mut R, n: usize) -> std::io::Result<Vec<T>> {
        let mut result = Vec::with_capacity(n);
        for _ in 0..n { result.push(Self::read(input)?); }
        Ok(result)
    }

    /// Deserialize array from the given `input`. Size of the array is stored in `VByte` format.
    fn read_array<R: std::io::Read + ?Sized>(input: &mut R) -> std::io::Result<Vec<T>> {
        let n = VByte::read(input)?;
        Self::read_n(input, n)
    }
}

impl<S> Serializer<usize> for S where S: Serializer<u64> {
    const CONST_SIZE: Option<usize> = S::CONST_SIZE;

    #[inline] fn size(val: usize) -> usize { S::size(val as u64) }

    #[inline] fn write<W: std::io::Write + ?Sized>(output: &mut W, val: usize) -> std::io::Result<()> {
        S::write(output, val as u64)
    }
    
    #[inline] fn read<R: std::io::Read + ?Sized>(input: &mut R) -> std::io::Result<usize> {
        S::read(input).map(|v| v as usize)
    }
}

/// Serialize values as-is, in little-endian bytes order.
#[derive(Clone, Copy)]
pub struct AsIs;

// Implement serializer that serialize values as-is, in little-endian bytes order.
macro_rules! impl_le_serializer {
    ($asistype:ty, $inttype:ty) => {
        impl Serializer<$inttype> for $asistype {
            const CONST_SIZE: Option<usize> = Some(::std::mem::size_of::<$inttype>());

            #[inline] fn size(_val: $inttype) -> usize { ::std::mem::size_of::<$inttype>() }

            fn write<W: ::std::io::Write + ?Sized>(output: &mut W, val: $inttype) -> ::std::io::Result<()> {
                ::std::io::Write::write_all(output, &val.to_le_bytes())
            }

            fn read<R: ::std::io::Read + ?Sized>(input: &mut R) -> ::std::io::Result<$inttype> {
                let mut buff = [0u8; ::std::mem::size_of::<$inttype>()];
                let result = ::std::io::Read::read_exact(input, &mut buff);
                result.map(|()| <$inttype>::from_le_bytes(buff))
            }
        }
    };

    ($asistype:ty, $x:ty, $($y:ty),+) => (
        impl_le_serializer!($asistype, $x);
        impl_le_serializer!($asistype, $($y),+);
    )
}

impl_le_serializer!(AsIs, u8, u16, u32, u64);  // , i8, i16, i32, i64, i128, isize

/// Returns byte whose 7 least significant bits are copied from `v` and the most significant bit is `1`.
macro_rules! m { ($v:expr) => { $v as u8 | (1 << 7) } }

/// Returns the value read from `input` and decoded from *VByte* format.
fn vbyte_read<R: std::io::Read + ?Sized>(input: &mut R, max_shift: u8) -> std::io::Result<u64> {
    let mut read: u8 = 0;
    let mut result = 0;
    let mut shift = 0;
    while shift < max_shift {
        input.read_exact(std::slice::from_mut(&mut read))?;
        result |= ((read & 0x7F) as u64) << shift;
        if read < 128 { return Ok(result) }
        shift += 7;
    }
    // last byte is always saved at 8-bits, as-is
    input.read_exact(std::slice::from_mut(&mut read))?;
    Ok(result | ((read as u64) << shift))
}

/// Serializer that uses improved *VByte*/*LEB128* encoding.
/// 
/// The encoding is identical to the classic *VByte*/*LEB128* for `u16` and `u32` values.
/// However:
/// - `u8` value are always stored as is, using 1 byte.
/// - For `u64` values below $2^63$, the encoding is identical to the classic *VByte*/*LEB128*;
///   For larger values, the encoding always stores the most significant byte of value as is, using a total of 9 bytes,
///   whereas a classic VByte could use 10 bytes.
#[derive(Clone, Copy)]
pub struct VByte;

impl_le_serializer!(VByte, u8);

impl Serializer<u16> for VByte {
    fn size(val: u16) -> usize {
        if val < (1 << 7) { 1 } else if val < (1 << 14) { 2 } else { 3 }
    }

    fn write<W: std::io::Write + ?Sized>(output: &mut W, val: u16) -> std::io::Result<()> {
        if val < (1 << 7) {
            output.write_all(&[val as u8])
        } else if val < (1 << 14) {
            output.write_all(&[m!(val), (val >> 7) as u8])
        } else {
            output.write_all(&[m!(val), m!(val >> 7), (val >> 14) as u8])
        } 
    }

    fn read<R: std::io::Read + ?Sized>(input: &mut R) -> std::io::Result<u16> {
        Ok(vbyte_read(input, 2 * 7)?.try_into().map_err(|_| std::io::ErrorKind::InvalidData)?)
    }
}

impl Serializer<u32> for VByte {
    fn size(val: u32) -> usize {
        if val < (1 << 7)  { 1 }
        else if val < (1 << 14) { 2 }
        else if val < (1 << 21) { 3 }
        else if val < (1 << 28) { 4 } else { 5 }
    }

    fn write<W: std::io::Write + ?Sized>(output: &mut W, val: u32) -> std::io::Result<()> {
        if val < (1 << 7) {
            output.write_all(&[val as u8])
        } else if val < (1 << 14) {
            output.write_all(&[m!(val), (val >> 7) as u8])
        } else if val < (1 << 21) {
            output.write_all(&[m!(val), m!(val >> 7), (val >> 14) as u8])
        } else if val < (1 << 28) {
            output.write_all(&[m!(val), m!(val >> 7), m!(val >> 14), (val >> 21) as u8])
        } else {
            output.write_all(&[m!(val), m!(val >> 7), m!(val >> 14), m!(val >> 21), (val >> 28) as u8])
        }
    }

    fn read<R: std::io::Read + ?Sized>(input: &mut R) -> std::io::Result<u32> {
        Ok(vbyte_read(input, 4 * 7)?.try_into().map_err(|_| std::io::ErrorKind::InvalidData)?)
    }
}

impl Serializer<u64> for VByte {
    fn size(mut val: u64) -> usize {
        let ta = if val < (1 << 28) { 0 } else { val >>= 28; 4 };
        ta + if val < (1 << 7)  { 1 }
        else if val < (1 << 14) { 2 }
        else if val < (1 << 21) { 3 }
        else if val < (1 << 28) { 4 } else { 5 }
    }

    fn write<W: std::io::Write + ?Sized>(output: &mut W, mut val: u64) -> std::io::Result<()> {
        if val >= (1 << 28) {
            output.write_all(&[m!(val), m!(val >> 7), m!(val >> 14), m!(val >> 21)])?;
            val >>= 28;
        }
        if val < (1 << 7) {
            output.write_all(&[val as u8])
        } else if val < (1 << 14) {
            output.write_all(&[m!(val), (val >> 7) as u8])
        } else if val < (1 << 21) {
            output.write_all(&[m!(val), m!(val >> 7), (val >> 14) as u8])
        } else if val < (1 << 28) {
            output.write_all(&[m!(val), m!(val >> 7), m!(val >> 14), (val >> 21) as u8])
        } else {
            output.write_all(&[m!(val), m!(val >> 7), m!(val >> 14), m!(val >> 21), (val >> 28) as u8])
        }
    }

    fn read<R: std::io::Read + ?Sized>(input: &mut R) -> std::io::Result<u64> {
        vbyte_read(input, 8 * 7)
    }
}


#[cfg(test)]
mod tests {
    use std::fmt::Debug;

    use super::*;

    fn test_serializer<T: Copy + PartialEq + Debug, S: Serializer<T>>(value: T) {
        let mut buff = Vec::new();
        assert!(S::write(&mut buff, value).is_ok());
        assert_eq!(buff.len(), S::size(value));
        assert_eq!(S::read(&mut &buff[..]).unwrap(), value)
    }

    fn test_serializer_array<T: Copy + PartialEq + Debug, S: Serializer<T>>(values: &[T]) {
        let mut buff = Vec::new();
        assert!(S::write_array(&mut buff, values).is_ok());
        assert_eq!(buff.len(), S::array_size(values));
        assert_eq!(S::read_array(&mut &buff[..]).unwrap(), values)
    }

    fn test_u16<S: Serializer<u16>>() {
        let test = test_serializer::<u16, S>;
        test(0);
        test(127);
        test(128);
        test(256);
        test(2256);
        test(32256);
        test(u16::MAX>>1);
        test(u16::MAX-1);
        test(u16::MAX);
    }

    #[test] fn vbyte_u16() { test_u16::<VByte>() }

    #[test] fn asis_u16() { test_u16::<AsIs>() }

    fn test_u16_array<S: Serializer<u16>>() {
        let test = test_serializer_array::<u16, S>;
        test(&[]);
        test(&[127, 128, 256, 2256, 32256, u16::MAX>>1, u16::MAX-1, u16::MAX]);
    }

    #[test] fn vbyte_u16_array() { test_u16_array::<VByte>() }

    #[test] fn asis_u16_array() { test_u16_array::<AsIs>() }

    #[test] fn vbyte_u32() {
        let test_vbyte = test_serializer::<u32, VByte>;
        test_vbyte(0);
        test_vbyte(127);
        test_vbyte(128);
        test_vbyte(256);
        test_vbyte(2256);
        test_vbyte(32256);
        test_vbyte(8912310);
        test_vbyte(2_000_000_000);
        test_vbyte((1<<28)-1);
        test_vbyte(1<<28);
        test_vbyte((1<<28)+1);
        test_vbyte(u32::MAX>>1);
        test_vbyte(u32::MAX-1);
        test_vbyte(u32::MAX);
    }

    #[test] fn vbyte_u64() {
        let test_vbyte = test_serializer::<u64, VByte>;
        test_vbyte(0);
        test_vbyte(127);
        test_vbyte(128);
        test_vbyte(256);
        test_vbyte(2256);
        test_vbyte(32256);
        test_vbyte(8912310);
        test_vbyte(2_000_000_000);
        test_vbyte(4_000_000_000);
        test_vbyte((1<<28)-1);
        test_vbyte(1<<28);
        test_vbyte((1<<28)+1);
        test_vbyte((1<<35)-1);
        test_vbyte(1<<35);
        test_vbyte((1<<35)+1);
        test_vbyte((1<<56)-1);
        test_vbyte(1<<56);
        test_vbyte((1<<56)+1);
        test_vbyte((1<<63)-1);
        test_vbyte(1<<63);
        test_vbyte((1<<63)+1);
        test_vbyte(u64::MAX>>1);
        test_vbyte(u64::MAX-1);
        test_vbyte(u64::MAX);
    }
}