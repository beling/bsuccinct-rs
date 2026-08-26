#![doc = include_str!("../README.md")]

use ph::phast::{Function, Function2, GenericCore, ShiftWrappedCore, SeedOnlyCore};
use ph::seeds::Bits8;

#[no_mangle]
pub extern "C" fn phast_get_f2_shift_wrap_m1(f: &Function2<GenericCore, Bits8, ShiftWrappedCore>, key: u64) -> usize {
    f.get(&key)
}

#[no_mangle]
pub extern "C" fn phast_get_f2_shift_wrap_m3(f: &Function2<GenericCore, Bits8, ShiftWrappedCore<3>>, key: u64) -> usize {
    f.get(&key)
}

#[no_mangle]
pub extern "C" fn phast_get_f2_seed(f: &Function2<GenericCore, Bits8, SeedOnlyCore>, key: u64) -> usize {
    f.get(&key)
}

#[no_mangle]
pub extern "C" fn phast_get_f_seed(f: &Function<GenericCore, Bits8, SeedOnlyCore>, key: u64) -> usize {
    f.get(&key)
}
