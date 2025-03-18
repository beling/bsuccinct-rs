use std::{cmp::Reverse, collections::BinaryHeap, ops::Range};
use bitm::{BitAccess, BitVec};
use rayon::iter::{IntoParallelRefMutIterator, ParallelIterator};

use crate::seeds::SeedSize;
use super::{conf::Conf, cyclic::CyclicSet, evaluator::BucketToActivateEvaluator, MAX_SPAN, MAX_VALUES};
use rayon::prelude::*;

pub type UsedValues = CyclicSet<MAX_VALUES>;

#[inline]
fn bucket_sizes_st<SS: SeedSize>(keys: &[u64], conf: &Conf<SS>) -> Box<[usize]> {
    let mut buckets = vec![0; conf.buckets_num + 1].into_boxed_slice();
    for key in keys.iter() { unsafe{ *buckets.get_unchecked_mut(conf.bucket_for(*key)) += 1 } }
    buckets
}

fn bucket_sizes_mt<SS: SeedSize>(keys: &[u64], conf: &Conf<SS>, mut threads_num: usize) -> Box<[usize]> {    
    //let mut threads_num = rayon::current_num_threads().min(keys.len() / (8*4096));
    threads_num = threads_num.min(keys.len() / (8*4096));
    if threads_num <= 1 { return bucket_sizes_st(keys, conf); }
    let mut buckets = vec![0; conf.buckets_num + 1].into_boxed_slice();
    let mut threads = Vec::with_capacity(threads_num);
    let mut remaining_buckets = &mut buckets[..];
    let mut remaining_first = 0;
    let mut key_idx = 0;
    while key_idx < keys.len() {
        let mut new_key_idx = key_idx + (keys.len() - key_idx) / threads_num;
        if let Some(mut last_bucket_for_thread) = keys.get(new_key_idx).map(|key| conf.bucket_for(*key)) {
            let keys_for_thread = &keys[key_idx..new_key_idx];
            unsafe{ *remaining_buckets.get_unchecked_mut(last_bucket_for_thread-remaining_first) += 1 };
            new_key_idx += 1;
            while new_key_idx < keys.len() {
                let b = conf.bucket_for(keys[new_key_idx]);
                unsafe{ *remaining_buckets.get_unchecked_mut(b-remaining_first) += 1 };
                new_key_idx += 1;
                if b != last_bucket_for_thread { break; }
            }
            last_bucket_for_thread += 1;
            let thread_buckets;
            (thread_buckets, remaining_buckets) = remaining_buckets.split_at_mut(last_bucket_for_thread-remaining_first);
            threads.push((keys_for_thread, thread_buckets, remaining_first));
            remaining_first = last_bucket_for_thread;
        } else {
            break;
        }
        key_idx = new_key_idx;
        threads_num -= 1;
    }
    threads.push((&keys[key_idx..], remaining_buckets, remaining_first));
    threads.into_par_iter().for_each(|(keys, buckets, first)| {
        for key in keys.iter() {
            unsafe{ *buckets.get_unchecked_mut(conf.bucket_for(*key)-first) += 1 };
        }
    });
    buckets
}

#[inline(always)]
fn accumulative_sum(buckets: std::slice::IterMut<'_, usize>) -> usize {
    let mut sum = 0;
    for b in buckets { 
        let inc = *b; *b = sum; sum += inc;
        //let old_sum = sum; sum += *b; *b = old_sum;
    }
    sum
}

/// Calculate bucket_begin array using a single thread. `keys` must be sorted.
pub fn bucket_begin_st<SS: SeedSize>(keys: &[u64], conf: &Conf<SS>) -> Box<[usize]> {
    let mut buckets = bucket_sizes_st(keys, conf);
    let sum = accumulative_sum(buckets.iter_mut()); //bucket_lens
    debug_assert_eq!(sum, keys.len());
    buckets
}

/// Calculate bucket_begin array using up to `threads_num` threads. `keys` must be sorted.
pub fn bucket_begin_mt<SS: SeedSize>(keys: &[u64], conf: &Conf<SS>, threads_num: usize) -> Box<[usize]> {
    let mut buckets = bucket_sizes_mt(keys, conf, threads_num);
    let sum = accumulative_sum(buckets.iter_mut()); //bucket_lens
    debug_assert_eq!(sum, keys.len());
    buckets
}

/*pub fn sorted(keys: &[u64], bucket_lens: &mut [usize], conf: &Conf) -> Box<[u64]> {
    //let result = vec![MaybeUninit::uninit(); keys.len()].into_boxed_slice();
    let mut result = Box::<[u64]>::new_uninit_slice(keys.len());
    for key in keys.iter() {
        let bucket = conf.bucket_for(*key);
        let dst = &mut bucket_lens[bucket];
        unsafe { result[*dst].as_mut_ptr().write(*key) };
        //result[*dst].write(*key);
        *dst += 1
    }
    bucket_lens.copy_within(0..bucket_lens.len()-2, 1);
    bucket_lens[0] = 0;
    let mut result = unsafe { result.assume_init() };
    for be in bucket_lens.windows(2) {
        let group = &mut result[be[0]..be[1]];
        if let Some((min_idx, _)) = group.iter().enumerate().min_by_key(|(_, v)| **v) {
            group.swap(0, min_idx);   // move minimum in each group to the begin
        }
    }
    result
}*/

const SMALL_BUCKET_LIMIT: usize = 8;

//// Read-only data shared by all threads.
struct BuildConf<'k, BE: BucketToActivateEvaluator, SS: SeedSize> {
    conf: Conf<SS>,
    span_limit: u16,
    evaluator: BE,
    keys: &'k [u64],
    bucket_begin: Box<[usize]>,
}

fn construct_unassigned(unassigned_len: usize) -> Box<[u64]> {
    let mut unassigned_values = Box::with_filled_bits(unassigned_len);
    if unassigned_len % 64 != 0 {
        *unassigned_values.last_mut().unwrap() >>= 64 - unassigned_len % 64;
    }
    unassigned_values
}

impl<'k, BE: BucketToActivateEvaluator, SS: SeedSize> BuildConf<'k, BE, SS> {
    #[inline]
    pub fn new(keys: &'k [u64], conf: Conf<SS>, span_limit: u16, evaluator: BE, bucket_begin: Box<[usize]>) -> (Self, Box<[SS::VecElement]>) {
        //let seeds = Box::with_zeroed_bits(conf.buckets_num * conf.bits_per_seed() as usize);
        let seeds = conf.new_seeds_vec();
        (Self {
            conf,
            span_limit,
            keys,
            bucket_begin,
            evaluator,
        }, seeds)
    }

    #[inline]
    pub fn process_bucket(&self, bucket: usize, seeds: &[SS::VecElement], unassigned_values: &mut [u64], unassigned_len: &mut usize) {
        let seed = self.conf.bits_per_seed.get_seed(&seeds, bucket);
        if seed == 0 { return; }
        let keys = &self.keys[self.bucket_begin[bucket]..self.bucket_begin[bucket+1]];
        for key_hash in keys {
            unassigned_values.clear_bit(self.conf.f(*key_hash, seed));
        }
        *unassigned_len -= keys.len();
    }

    pub fn unassigned_values(&self, seeds: &[SS::VecElement]) -> (Box<[u64]>, usize) {
        let mut unassigned_len = self.keys.len();
        let mut unassigned_values = construct_unassigned(unassigned_len);
        for bucket in 0..self.bucket_begin.len()-1 {
            self.process_bucket(bucket, seeds, &mut unassigned_values, &mut unassigned_len);
        }
        (unassigned_values, unassigned_len)
    }
}

/*#[inline]
pub fn build_st<'k, BE>(keys: &'k [u64], conf: Conf, span_limit: u16, evaluator: BE) -> Box<[u64]>
    where BE: BucketToActivateEvaluator
{
    let (builder, mut seeds) = BuildConf::new(keys, conf, span_limit, evaluator);
    ThreadBuilder::new(&builder, 0..conf.buckets_num, 0, &mut seeds).build();
    return seeds;
}*/

#[inline] fn gap_for(partition_size: u16, bucket_size100: u16) -> usize {
    (100 * partition_size as usize - 1) / bucket_size100 as usize + 2
}

#[inline(always)]
pub(crate) fn build_st<'k, BE, SS: SeedSize>(keys: &'k [u64], conf: Conf<SS>, evaluator: BE)
-> (Box<[SS::VecElement]>, Box<[u64]>, usize)
where BE: BucketToActivateEvaluator + Send + Sync, BE::Value: Send
{
    let (builder, mut seeds) = BuildConf::new(keys, conf, 256, evaluator, bucket_begin_st(keys, &conf));
    ThreadBuilder::new(&builder, 0..conf.buckets_num, 0, &mut seeds).build();
    let (unassigned_values, unassigned_len) = builder.unassigned_values(&seeds);
    (seeds, unassigned_values, unassigned_len)
}

pub(crate) fn build_mt<'k, BE, SS: SeedSize>(keys: &'k [u64], conf: Conf<SS>, bucket_size100: u16, span_limit: u16, evaluator: BE, threads_num: usize)
 -> (Box<[SS::VecElement]>, Box<[u64]>, usize)
where BE: BucketToActivateEvaluator + Send + Sync, BE::Value: Send
{
    //let threads_num = rayon::current_num_threads();
    let threads_num = threads_num.min(rayon::current_num_threads()).min(conf.buckets_num / 4096).max(1);
    if threads_num == 1 {
        let (builder, mut seeds) = BuildConf::new(keys, conf, span_limit, evaluator, bucket_begin_st(keys, &conf));
        ThreadBuilder::new(&builder, 0..conf.buckets_num, 0, &mut seeds).build();
        let (unassigned_values, unassigned_len) = builder.unassigned_values(&seeds);
        return (seeds, unassigned_values, unassigned_len);
        //return build_st(keys, conf, span_limit, evaluator);
    }
    let bucket_begin = bucket_begin_mt(keys, &conf, threads_num);   // moving down makes program slower
    let chunk_size = SS::VEC_ELEMENT_BIT_SIZE >> conf.bits_per_seed().trailing_zeros();
    // keys_per_thread = buckets number / max_threads rounded to multiple of chunk_size
    let buckets_per_thread = ((conf.buckets_num + (threads_num*chunk_size)/2) / (threads_num*chunk_size)) * chunk_size;
    let seed_words_per_thread = buckets_per_thread * conf.bits_per_seed() as usize / SS::VEC_ELEMENT_BIT_SIZE;
    let (builder, mut seeds) = BuildConf::new(keys, conf, span_limit, evaluator, bucket_begin);
    let mut thread_builders = Vec::with_capacity(threads_num);
    let mut bucket_begin = 0;
    let mut remaining_seeds = &mut seeds[..];
    let gap = gap_for(conf.partition_size(), bucket_size100);
    //dbg!(conf.partition_size(), bucket_size100, gap);
    for _ in 0..threads_num-1 {
        let seeds;
        (seeds, remaining_seeds) = remaining_seeds.split_at_mut(seed_words_per_thread);
        thread_builders.push(ThreadBuilder::new(&builder, bucket_begin..bucket_begin+buckets_per_thread, gap, seeds));
        bucket_begin += buckets_per_thread;
    }
    thread_builders.push(ThreadBuilder::new(&builder, bucket_begin..conf.buckets_num, 0, remaining_seeds));
    /*thread::scope(|s| {
        for thread in &mut thread_builders {
            s.spawn(|| thread.build());
        }
    });*/
    thread_builders.par_iter_mut().for_each(ThreadBuilder::build);  // marginally faster than std
    for next in 1..thread_builders.len() {
        let prev = next-1;
        for bucket in 0..gap {
            let seed = conf.bits_per_seed.get_seed(&thread_builders[next].seeds, bucket) as u16;
            if seed == 0 { continue; }
            for key in &builder.keys[thread_builders[next].bucket_begin[bucket]..thread_builders[next].bucket_begin[bucket+1]] {
                thread_builders[prev].used_values.add(builder.conf.f(*key, seed));
            }
        }
        thread_builders[prev].buckets_num += gap;
        thread_builders[prev].build();
    }
    drop(thread_builders);
    let (unassigned_values, unassigned_len) = builder.unassigned_values(&seeds);
    (seeds, unassigned_values, unassigned_len)
}

/// Data stored by each thread.
// Align, to prevent false sharing, copied from crossbean library:
// Starting from Intel's Sandy Bridge, spatial prefetcher is now pulling pairs of 64-byte cache
// lines at a time, so we have to align to 128 bytes rather than 64.
//
// Sources:
// - https://www.intel.com/content/dam/www/public/us/en/documents/manuals/64-ia-32-architectures-optimization-manual.pdf
// - https://github.com/facebook/folly/blob/1b5288e6eea6df074758f877c849b6e73bbb9fbb/folly/lang/Align.h#L107
//
// ARM's big.LITTLE architecture has asymmetric cores and "big" cores have 128-byte cache line size.
//
// Sources:
// - https://www.mono-project.com/news/2016/09/12/arm64-icache/
//
// powerpc64 has 128-byte cache line size.
//
// Sources:
// - https://github.com/golang/go/blob/3dd58676054223962cd915bb0934d1f9f489d4d2/src/internal/cpu/cpu_ppc64x.go#L9
// - https://github.com/torvalds/linux/blob/3516bd729358a2a9b090c1905bd2a3fa926e24c6/arch/powerpc/include/asm/cache.h#L26
#[cfg_attr(
    any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "powerpc64",
    ),
    repr(align(128))
)]
// arm, mips, mips64, sparc, and hexagon have 32-byte cache line size.
//
// Sources:
// - https://github.com/golang/go/blob/3dd58676054223962cd915bb0934d1f9f489d4d2/src/internal/cpu/cpu_arm.go#L7
// - https://github.com/golang/go/blob/3dd58676054223962cd915bb0934d1f9f489d4d2/src/internal/cpu/cpu_mips.go#L7
// - https://github.com/golang/go/blob/3dd58676054223962cd915bb0934d1f9f489d4d2/src/internal/cpu/cpu_mipsle.go#L7
// - https://github.com/golang/go/blob/3dd58676054223962cd915bb0934d1f9f489d4d2/src/internal/cpu/cpu_mips64x.go#L9
// - https://github.com/torvalds/linux/blob/3516bd729358a2a9b090c1905bd2a3fa926e24c6/arch/sparc/include/asm/cache.h#L17
// - https://github.com/torvalds/linux/blob/3516bd729358a2a9b090c1905bd2a3fa926e24c6/arch/hexagon/include/asm/cache.h#L12
#[cfg_attr(
    any(
        target_arch = "arm",
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "mips64",
        target_arch = "mips64r6",
        target_arch = "sparc",
        target_arch = "hexagon",
    ),
    repr(align(32))
)]
// m68k has 16-byte cache line size.
//
// Sources:
// - https://github.com/torvalds/linux/blob/3516bd729358a2a9b090c1905bd2a3fa926e24c6/arch/m68k/include/asm/cache.h#L9
#[cfg_attr(target_arch = "m68k", repr(align(16)))]
// s390x has 256-byte cache line size.
//
// Sources:
// - https://github.com/golang/go/blob/3dd58676054223962cd915bb0934d1f9f489d4d2/src/internal/cpu/cpu_s390x.go#L7
// - https://github.com/torvalds/linux/blob/3516bd729358a2a9b090c1905bd2a3fa926e24c6/arch/s390/include/asm/cache.h#L13
#[cfg_attr(target_arch = "s390x", repr(align(256)))]
// x86, wasm, riscv, and sparc64 have 64-byte cache line size.
//
// Sources:
// - https://github.com/golang/go/blob/dda2991c2ea0c5914714469c4defc2562a907230/src/internal/cpu/cpu_x86.go#L9
// - https://github.com/golang/go/blob/3dd58676054223962cd915bb0934d1f9f489d4d2/src/internal/cpu/cpu_wasm.go#L7
// - https://github.com/torvalds/linux/blob/3516bd729358a2a9b090c1905bd2a3fa926e24c6/arch/riscv/include/asm/cache.h#L10
// - https://github.com/torvalds/linux/blob/3516bd729358a2a9b090c1905bd2a3fa926e24c6/arch/sparc/include/asm/cache.h#L19
//
// All others are assumed to have 64-byte cache line size.
#[cfg_attr(
    not(any(
        target_arch = "x86_64",
        target_arch = "aarch64",
        target_arch = "powerpc64",
        target_arch = "arm",
        target_arch = "mips",
        target_arch = "mips32r6",
        target_arch = "mips64",
        target_arch = "mips64r6",
        target_arch = "sparc",
        target_arch = "hexagon",
        target_arch = "m68k",
        target_arch = "s390x",
    )),
    repr(align(64))
)]
struct ThreadBuilder<'k, BE: BucketToActivateEvaluator, SS: SeedSize> {
    conf: &'k BuildConf<'k, BE, SS>,

    /// buckets to process by the thread
    bucket_begin: &'k [usize],

    /// First bucket in the span.
    span_begin: usize,

    /// number of buckets to process by the thread
    buckets_num: usize,

    /// Values used by committed choices.
    used_values: UsedValues,
    /// Next value to remove from `used_values`.
    value_to_clear: usize,

    candidates_to_active: BinaryHeap<(BE::Value, Reverse<usize>)>,    // (value, bucket)
    in_candidates_to_active: CyclicSet<MAX_SPAN>,

    seeds: &'k mut [SS::VecElement]
}

impl<'k, BE: BucketToActivateEvaluator, SS: SeedSize> ThreadBuilder<'k, BE, SS> {
    pub(crate) fn new(conf: &'k BuildConf<'k, BE, SS>, buckets: Range<usize>, gap: usize, seeds: &'k mut [SS::VecElement]) -> Self {
        Self {
            used_values: UsedValues::default(),
            conf,
            span_begin: 0,
            buckets_num: buckets.len()-gap,
            value_to_clear: 0,
            candidates_to_active: BinaryHeap::with_capacity(conf.span_limit as usize),
            in_candidates_to_active: CyclicSet::default(),
            bucket_begin: &conf.bucket_begin[buckets.start..buckets.end+1],
            seeds,
        }
    }

    pub(crate) fn build(&mut self) {
        //let mut seeds = vec![0; self.conf.buckets_num].into_boxed_slice();
        if !self.find_nonempty() { return; }
        self.value_to_clear = self.partition_begin(self.span_begin);
        self.add_candidates_from(self.span_begin);
        while let Some((_, Reverse(best_bucket))) = self.candidates_to_active.pop() {
        //while let Some(best_bucket) = self.extract_best_bucket() {
            self.in_candidates_to_active.remove(best_bucket);
            let best_seed = self.best_seed(best_bucket);
            //self.seeds.set_fragment(best_bucket, best_seed as u64, self.conf.conf.bits_per_seed());
            self.conf.conf.bits_per_seed.set_seed(&mut self.seeds, best_bucket, best_seed as u16);
            if best_bucket == self.span_begin {
                let old_span_end = self.span_end();
                self.span_begin += 1;
                while self.span_begin < old_span_end && !self.in_candidates_to_active.contain(self.span_begin) {
                    self.span_begin += 1;
                }
                if self.span_begin == old_span_end {
                    if !self.find_nonempty() { return; }
                }
                self.clear_used();
                self.add_candidates_from(old_span_end);
            }
        }
    }

    #[inline]
    fn find_nonempty(&mut self) -> bool {
        loop {
            if self.span_begin == self.buckets_num { return false; }
            if !self.bucket_is_empty(self.span_begin) { return true; }
            self.span_begin += 1;
        }
    }

    #[inline]
    fn add_candidates_from(&mut self, first_to_add: usize) {
        for bucket in first_to_add..self.span_end() {
            let bucket_size = self.bucket_size(bucket);
            if bucket_size != 0 {
                self.candidates_to_active.push((self.conf.evaluator.eval(bucket, bucket_size), Reverse(bucket)));
                self.in_candidates_to_active.add(bucket);
            }
        }
    }

    /// Clear used before span_begin.
    #[inline]
    pub fn clear_used(&mut self) {
        let end = self.partition_begin(self.span_begin);
        while self.value_to_clear != end {
            self.used_values.remove(self.value_to_clear);
            self.value_to_clear += 1;
        }
    }

    #[inline]
    fn partition_begin(&self, non_empty_bucket: usize) -> usize {
        self.conf.conf.partition_begin(self.conf.keys[self.bucket_begin[non_empty_bucket]])
    }

    fn best_seed_big(&mut self, keys: &[u64]) -> u16 {
        let mut best_value = usize::MAX;
        let mut best_seed = 0;
        let mut values_used_by_seed = Vec::with_capacity(keys.len());
        let simd_keys = keys.len() / 4 * 4;
        //assert!(simd_keys <= keys.len());
        'outer: for seed in 1u16..self.conf.conf.seeds_num() {    // seed=0 is special = no seed,
            values_used_by_seed.clear();
            for i in (0..simd_keys).step_by(4) {
                let values = [
                    self.conf.conf.f(keys[i], seed),
                    self.conf.conf.f(keys[i+1], seed),
                    self.conf.conf.f(keys[i+2], seed),
                    self.conf.conf.f(keys[i+3], seed),
                ];
                let contains = [
                    self.used_values.contain(values[0]),
                    self.used_values.contain(values[1]),
                    self.used_values.contain(values[2]),
                    self.used_values.contain(values[3]),
                ];
                if contains.iter().any(|b| *b) { continue 'outer; }
                //if contains[0] || contains[1] || contains[2] || contains[3] { continue 'outer; }
                values_used_by_seed.push(values[0]);
                values_used_by_seed.push(values[1]);
                values_used_by_seed.push(values[2]);
                values_used_by_seed.push(values[3]);
            }
            //assert!(keys.len() - simd_keys < 4);
            for i in simd_keys..keys.len() {
                let value = self.conf.conf.f(keys[i], seed);
                if self.used_values.contain(value) { continue 'outer; }
                values_used_by_seed.push(value);
            }
            let seed_value = values_used_by_seed.iter().sum();
            if seed_value < best_value {
                values_used_by_seed.sort();
                if values_used_by_seed.windows(2).any(|v| v[0]==v[1]) {
                    continue;
                }
                best_value = seed_value;
                best_seed = seed;
            }
        }
        best_seed
    }

    #[inline]
    fn best_seed_small(&mut self, keys: &[u64]) -> u16 {
        assert!(keys.len() <= SMALL_BUCKET_LIMIT);  // seems to speeds up a bit
        let mut best_value = usize::MAX;
        let mut best_seed = 0;
        let mut values_used_by_seed = arrayvec::ArrayVec::<_, SMALL_BUCKET_LIMIT>::new(); // Vec::with_capacity(keys.len());
        'outer: for seed in 1u16..self.conf.conf.seeds_num() {    // seed=0 is special = no seed,
            values_used_by_seed.clear();
            for key in keys.iter().copied() {
                let value = self.conf.conf.f(key, seed);
                if self.used_values.contain(value) { continue 'outer; }
                values_used_by_seed.push(value);
            }
            let seed_value = values_used_by_seed.iter().sum();
            if seed_value < best_value {
                values_used_by_seed.sort_unstable();
                for i in 1..values_used_by_seed.len() {
                    if values_used_by_seed[i-1] == values_used_by_seed[i] { continue 'outer; }
                }
                best_value = seed_value;
                best_seed = seed;
            }
        }
        best_seed
    }

    #[inline]
    fn best_seed(&mut self, bucket_nr: usize) -> u16 {
        let keys = &self.conf.keys[self.bucket_begin[bucket_nr]..self.bucket_begin[bucket_nr+1]];
        //if keys.len() == 1 { return self.best_seed1(keys[0]); }
        let best_seed = if keys.len() <= SMALL_BUCKET_LIMIT {
            self.best_seed_small(keys)
        } else {
            self.best_seed_big(keys)
        };
        if best_seed != 0 { // can assign seed to the bucket
            for key in keys {
                self.used_values.add(self.conf.conf.f(*key, best_seed));
            }
        };
        best_seed
    }

    /// Number of the last bucket included in the span limit + 1.
    #[inline]
    pub(crate) fn span_end(&self) -> usize {
        (self.span_begin + self.conf.span_limit as usize).min(self.buckets_num)
    }

    #[inline]
    pub(crate) fn bucket_size(&self, bucket_nr: usize) -> usize {
        self.bucket_begin[bucket_nr+1] - self.bucket_begin[bucket_nr]
    }

    #[inline]
    pub(crate) fn bucket_is_empty(&self, bucket_nr: usize) -> bool {
        self.bucket_begin[bucket_nr+1] == self.bucket_begin[bucket_nr]
    }
}
