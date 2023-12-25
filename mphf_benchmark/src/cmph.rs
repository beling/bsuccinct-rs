use std::fs::File;
use std::io::Write;
use std::hash::Hash;
use std::mem::size_of;
use cmph_sys::{cmph_io_struct_vector_adapter, cmph_config_new, cmph_config_set_algo,
               CMPH_ALGO_CMPH_CHD, cmph_config_set_graphsize, cmph_config_set_b, cmph_uint32,
               cmph_new, cmph_config_destroy, cmph_io_struct_vector_adapter_destroy,
               cmph_packed_size, cmph_pack, cmph_destroy, cmph_search_packed, cmph_io_adapter_t};
use crate::{Conf, MPHFBuilder, Threads};

use std::ffi::{c_void, c_char, c_int};

struct CHDConf { lambda: u8 }

pub trait CMPHSource: Sized {
    fn new_cmph_source(keys: &[Self]) -> *mut cmph_io_adapter_t;
    fn del_cmph_source(source: *mut cmph_io_adapter_t);
    fn key_data_and_len(key: &Self) -> (*const c_char, u32);
}

impl<K: Hash + CMPHSource> MPHFBuilder<K> for CHDConf {
    type MPHF = Box<[u8]>;

    const CAN_DETECT_ABSENCE: bool = false;
    const BUILD_THREADS: Threads = Threads::Single;

    fn new(&self, keys: &[K], _use_multiple_threads: bool) -> Self::MPHF {
        unsafe {
            let source = K::new_cmph_source(keys);
            let config = cmph_config_new(source);
            //cmph_config_set_algo(config, CMPH_CHD_PH); // CMPH_CHD or CMPH_BDZ
            cmph_config_set_algo(config, CMPH_ALGO_CMPH_CHD); // CMPH_CHD or CMPH_BDZ
            cmph_config_set_graphsize(config, 1.01);
            cmph_config_set_b(config, self.lambda as cmph_uint32);
            let hash = cmph_new(config);
            cmph_config_destroy(config);
            K::del_cmph_source(source);//was: cmph_io_vector_adapter_destroy(source);
            //to_find_perfect_hash.release();

            //let mut packed_hash = vec![MaybeUninit::<u8>::uninit(); cmph_packed_size(hash) as usize].into_boxed_slice();
            let mut packed_hash = vec![0u8; cmph_packed_size(hash) as usize].into_boxed_slice();
            cmph_pack(hash, packed_hash.as_mut_ptr() as *mut c_void);
            cmph_destroy(hash);

            packed_hash
        }
    }

    #[inline(always)] fn value(mphf: &Self::MPHF, key: &K, _levels: &mut u64) -> Option<u64> {
        let (k_data, k_len) = K::key_data_and_len(key);
        Some(unsafe{ cmph_search_packed(mphf.as_ptr() as *mut c_void, k_data, k_len) as u64 })
    }
}

pub fn chd_benchmark<K: Hash + CMPHSource>(csv_file: &mut Option<File>, i: &(Vec<K>, Vec<K>), conf: &Conf, lambda: u8) {
    let b = CHDConf{ lambda }.benchmark(i, conf);
    if let Some(ref mut f) = csv_file { writeln!(f, "{} {}", lambda, b.all()).unwrap(); }
    println!(" {}\t{}", lambda, b);
}

struct CmphStrVecAdapter<'k> {
    keys: &'k [Box<[u8]>],
    position: usize,
    adapter: cmph_io_adapter_t
}

impl CmphStrVecAdapter<'_> {
    unsafe extern "C" fn read(data: *mut c_void, key: *mut *mut c_char, len: *mut cmph_uint32) -> c_int {
        let data = data.cast::<CmphStrVecAdapter>().as_mut().unwrap();
        let k = unsafe{data.keys.get_unchecked(data.position)};
        *key = k.as_ptr() as *mut u8 as *mut c_char;
        let l = k.len();
        *len = l as cmph_uint32;
        data.position += 1;
        return l as c_int;
    }
    
    unsafe extern "C" fn dispose(_: *mut c_void, _: *mut c_char, _: cmph_uint32) {}
    
    unsafe extern "C" fn rewind(data: *mut c_void) {
        (*data.cast::<CmphStrVecAdapter>()).position = 0;
    }
}

impl CMPHSource for Box<[u8]> {
    fn new_cmph_source(keys: &[Self]) -> *mut cmph_io_adapter_t {
        let mut result = Box::new(CmphStrVecAdapter {keys, position: 0,
            adapter: cmph_io_adapter_t {
                data: std::ptr::null_mut(),
                nkeys: keys.len() as cmph_uint32,
                read: Some(CmphStrVecAdapter::read),
                dispose: Some(CmphStrVecAdapter::dispose),
                rewind: Some(CmphStrVecAdapter::rewind),
            }
        });
        result.adapter.data = result.as_mut() as *mut _ as *mut c_void;
        &mut Box::leak(result).adapter
    }

    fn del_cmph_source(source: *mut cmph_io_adapter_t) {
        drop(unsafe{Box::from_raw((*source).data.cast::<CmphStrVecAdapter>())});
    }

    fn key_data_and_len(key: &Self) -> (*const c_char, u32) {
        (key.as_ptr() as *mut u8 as *mut c_char, key.len() as u32)
    }
}

fn new_cmph_io_struct_vector_adapter<K>(keys: &[K]) -> *mut cmph_io_adapter_t {
    unsafe { cmph_io_struct_vector_adapter(
        keys.as_ptr() as *mut c_void,         // structs
        size_of::<K>() as u32, // struct_size
        0,           // key_offset
        size_of::<K>() as u32, // key_len
        keys.len() as u32) } // nkeys
}

impl CMPHSource for u64 {
    fn new_cmph_source(keys: &[Self]) -> *mut cmph_io_adapter_t {
        new_cmph_io_struct_vector_adapter(keys)
    }

    fn del_cmph_source(source: *mut cmph_io_adapter_t) {
        unsafe{cmph_io_struct_vector_adapter_destroy(source)};
    }

    fn key_data_and_len(key: &Self) -> (*const c_char, u32) {
        (key as *const Self as *const c_char, size_of::<Self>() as u32)
    }
}

impl CMPHSource for u32 {
    fn new_cmph_source(keys: &[Self]) -> *mut cmph_io_adapter_t {
        new_cmph_io_struct_vector_adapter(keys)
    }

    fn del_cmph_source(source: *mut cmph_io_adapter_t) {
        unsafe{cmph_io_struct_vector_adapter_destroy(source)};
    }

    fn key_data_and_len(key: &Self) -> (*const c_char, u32) {
        (key as *const Self as *const c_char, size_of::<Self>() as u32)
    }
}