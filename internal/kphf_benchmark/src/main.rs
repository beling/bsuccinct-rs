#![doc = include_str!("../README.md")]

use ph::{GetSize, fmph::{Bits8, SeedSize}, phast::{Generic, KFunction, Perfect, SeedOnlyK, bucket_size_normalization_multiplier, compressed_array::DefaultCompressedArray}, seeds::BitsFast};

use crate::kptrhash::{KptrHash, Mode, space_lower_bound};

mod kptrhash;

/*pub fn benchmark<R, F: FnOnce() -> R>(f: F) -> (R, Duration) {
    let start_moment = Instant::now();
    let r = f();
    let time = start_moment.elapsed();
    (r, time)
}*/

pub fn keys(keys_num: usize, seed: u32) -> Box<[u32]> {
    butils::XorShift32(seed).take(keys_num as usize).collect()
}

//const ALPHAS: [f32; 6] = [0.7, 0.8, 0.9, 0.95, 0.98, 0.99];
//const FACTORS: [f32; 7] = [2.5, 2.25, 2.0, 1.75, 1.5, 1.25, 1.0];
//const QUERY: bool = true;

fn bench_kptrhash<const MODE: u8, const K: usize>(alpha: f32, factor: f32, keys: &[u32]) {
    let n = keys.len();
    let lb = space_lower_bound(K, alpha);
    let target_bits_per_key = lb * factor;
    // Construction
    let t0 = std::time::Instant::now();
    let kphf = KptrHash::<MODE, K>::new(alpha, target_bits_per_key, &keys).unwrap();
    let build_ns: f32 = t0.elapsed().as_nanos() as f32 / n as f32;

    // Space
    let actual_bits_per_key = kphf.bits_used() as f32 / n as f32;
    //let bumped_frac = kphf.num_bumped() as f32 / n as f32;
    let actual_alpha = n as f32 / (kphf.num_bins() as f32 * K as f32);
   
    let start = std::time::Instant::now();
    for key in keys {
        std::hint::black_box(kphf.get(*key));
    }

    let loop_ns = start.elapsed().as_nanos() as f32 / n as f32;

    /*let start = std::time::Instant::now();
    let lookahead = 32;
    for i in 0..keys.len().saturating_sub(lookahead) {
        kphf.prefetch(keys[i + lookahead]);
        std::hint::black_box(kphf.get(keys[i]));
    }
    let throughput_ns = start.elapsed().as_nanos() as f32 / n as f32;*/

    println!("kPtrHash Sort+Bump α={alpha:.2} factor={factor}  α={actual_alpha:.3}  {actual_bits_per_key:.3} bit/key  {build_ns:.0} {loop_ns:.2} ns/key");
}

fn bench_kphast<SS: SeedSize>(k: u16, mut conf: ph::phast::Conf<SS, Generic>, keys: &[u32]) {
    let n = keys.len();
    conf.core_conf.bucket_size100 = (conf.core_conf.bucket_size100 as f64 * bucket_size_normalization_multiplier(k)) as _;
    let bucket_size_100 = conf.core_conf.bucket_size100;
    let seed_size: u8 = conf.seed_size.into();

    let t0 = std::time::Instant::now();
    let f: KFunction<_, _, _, DefaultCompressedArray, _> = ph::phast::KFunction::with_slice_conf_sc(keys, conf, SeedOnlyK::new(k));
    let build_ns: f32 = t0.elapsed().as_nanos() as f32 / n as f32;
   
    let start = std::time::Instant::now();
    for key in keys {
        std::hint::black_box(f.get(key));
    }
    let loop_ns = start.elapsed().as_nanos() as f32 / n as f32;

    let actual_bits_per_key = (f.size_bytes() * 8) as f32 / n as f32;
    let actual_alpha = n as f32 / (f.output_range() as f32 * k as f32);
    println!("kPHast S={seed_size} repaired λ={:.2}  α={actual_alpha:.3}  {actual_bits_per_key:.3} bit/key  {build_ns:.0} {loop_ns:.2} ns/key",
            bucket_size_100 as f64/100.0);
}

fn bench_perfect<SS: SeedSize>(k: u16, mut conf: ph::phast::Conf<SS, Generic>, keys: &[u32]) {
    let n = keys.len();
    conf.core_conf.bucket_size100 = (conf.core_conf.bucket_size100 as f64 * bucket_size_normalization_multiplier(k)) as _;
    let conf_loading_factor = conf.loading_factor_1000 as f32 / 1000.0;
    let bucket_size_100 = conf.core_conf.bucket_size100;
    let seed_size: u8 = conf.seed_size.into();

    let t0 = std::time::Instant::now();
    let f = Perfect::with_slice_conf_sc(keys, conf, SeedOnlyK::new(k));
    let build_ns: f32 = t0.elapsed().as_nanos() as f32 / n as f32;
   
    let start = std::time::Instant::now();
    for key in keys {
        std::hint::black_box(f.get(key));
    }
    let loop_ns = start.elapsed().as_nanos() as f32 / n as f32;

    let actual_bits_per_key = (f.size_bytes() * 8) as f32 / n as f32;
    let actual_alpha = n as f32 / (f.output_range() as f32 * k as f32);
    println!("kPHast S={seed_size} α={conf_loading_factor:.2} λ={:.2}  α={actual_alpha:.3}  {actual_bits_per_key:.3} bit/key  {build_ns:.0} {loop_ns:.2} ns/key", bucket_size_100 as f64/100.0);
}

fn conf<SS: SeedSize>(seed_size: SS, alpha: f32, bucket_size100: u32, preferred_slice_len: u16) -> ph::phast::Conf<SS, Generic> {
    ph::phast::Conf{
        loading_factor_1000: (alpha*1000.0).round() as u16,
        seed_size,
        core_conf: Generic::with_psl(bucket_size100, preferred_slice_len),
        hasher: ph::seedable_hash::BuildGxHash,
    }
}

fn k16_a99(keys: &[u32]) {
    println!("---- k=16 α=0.99 ----");
    bench_kptrhash::<{ Mode::SortBump as u8 }, 16>(0.99, 1.5, &keys);

    bench_perfect(16, conf(Bits8, 1.0, 570, 0), &keys);
    bench_perfect(16, conf(BitsFast(6), 1.0, 406, 0), &keys);

    bench_kphast(16, conf(Bits8, 0.97, 520, 0), &keys);
    bench_kphast(16, conf(BitsFast(6), 0.97, 390, 0), &keys);
}

fn k4_a99(keys: &[u32]) {
    println!("---- k=4 α=0.99 ----");
    bench_kptrhash::<{ Mode::SortBump as u8 }, 4>(0.99, 1.5, &keys);

    bench_perfect(4, conf(Bits8, 1.0, 482, 0), &keys);
    bench_perfect(4, conf(BitsFast(6), 1.0, 340, 0), &keys);

    bench_kphast(4, conf(Bits8, 0.98, 480, 0), &keys);
    bench_kphast(4, conf(BitsFast(6), 0.98, 340, 0), &keys);
}

fn k16_a90(keys: &[u32]) {
    println!("---- k=16 α=0.9 ----");
    bench_kptrhash::<{ Mode::SortBump as u8 }, 16>(0.9, 1.5, &keys);    // α=0.896 0.098

    bench_perfect(16, conf(Bits8, 0.91, 1192, 0), &keys);  // 0.099
    bench_perfect(16, conf(BitsFast(6), 0.91, 846, 0), &keys); // 0.104

    //bench_kphast(16, conf(Bits8, 0.896, 1100, 0), &keys);  // 0.109
    //bench_kphast(16, conf(BitsFast(6), 0.896, 775, 0), &keys); // 0.116
}

fn k4_a90(keys: &[u32]) {
    println!("---- k=4 α=0.9 ----");
    bench_kptrhash::<{ Mode::SortBump as u8 }, 4>(0.9, 1.5, &keys); // α=0.900 0.490

    bench_perfect(4, conf(Bits8, 0.91, 754, 0), &keys);    // 0.438
    bench_perfect(4, conf(BitsFast(6), 0.91, 533, 0), &keys);  // 0.465

    //bench_kphast(4, conf(Bits8, 0.9, 720, 0), &keys);  // 0.462
    //bench_kphast(4, conf(BitsFast(6), 0.9, 500, 0), &keys);    // 0.496
}


fn k4_a80(keys: &[u32]) {
    println!("---- k=4 α=0.8 ----");
    bench_kptrhash::<{ Mode::SortBump as u8 }, 4>(0.8, 1.5, &keys); // α=0.800 0.316

    bench_perfect(4, conf(Bits8, 0.81, 1167, 0), &keys);  // 0.283
    bench_perfect(4, conf(BitsFast(6), 0.81, 830, 0), &keys);  // 0.299

    //bench_kphast(4, conf(Bits8, 0.8, 1070, 0), &keys); // 0.310
    //bench_kphast(4, conf(BitsFast(6), 0.8, 730, 0), &keys);    // 0.345
}

fn k16_a80(keys: &[u32]) {  //??
    println!("---- k=16 α=0.8 ----");
    bench_kptrhash::<{ Mode::SortBump as u8 }, 16>(0.8, 1.5, &keys); // α=0.798 0.041

    bench_perfect(16, conf(Bits8, 0.81, 2450, 0), &keys);  // 0.049
    bench_perfect(16, conf(BitsFast(6), 0.81, 1780, 0), &keys);   // 0.049
}

fn main() {
    let keys = keys(100_000_000, 1234);

    for b in (2350..2800).step_by(10) {
        println!("{b}");
        bench_perfect(16, conf(Bits8, 0.805, b, 0), &keys);  // 0.049

    }

    k4_a80(&keys);
    k4_a90(&keys);
    k4_a99(&keys);

    k16_a80(&keys);
    k16_a90(&keys);
    k16_a99(&keys);
}