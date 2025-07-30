use std::{cmp::Reverse, collections::BinaryHeap, ops::Range};
use bitm::{BitAccess, BitVec};
use rayon::iter::{IntoParallelRefMutIterator, ParallelIterator};

use crate::seeds::SeedSize;
use super::{conf::Conf, cyclic::CyclicSet, cyclic::GenericUsedValue, evaluator::BucketToActivateEvaluator, seed_chooser::{SeedChooser, SeedOnlyNoBump}, MAX_WINDOW_SIZE, WINDOW_SIZE};
use rayon::prelude::*;

#[inline]
fn bucket_sizes_st(keys: &[u64], conf: &Conf) -> Box<[usize]> {
    let mut buckets = vec![0; conf.buckets_num + 1].into_boxed_slice();
    for key in keys.iter() { unsafe{ *buckets.get_unchecked_mut(conf.bucket_for(*key)) += 1 } }
    buckets
}

fn bucket_sizes_mt(keys: &[u64], conf: &Conf, mut threads_num: usize) -> Box<[usize]> { 
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
pub fn bucket_begin_st(keys: &[u64], conf: &Conf) -> Box<[usize]> {
    let mut buckets = bucket_sizes_st(keys, conf);
    let sum = accumulative_sum(buckets.iter_mut()); //bucket_lens
    debug_assert_eq!(sum, keys.len());
    buckets
}

/// Calculate bucket_begin array using up to `threads_num` threads. `keys` must be sorted.
pub fn bucket_begin_mt(keys: &[u64], conf: &Conf, threads_num: usize) -> Box<[usize]> {
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

//// Read-only data shared by all threads.
pub(crate) struct BuildConf<'k, BE: BucketToActivateEvaluator, SS: SeedSize, SC: SeedChooser> {
    conf: Conf,
    span_limit: u16,
    evaluator: BE,
    keys: &'k [u64],
    bucket_begin: Box<[usize]>,
    pub(crate) seed_chooser: SC,
    seed_size: SS
}

fn construct_unassigned(unassigned_len: usize) -> Box<[u64]> {
    let mut unassigned_values = Box::with_filled_bits(unassigned_len);
    if unassigned_len % 64 != 0 {
        *unassigned_values.last_mut().unwrap() >>= 64 - unassigned_len % 64;
    }
    unassigned_values
}

impl<'k, BE: BucketToActivateEvaluator, SS: SeedSize, SC: SeedChooser> BuildConf<'k, BE, SS, SC> {
    #[inline]
    pub fn new(keys: &'k [u64], conf: Conf, seed_size: SS, span_limit: u16, evaluator: BE, bucket_begin: Box<[usize]>, seed_chooser: SC) -> (Self, Box<[SS::VecElement]>) {
        //let seeds = Box::with_zeroed_bits(conf.buckets_num * conf.bits_per_seed() as usize);
        let seeds = conf.new_seeds_vec(seed_size);
        (Self {
            conf,
            span_limit,
            keys,
            bucket_begin,
            evaluator,
            seed_chooser,
            seed_size
        }, seeds)
    }

    /// Clears bits of `unassigned_values` occupied by the keys in given `bucket`.
    /// Decreases `unassigned_len` by `keys.len()`.
    #[inline]
    pub fn clear_assigned_from_bucket(&self, bucket: usize, seeds: &[SS::VecElement], unassigned_values: &mut [u64], unassigned_len: &mut usize) {
        let seed = self.seed_size.get_seed(&seeds, bucket);
        if SC::BUMPING && seed == 0 { return; }
        let keys = &self.keys[self.bucket_begin[bucket]..self.bucket_begin[bucket+1]];
        for key_hash in keys {
            //debug_assert!(unassigned_values.get_bit(self.seed_chooser.f(*key_hash, seed, &self.conf)));
            unassigned_values.clear_bit(self.seed_chooser.f(*key_hash, seed, &self.conf));
        }
        *unassigned_len -= keys.len();
    }

    /// Calculates bitmap of unassigned values and number of unassigned values of 1-perfect function.
    pub fn unassigned_values(&self, seeds: &[SS::VecElement]) -> (Box<[u64]>, usize) {
        let mut unassigned_len = self.conf.output_range(&self.seed_chooser, self.seed_size.into());
        let mut unassigned_values = construct_unassigned(unassigned_len);
        for bucket in 0..self.bucket_begin.len()-1 {
            self.clear_assigned_from_bucket(bucket, seeds, &mut unassigned_values, &mut unassigned_len);
        }
        (unassigned_values, unassigned_len)
    }

    /// Calculates number of unassigned keys.
    pub fn unassigned_len(&self, seeds: &[SS::VecElement]) -> usize {
        if !SC::BUMPING { return 0; }
        (0..self.bucket_begin.len()-1)
            .filter(|bucket| self.seed_size.get_seed(&seeds, *bucket) == 0)
            .map(|bucket| self.bucket_begin[bucket+1] - self.bucket_begin[bucket])
            .sum()

        /*let mut unassigned_len = 0;
        for bucket in 0..self.bucket_begin.len()-1 {
            if self.conf.bits_per_seed.get_seed(&seeds, bucket) == 0 {
                unassigned_len += self.bucket_begin[bucket+1] - self.bucket_begin[bucket];
            }
        }
        unassigned_len*/
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

#[inline] fn gap_for(effective_slice_len: u16, bucket_num: usize, output_range: usize) -> usize {
    let effective_slice_len = effective_slice_len as usize;
    effective_slice_len * bucket_num / (output_range + 1 - effective_slice_len) + 1
}

/// Returns gap size for given `slice_len` and `bucket_size100`.
/*#[inline] fn gap_for_old(slice_len: u16, bucket_size100: u16) -> usize {
    // roundup((L + lambda) / lambda) =
    (100 * slice_len as usize - 1) / bucket_size100 as usize + 2
}*/

/*#[inline] fn gap_for(slice_len: u16, number_of_buckets: usize, number_of_keys: usize) -> usize {
    // roundup((L + lambda) / lambda) =
    (slice_len as usize * number_of_buckets - 1) / number_of_keys + 2   + 20
}*/

#[inline(always)]
pub(crate) fn build_last_level<'k, BE, SS: SeedSize>(keys: &'k [u64], conf: Conf, seed_size: SS, evaluator: BE)
-> Option<(Box<[SS::VecElement]>, Box<[u64]>, usize)>
where BE: BucketToActivateEvaluator + Send + Sync, BE::Value: Send
{
    let (builder, mut seeds) = BuildConf::new(keys, conf, seed_size, WINDOW_SIZE, evaluator, bucket_begin_st(keys, &conf), SeedOnlyNoBump);
    let mut tb = ThreadBuilder::<SeedOnlyNoBump, _, _>::new(&builder, 0..conf.buckets_num, 0, &mut seeds);
    tb.build();
    if !tb.finished() { return None; }
    drop(tb);
    let (unassigned_values, unassigned_len) = builder.unassigned_values(&seeds);
    Some((seeds, unassigned_values, unassigned_len))
}

#[inline(always)]
pub(crate) fn build_st<'k, SC, BE, SS: SeedSize>(keys: &'k [u64], conf: Conf, seed_size: SS, evaluator: BE, seed_chooser: SC)
-> (Box<[SS::VecElement]>, BuildConf<'k, BE, SS, SC>)
where SC: SeedChooser, BE: BucketToActivateEvaluator
{
    let (builder, mut seeds) = BuildConf::new(keys, conf, seed_size, WINDOW_SIZE, evaluator, bucket_begin_st(keys, &conf), seed_chooser);
    ThreadBuilder::<SC, _, _>::new(&builder, 0..conf.buckets_num, 0, &mut seeds).build();
    //let (unassigned_values, unassigned_len) = builder.unassigned_values(&seeds);
    (seeds, builder)
}

pub(crate) fn build_mt<'k, SC, BE, SS: SeedSize>(keys: &'k [u64], conf: Conf, seed_size: SS, span_limit: u16, evaluator: BE, seed_chooser: SC, threads_num: usize)
 -> (Box<[SS::VecElement]>, BuildConf<'k, BE, SS, SC>)
where SC: SeedChooser + Sync, BE: BucketToActivateEvaluator + Sync, BE::Value: Send
{
    //let threads_num = rayon::current_num_threads();
    let threads_num = threads_num.min(rayon::current_num_threads()).min(conf.buckets_num / 4096).max(1);
    if threads_num == 1 {
        let (builder, mut seeds) = BuildConf::new(keys, conf, seed_size, span_limit, evaluator, bucket_begin_st(keys, &conf), seed_chooser);
        ThreadBuilder::<SC, _, _>::new(&builder, 0..conf.buckets_num, 0, &mut seeds).build();
        //let (unassigned_values, unassigned_len) = builder.unassigned_values(&seeds);
        return (seeds, builder);
        //return build_st(keys, conf, span_limit, evaluator);
    }
    let bucket_begin = bucket_begin_mt(keys, &conf, threads_num);   // moving down makes program slower
    let chunk_size = SS::VEC_ELEMENT_BIT_SIZE >> seed_size.into().trailing_zeros();
    // keys_per_thread = buckets number / max_threads rounded to multiple of chunk_size
    let buckets_per_thread = ((conf.buckets_num + (threads_num*chunk_size)/2) / (threads_num*chunk_size)) * chunk_size;
    let seed_words_per_thread = buckets_per_thread * seed_size.into() as usize / SS::VEC_ELEMENT_BIT_SIZE;
    let (builder, mut seeds) = BuildConf::new(keys, conf, seed_size, span_limit, evaluator, bucket_begin, seed_chooser);
    let mut thread_builders = Vec::with_capacity(threads_num);
    let mut bucket_begin = 0;
    let mut remaining_seeds = &mut seeds[..];
    let gap = gap_for(conf.slice_len() + builder.seed_chooser.extra_shift(seed_size.into()),
        conf.buckets_num, conf.output_range(&builder.seed_chooser, seed_size.into()));
    //dbg!(conf.slice_len(), bucket_size100, gap);
    for _ in 0..threads_num-1 {
        let seeds;
        (seeds, remaining_seeds) = remaining_seeds.split_at_mut(seed_words_per_thread);
        thread_builders.push(ThreadBuilder::new(&builder, bucket_begin..bucket_begin+buckets_per_thread, gap, seeds));
        bucket_begin += buckets_per_thread;
    }
    thread_builders.push(ThreadBuilder::<SC, _, _>::new(&builder, bucket_begin..conf.buckets_num, 0, remaining_seeds));
    /*thread::scope(|s| {
        for thread in &mut thread_builders {
            s.spawn(|| thread.build());
        }
    });*/
    thread_builders.par_iter_mut().for_each(ThreadBuilder::<SC, _, _>::build);  // marginally faster than std
    for next in 1..thread_builders.len() {
        let prev = next-1;
        for bucket in 0..gap {
            let seed = seed_size.get_seed(&thread_builders[next].seeds, bucket) as u16;
            if SC::BUMPING && seed == 0 { continue; }
            for key in &builder.keys[thread_builders[next].bucket_begin[bucket]..thread_builders[next].bucket_begin[bucket+1]] {
                thread_builders[prev].used_values.add(builder.seed_chooser.f(*key, seed, &builder.conf));
            }
        }
        thread_builders[prev].buckets_num += gap;
        thread_builders[prev].build();
    }
    drop(thread_builders);
    //let (unassigned_values, unassigned_len) = builder.unassigned_values(&seeds);
    (seeds, builder)
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
struct ThreadBuilder<'k, SC: SeedChooser, BE: BucketToActivateEvaluator, SS: SeedSize> {
    conf: &'k BuildConf<'k, BE, SS, SC>,

    /// buckets to process by the thread
    bucket_begin: &'k [usize],

    /// First bucket in the span.
    span_begin: usize,

    /// number of buckets to process by the thread
    buckets_num: usize,

    /// Values used by committed choices.
    used_values: SC::UsedValues,
    /// Next value to remove from `used_values`.
    value_to_clear: usize,

    candidates_to_active: BinaryHeap<(BE::Value, Reverse<usize>)>,    // (value, bucket)
    in_candidates_to_active: CyclicSet<{MAX_WINDOW_SIZE/64}>,

    seeds: &'k mut [SS::VecElement],
}

impl<'k, SC: SeedChooser, BE: BucketToActivateEvaluator, SS: SeedSize> ThreadBuilder<'k, SC, BE, SS> {
    pub(crate) fn new(conf: &'k BuildConf<'k, BE, SS, SC>, buckets: Range<usize>, gap: usize, seeds: &'k mut [SS::VecElement]) -> Self {
        Self {
            used_values: SC::UsedValues::default(),
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
        self.value_to_clear = self.slice_begin(self.span_begin); // / 64;
        self.add_candidates_from(self.span_begin);
        while let Some((_, Reverse(best_bucket))) = self.candidates_to_active.pop() {
        //while let Some(best_bucket) = self.extract_best_bucket() {
            self.in_candidates_to_active.remove(best_bucket);
            let best_seed = self.best_seed(best_bucket);
            if !SC::BUMPING && best_seed == u16::MAX { return; }
            //self.seeds.set_fragment(best_bucket, best_seed as u64, self.conf.conf.bits_per_seed());
            self.conf.seed_size.set_seed(&mut self.seeds, best_bucket, best_seed);
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

    /// Move `span_begin` forward to the first non-empty bucket (and returns `true`)
    /// or to the end (and returns `false`).
    #[inline]
    fn find_nonempty(&mut self) -> bool {
        loop {
            if self.span_begin == self.buckets_num { return false; }
            if !self.bucket_is_empty(self.span_begin) { return true; }
            self.span_begin += 1;
        }
    }

    /// Returns whether the construction was successfully completed.
    #[inline]
    fn finished(&self) -> bool {
        self.span_begin == self.buckets_num
    }

    /// Adds buckets `[first_to_add, span_end())` to candidates queue.
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
        let end = self.slice_begin(self.span_begin); // / 64;
        while self.value_to_clear != end {  // TODO clear in 64-bit steps
            self.used_values.remove(self.value_to_clear);
            //self.used_values.remove_fragment_64(self.value_to_clear);
            self.value_to_clear += 1;
        }
    }

    #[inline]
    fn slice_begin(&self, non_empty_bucket: usize) -> usize {
        self.conf.conf.slice_begin(self.conf.keys[self.bucket_begin[non_empty_bucket]])
    }

    #[inline(always)]
    fn best_seed(&mut self, bucket_nr: usize) -> u16 {
        let keys = &self.conf.keys[self.bucket_begin[bucket_nr]..self.bucket_begin[bucket_nr+1]];
        self.conf.seed_chooser.best_seed(&mut self.used_values, keys, &self.conf.conf, self.conf.seed_size.into())
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
