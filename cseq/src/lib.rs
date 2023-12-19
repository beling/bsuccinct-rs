#![doc = include_str!("../README.md")]

mod wavelet_matrix;
pub use wavelet_matrix::WaveletMatrix;

mod elias_fano;
pub use elias_fano::{EliasFano, EliasFanoBuilder};