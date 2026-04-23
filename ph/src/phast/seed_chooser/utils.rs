
/// Wrapper over `f64` with compare operator using `total_cmp`.
#[derive(Default, Clone, Copy)]
#[repr(transparent)]
pub struct ComparableF64(pub f64);

impl PartialEq for ComparableF64 {
    #[inline(always)] fn eq(&self, other: &Self) -> bool { self.cmp(other).is_eq() }
}

impl PartialOrd for ComparableF64 {
    #[inline(always)] fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> { Some(self.cmp(other)) }
}

impl Eq for ComparableF64 {}

impl Ord for ComparableF64 {
    #[inline(always)] fn cmp(&self, other: &Self) -> std::cmp::Ordering { self.0.total_cmp(&other.0) }
}


/// A type designed for comparing the products (of fixed length) of positive floating-point numbers,
/// which is much more resistant to overflow and underflow than `f64`.
#[derive(PartialEq, Eq, PartialOrd, Ord)]
pub struct ProdCmp {
    sum_of_exponents: u64,   // sum of exponents + 1023 * number of multiplied numbers
    mantissa: ComparableF64, // mantissa of product, normalized to the range [1, 2)
}

impl ProdCmp {
    pub const MAX: Self = Self { sum_of_exponents: u64::MAX, mantissa: ComparableF64(f64::MAX) };
}

impl Default for ProdCmp {
    #[inline] fn default() -> Self { Self { sum_of_exponents: 0, mantissa: ComparableF64(1.0) } }
}

impl std::ops::MulAssign<f64> for ProdCmp {
    fn mul_assign(&mut self, rhs: f64) {
        self.mantissa.0 *= rhs;
        let bits = self.mantissa.0.to_bits();
        self.mantissa.0 = f64::from_bits((bits & ((1u64 << 52) - 1)) | (1023u64 << 52)); // set exponent to 1023 = normalization to [1, 2)
        self.sum_of_exponents += (bits >> 52) & 0x7ff;  // += exponent + 1023
        //self.sum_of_exponents += bits >> 52;    // zakladamy dodatniosc liczby, bit znaku = 0

        /*let bits = rhs.to_bits();
        self.sum_of_exponents += (bits >> 52) & 0x7ff;  // += exponent + 1023
        self.mantissa.0 *= f64::from_bits((bits & ((1u64 << 52) - 1)) | (1023u64 << 52));   // *= float in range [1, 2)
        if self.mantissa.0 >= 2.0 { // normalization of mantissa to [1,2)
            self.mantissa.0 *= 0.5;
            self.sum_of_exponents += 1;
        }*/
    }
}

/// Returns approximation of lower bound of space (in bits/key)
/// needed to represent minimal `k`-perfect function.
pub fn space_lower_bound(k: u16) -> f64 {
    match k {
        0|1 => 1.4426950408889634,  // TODO? 0 should panic
        2 => 0.9426950408889634,
        3 => 0.7193867070748593,
        _ => {
            const LOG2PI: f64 = 2.651496129472319;
            let k = k as f64;
            //let k2 = 2.0 * k;
            //log2(pi*k2)/k2 + 0.12/(k*k)
            0.5 * (LOG2PI + k.log2()) / k + 0.12/(k*k)
        }
    }
}