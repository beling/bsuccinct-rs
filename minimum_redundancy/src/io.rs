#![macro_use]

/// Writes primitive (integer) to `output` (which implements `std::io::Write`);
/// in little-endian bytes order.
///
/// # Example
///
/// ```
/// use minimum_redundancy::write_int;
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
/// in little-endian bytes order, returning `std::io::Read`.
///
/// # Example
///
/// ```
/// use minimum_redundancy::read_int;
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