#![doc = include_str!("../README.md")]

use std::sync::atomic::{AtomicBool, AtomicI8, AtomicI16, AtomicI32, AtomicI64, AtomicIsize,
    AtomicU8, AtomicU16, AtomicU32, AtomicU64, AtomicUsize};

/// Provides methods to get dynamic and total size of the variable.
pub trait GetSize {
    /// Returns approximate number of bytes occupied by dynamic (heap) part of `self`.
    /// Same as `self.size_bytes() - std::mem::size_of_val(self)`.
    #[inline] fn size_bytes_dyn(&self) -> usize { self.size_bytes_content_dyn() }

    /// Returns approximate number of bytes occupied by dynamic (heap) part of `self` content.
    /// It usually equals to `size_bytes_dyn()`.
    /// However, sometimes it is smaller by the amount of memory reserved but not yet used
    /// (e.g., `size_bytes_content_dyn()` only takes into account the length of the vector and not its capacity).
    #[inline] fn size_bytes_content_dyn(&self) -> usize { 0 }

    /// Returns approximate, total (including heap memory) number of bytes occupied by `self`.
    #[inline] fn size_bytes(&self) -> usize {
        std::mem::size_of_val(self) + self.size_bytes_dyn()
    }

    /// `true` if and only if the variables of this type can use dynamic (heap) memory.
    const USES_DYN_MEM: bool = false;
}

macro_rules! impl_nodyn_getsize_for {
    ($x:ty) => (impl self::GetSize for $x {});
    // `$x` followed by at least one `$y,`
    ($x:ty, $($y:ty),+) => (
        impl self::GetSize for $x {}
        impl_nodyn_getsize_for!($($y),+);
    )
}

impl_nodyn_getsize_for!(u8, u16, u32, u64, u128, usize,
    AtomicU8, AtomicU16, AtomicU32, AtomicU64, AtomicUsize,
    bool, AtomicBool,
    i8, i16, i32, i64, i128, isize,
    AtomicI8, AtomicI16, AtomicI32, AtomicI64, AtomicIsize,
    f32, f64, char, ());

//impl<T: GetSize> GetSize for [T] {    // this works also with slices, but is this sound?
impl<T: GetSize, const N: usize> GetSize for [T; N] {
    fn size_bytes_dyn(&self) -> usize {
        if T::USES_DYN_MEM {
            self.iter().map(self::GetSize::size_bytes_dyn).sum()
        } else {
            0
        }
    }
    fn size_bytes_content_dyn(&self) -> usize {
        if T::USES_DYN_MEM {
            self.iter().map(self::GetSize::size_bytes_content_dyn).sum()
        } else {
            0
        }
    }
    const USES_DYN_MEM: bool = T::USES_DYN_MEM;
}

macro_rules! impl_getsize_methods_for_pointer {
    () => (
        fn size_bytes_content_dyn(&self) -> ::std::primitive::usize {
            ::std::ops::Deref::deref(self).size_bytes()
        }
        const USES_DYN_MEM: bool = true;
    );
}

impl <T: GetSize> GetSize for Box<T> {
    impl_getsize_methods_for_pointer!();
}

impl <T: GetSize> GetSize for ::std::rc::Rc<T> {
    fn size_bytes_content_dyn(&self) -> ::std::primitive::usize {
        // round((size of T + size of strong and weak reference counters) / number of strong references)
        let c = ::std::rc::Rc::strong_count(self);
        (::std::ops::Deref::deref(self).size_bytes() + 2*::std::mem::size_of::<usize>() + c/2) / c
    }
    const USES_DYN_MEM: bool = true;
}

macro_rules! impl_getsize_methods_for_dyn_arr {
    ($T:ty) => (
        fn size_bytes_content_dyn(&self) -> ::std::primitive::usize {
            if <$T>::USES_DYN_MEM {
                self.iter().map(self::GetSize::size_bytes).sum()
            } else {
                ::std::mem::size_of::<$T>() * self.len()
            }
        }
        const USES_DYN_MEM: bool = true;
    );
}

impl<T: GetSize> GetSize for Box<[T]> {
    impl_getsize_methods_for_dyn_arr!(T);
}

impl<T: GetSize> GetSize for Vec<T> {
    fn size_bytes_dyn(&self) -> usize {
        let c = ::std::mem::size_of::<T>() * self.capacity();
        if T::USES_DYN_MEM {
            c + self.iter().map(GetSize::size_bytes_dyn).sum::<usize>()
        } else {
            c
        }
    }
    fn size_bytes_content_dyn(&self) -> usize {
        let c = ::std::mem::size_of::<T>() * self.len();
        if T::USES_DYN_MEM {
            c + self.iter().map(GetSize::size_bytes_content_dyn).sum::<usize>()
        } else {
            c
        }
    }
    const USES_DYN_MEM: bool = true;
}

macro_rules! impl_getsize_for_tuple {
    ($( $T:ident ),+) => {
        impl<$( $T: self::GetSize ),+> self::GetSize for ($( $T, )+) {
            #[allow(non_snake_case)]
            fn size_bytes_dyn(&self) -> ::std::primitive::usize {
                let &($( ref $T, )+) = self;
                0 $( + $T.size_bytes_dyn() )+
            }
            #[allow(non_snake_case)]
            fn size_bytes_content_dyn(&self) -> ::std::primitive::usize {
                let &($( ref $T, )+) = self;
                0 $( + $T.size_bytes_content_dyn() )+
            }
            const USES_DYN_MEM: bool = $( $T::USES_DYN_MEM )|*;
        }
    }
}

impl_getsize_for_tuple!(A);
impl_getsize_for_tuple!(A, B);
impl_getsize_for_tuple!(A, B, C);
impl_getsize_for_tuple!(A, B, C, D);
impl_getsize_for_tuple!(A, B, C, D, E);
impl_getsize_for_tuple!(A, B, C, D, E, F);
impl_getsize_for_tuple!(A, B, C, D, E, F, G);
impl_getsize_for_tuple!(A, B, C, D, E, F, G, H);
impl_getsize_for_tuple!(A, B, C, D, E, F, G, H, I);
impl_getsize_for_tuple!(A, B, C, D, E, F, G, H, I, J);



#[cfg(test)]
mod tests {
    use super::*;

    fn test_primitive<T: GetSize>(v: T) {
        assert_eq!(v.size_bytes_dyn(), 0);
        assert_eq!(v.size_bytes_content_dyn(), 0);
        assert_eq!(v.size_bytes(), std::mem::size_of_val(&v));
        assert!(!T::USES_DYN_MEM);
    }

    #[test]
    fn test_primitives() {
        test_primitive(1u32);
        test_primitive(1.0f32);
    }

    #[test]
    fn test_array() {
        assert_eq!([1u32, 2u32, 3u32].size_bytes(), 3*4);
        assert_eq!([[1u32, 2u32], [3u32, 4u32]].size_bytes(), 4*4);
        assert_eq!([vec![1u32, 2u32], vec![3u32, 4u32]].size_bytes_content_dyn(), 4*4);
    }

    #[test]
    fn test_vec() {
        assert_eq!(vec![1u32, 2u32, 3u32].size_bytes_content_dyn(), 3*4);
        assert_eq!(vec![[1u32, 2u32], [3u32, 4u32]].size_bytes_content_dyn(), 4*4);
        let v = vec![1u32, 2u32];
        assert_eq!(vec![v.clone(), v.clone()].size_bytes_dyn(), 2*v.size_bytes());
        assert_eq!(Vec::<u32>::with_capacity(3).size_bytes_dyn(), 3*4);
        assert_eq!(Vec::<u32>::with_capacity(3).size_bytes_content_dyn(), 0);
    }

    #[test]
    fn test_boxed_slice() {
        let bs = vec![1u32, 2u32, 3u32].into_boxed_slice();
        assert_eq!(bs.size_bytes_dyn(), 3*4);
        assert_eq!(bs.size_bytes(), 3*4 + std::mem::size_of_val(&bs));
    }

    #[test]
    fn test_tuple() {
        assert_eq!((1u32, 2u32).size_bytes_dyn(), 0);
        assert_eq!((1u32, vec![3u32, 4u32]).size_bytes_dyn(), 2*4);
        assert_eq!((vec![1u32, 2u32], vec![3u32, 4u32]).size_bytes_dyn(), 4*4);
    }

    #[test]
    fn test_box() {
        assert_eq!(Box::new(1u32).size_bytes_dyn(), 4);
        assert_eq!(Box::new([1u32, 2u32]).size_bytes_dyn(), 2*4);
    }
}