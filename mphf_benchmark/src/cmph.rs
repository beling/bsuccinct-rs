use std::fs::File;
use std::io::Write;
use std::hash::Hash;
use std::mem::size_of;
use cmph_sys::{cmph_io_struct_vector_adapter, cmph_config_new, cmph_config_set_algo,
               CMPH_ALGO_CMPH_CHD, cmph_config_set_graphsize, cmph_config_set_b, cmph_uint32,
               cmph_new, cmph_config_destroy, cmph_io_struct_vector_adapter_destroy,
               cmph_packed_size, cmph_pack, cmph_destroy, cmph_search_packed};
use crate::{Conf, MPHFBuilder, Threads};

struct CHDConf { lambda: u8 }

impl<K: Hash> MPHFBuilder<K> for CHDConf {
    type MPHF = Box<[u8]>;

    const CAN_DETECT_ABSENCE: bool = false;
    const BUILD_THREADS: Threads = Threads::Single;

    fn new(&self, keys: &[K], _use_multiple_threads: bool) -> Self::MPHF {
        unsafe {
            let source = cmph_io_struct_vector_adapter(
                keys.as_ptr() as *mut ::std::os::raw::c_void,         // structs
                size_of::<K>() as u32, // struct_size
                0,           // key_offset
                size_of::<K>() as u32, // key_len
                keys.len() as u32); // nkeys

            let config = cmph_config_new(source);
            //cmph_config_set_algo(config, CMPH_CHD_PH); // CMPH_CHD or CMPH_BDZ
            cmph_config_set_algo(config, CMPH_ALGO_CMPH_CHD); // CMPH_CHD or CMPH_BDZ
            cmph_config_set_graphsize(config, 1.01);
            cmph_config_set_b(config, self.lambda as cmph_uint32);
            let hash = cmph_new(config);
            cmph_config_destroy(config);
            cmph_io_struct_vector_adapter_destroy(source);//was: cmph_io_vector_adapter_destroy(source);
            //to_find_perfect_hash.release();

            //let mut packed_hash = vec![MaybeUninit::<u8>::uninit(); cmph_packed_size(hash) as usize].into_boxed_slice();
            let mut packed_hash = vec![0u8; cmph_packed_size(hash) as usize].into_boxed_slice();
            cmph_pack(hash, packed_hash.as_mut_ptr() as *mut ::std::os::raw::c_void);
            cmph_destroy(hash);

            packed_hash
        }
    }

    #[inline(always)] fn value(mphf: &Self::MPHF, key: &K, _levels: &mut u64) -> Option<u64> {
        Some(unsafe{ cmph_search_packed(
            mphf.as_ptr() as *mut ::std::os::raw::c_void,
            key as *const K as *const i8,
            size_of::<K>() as u32) as u64 })
    }
}

pub fn chd_benchmark<K: Hash>(csv_file: &mut Option<File>, i: &(Vec<K>, Vec<K>), conf: &Conf, lambda: u8) {
    let b = CHDConf{ lambda }.benchmark(i, conf);
    if let Some(ref mut f) = csv_file { writeln!(f, "{} {}", lambda, b.all()).unwrap(); }
    println!(" {}\t{}", lambda, b);
}