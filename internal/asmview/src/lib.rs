use ph::phast::{Function, Function2, GenericCore, ShiftWrappedCore, SeedCore};
use ph::seeds::Bits8;

#[no_mangle]
pub extern "C" fn phast_get_f2_shift_only_wrapped_1(f: &Function2<GenericCore, Bits8, ShiftWrappedCore>, key: u64) -> usize {
    f.get(&key)
}

#[no_mangle]
pub extern "C" fn phast_get_f2_seed_only(f: &Function2<GenericCore, Bits8, SeedCore>, key: u64) -> usize {
    f.get(&key)
}

#[no_mangle]
pub extern "C" fn phast_get_f_seed_only(f: &Function<GenericCore, Bits8, SeedCore>, key: u64) -> usize {
    f.get(&key)
}
