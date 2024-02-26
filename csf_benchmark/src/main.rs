#![doc = include_str!("../README.md")]

use clap::{Args, Parser, Subcommand, ValueEnum};
use csf;
use csf::coding::BuildMinimumRedundancy;
use csf::{fp, GetSize};
use distribution::{Input, kv_dominated_lo_entropy};
use function::{CSFBuilder, PrintParams, CLS_HEADER, CFP_HEADER, FPGO_HEADER, FP_HEADER};
use ph::fmph::Bits;
use std::fs;
use std::fs::File;
use std::io::prelude::*;
use std::ops::RangeInclusive;

use crate::distribution::kv_dominated_lo;

mod distribution;
mod function;

#[allow(non_camel_case_types)]
#[derive(Args)]
pub struct FPConf {
    /// Relative level size as a percentage of the number of keys or automatically selected size (0 to use default value)
    #[arg(short = 'l', long, default_value_t = 0)]
    pub level_size: u16,

    /// Whether to use proportional level sizes instead of automatically calculated (optimal) ones
    #[arg(short = 'p', long, default_value_t = false)]
    pub level_size_proportional: bool,
}

#[allow(non_camel_case_types)]
#[derive(Args)]
pub struct FPGOConf {
    /// Number of bits to store seed of each group, *s*
    #[arg(short='s', long, value_parser = clap::value_parser!(u8).range(1..16))]
    pub bits_per_group_seed: Option<u8>,
    /// The size of each group, *b*
    #[arg(short='b', long, value_parser = clap::value_parser!(u8).range(1..63))]
    pub group_size: Option<u8>,
    /// Relative level size as a percentage of the number of keys or automatically selected size (0 to use default value)
    #[arg(short = 'l', long, default_value_t = 0)]
    pub level_size: u16,
    /// Whether to use proportional level sizes instead of automatically calculated (optimal) ones
    #[arg(short = 'p', long, default_value_t = false)]
    pub level_size_proportional: bool,
}

#[allow(non_camel_case_types)]
//#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
#[derive(Subcommand)]
pub enum Function {
    /// Based on Finger-Printing, with compressed values, all configurations
    CFPGO_all(FPConf),
    /// Based on Finger-Printing, with compressed values and group optimization
    CFPGO(FPGOConf),
    /// Based on Finger-Printing, with compressed values
    CFP(FPConf),
    /// Based on Finger-Printing
    FP(FPConf),
    /// Based on solving linear systems, with compressed values
    CLS,
    /// Based on solving linear systems
    LS
}

#[allow(non_camel_case_types)]
#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
pub enum Distribution {
    /// Possibly equal occurrence of each value.
    Equal,
    /// Dominated by a single value.
    Dominated,
}

impl Distribution {
    fn name(&self) -> &'static str {
        match self {
            Distribution::Equal => "equal",
            Distribution::Dominated => "dominated",
        }
    }
}

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
/// Static function benchmark.
pub struct Conf {
    /// Function to test
    #[command(subcommand)]
    pub function: Function,

    /// Distribution of the values
    #[arg(short='d', long, value_enum, default_value_t = Distribution::Equal)]
    pub distribution: Distribution,

    /// The number of random key-value pairs to use
    #[arg(short = 'n', long, default_value_t = 1024*1024)]
    pub keys_num: u32,

    // Number of foreign keys used to test the frequency of detection of non-contained keys
    //#[arg(short = 'f', long, default_value_t = 0)]
    //pub foreign_keys_num: usize,

    /// Save detailed results to CSV-like (but space separated) file
    #[arg(short = 's', long, default_value_t = false)]
    pub save_details: bool,

    /// Bits per fragment of each Huffman codeword
    #[arg(short = 'f', long)]
    pub bits_per_fragment: Option<u8>,

    /// Minimum entropy difference between two consecutive inputs
    #[arg(short = 'r', long, default_value_t = 0.02)]
    pub resolution: f64,

    /// Minimum input entropy to be considered (included)
    #[arg(long, default_value_t = f64::NEG_INFINITY)]
    pub from: f64,

    /// Maximum input entropy to be considered (excluded)
    #[arg(long, default_value_t = f64::INFINITY)]
    pub to: f64,
}

impl Conf {
    #[inline] pub fn bits_per_fragments(&self) -> RangeInclusive<u8> {
        if let Some(b) = self.bits_per_fragment { b..=b } else { 1..=8 }
    }
}

/// Calculate bits per entry for given number of entries and number of bytes occupied by them.
fn bits_per_entry(bytes: usize, entries: usize) -> f64 {
    (bytes * 8) as f64 / entries as f64
}

const BENCHMARK_HEADER: &'static str = "bits/entry levels/query";

/// Test given `csf` on given `input` and print results to standard output and (optionally) to the given `file`.
fn benchmark<CSF: CSFBuilder+PrintParams>(input: Input, csf: CSF, file: &mut Option<File>) {
    input.print_params_to(file);
    csf.print_params(file);
    let map = csf.new(
        input.keys.as_ref(),
        input.values.as_ref(),
        &input.frequencies,
    );
    let mut levels_searched = 0u64;
    for (k, expected_v) in input.keys.iter().copied().zip(input.values.iter().copied()) {
        let v = CSF::value(&map, k, &mut levels_searched);
        if let Some(v) = v {
            if v != expected_v {
                eprintln!("error while checking integrity for key {k}, its value is {v}, but should be {expected_v}");
            }
        } else {
            eprintln!("error while checking integrity for key {k}, its value is None, but should be {expected_v}");
        }
    }
    let bits_per_entry = bits_per_entry(map.size_bytes(), input.keys.len());
    let levels_per_query = levels_searched as f64 / input.keys.len() as f64;
    let overhead = bits_per_entry-input.entropy;
    println!("{:.2} (entropy) + {:.2} ({:.0}%) = {:.2} bits/kv {:.2} levels/query", input.entropy, overhead, 100.0*overhead/input.entropy, bits_per_entry, levels_per_query);
    if let Some(ref mut f) = file {
        writeln!(f, " {} {}", bits_per_entry, levels_per_query).unwrap();
    }
}

#[inline] fn rounded_div(a: u32, b: u32) -> u32 { (a+b/2)/b }

fn benchmark_all_functions<CSF, CSFIter, GetFunctions>(conf: &Conf, file: &mut Option<File>, functions: GetFunctions)
where GetFunctions: Fn() -> CSFIter, CSFIter: IntoIterator<Item = CSF>, CSF: CSFBuilder+PrintParams
{
    let has_multiple_functions = functions().into_iter().nth(1).is_some();
    match conf.distribution {
        Distribution::Equal => {
            let mut prev_entropy = -1.0f64;
            for different_values in 2..=256 {
                let each_value_len = rounded_div(conf.keys_num, different_values);
                for last_count in 1..=each_value_len {
                    let total_len = (different_values - 1) * each_value_len + last_count;
                    let entropy = kv_dominated_lo_entropy(total_len, different_values, each_value_len);
                    if entropy < conf.from { continue; }
                    if entropy >= conf.to { return; }
                    if (different_values == 256 && last_count == each_value_len) || entropy - prev_entropy >= conf.resolution {
                        print!(
                            "{}*{}+{}={} key-values: ",
                            different_values-1, each_value_len, last_count, total_len
                        );
                        if has_multiple_functions { println!(); }
                        prev_entropy = entropy;
                        for csf in functions() {
                            if has_multiple_functions { print!("\t"); }
                            let (k, v) = kv_dominated_lo(total_len, different_values, each_value_len);
                            benchmark((k, v, entropy).into(), csf, file)
                        }
                    }
                }
            }
        },
        Distribution::Dominated => {
            let different_values = 256;
            /*let len = 1_000_000;
            let different_values = 200;*/
            let mut prev_entropy = -1.0f64;
            for lo_count in 1..=conf.keys_num / different_values {
                let entropy = kv_dominated_lo_entropy(conf.keys_num, different_values, lo_count);
                if entropy < conf.from { continue; }
                if entropy >= conf.to { return; }
                if lo_count == conf.keys_num / different_values || entropy - prev_entropy >= conf.resolution {
                    print!("{} {}: ", lo_count, entropy);
                    if has_multiple_functions { println!(); }
                    prev_entropy = entropy;
                    for csf in functions() {
                        if has_multiple_functions { print!("\t"); }
                        let (k, v) = kv_dominated_lo(conf.keys_num, different_values, lo_count);
                        benchmark((k, v, entropy).into(), csf, file)
                    }
                }
            }
        },
    }
}

fn file(conf: &Conf, function_name: &str, function_header: &str) -> Option<File> {
    conf.save_details.then(|| {
        if let Err(e) = fs::create_dir("csf_benchmark_results") {
            println!("create_dir csf_benchmark_results: {}", e);
        }
        let file_name = format!("csf_benchmark_results/{}_{}.csv", function_name, conf.distribution.name());
        let file_already_existed = std::path::Path::new(&file_name).exists();
        let mut file = fs::OpenOptions::new().append(true).create(true).open(&file_name).unwrap();
        if !file_already_existed {
            if function_header.is_empty() {
                writeln!(file, "{} {}", Input::HEADER, BENCHMARK_HEADER).unwrap();
            } else {
                writeln!(file, "{} {} {}", Input::HEADER, function_header, BENCHMARK_HEADER).unwrap();
            }
        }
        file
    })
}

/*
fn gen_dominated() {
    let mut meta_file = File::create("dominated_params.csv").unwrap();
    writeln!(meta_file, "{}", "i,total,lo,dominate").unwrap();
    for i in 0..=8 {
        // to_file(&format!("optimal_dominated_{}", i), kv_dominated_lo(1_024*1_024, 8, (1<<i)*(1_024/8)*i/10), OptimalLevelSize::default());
        // to_file(&format!("prop90_dominated_{}", i), kv_dominated_lo(1_024*1_024, 8, (1<<i)*(1_024/8)*i/10), ProportionalLevelSize::default());
        // to_file(&format!("optimal_dominated_{}", i), kv_dominated_lo(1_600_008, 8, 1+i*i*1_600_000/8/81), OptimalLevelSize::default());
        // to_file(&format!("prop90_dominated_{}", i), kv_dominated_lo(1_600_008, 8, 1+i*i*1_600_000/8/81), ProportionalLevelSize::default());
        // to_file(&format!("optimal_dominated_{}", i), kv_dominated_lo((1<<21), 8, (1<<(9+i))), OptimalLevelSize::default());
        // to_file(&format!("prop90_dominated_{}", i), kv_dominated_lo((1<<21), 8, (1<<(9+i))), ProportionalLevelSize::default());

        /*let lo_count = ((i-4i32).pow(3)+65)*1000;
        to_file(&format!("optimal_dominated_{}", i), kv_dominated_lo(1032000, 8, lo_count as u32), OptimalLevelSize::default());
        to_file(&format!("prop90_dominated_{}", i), kv_dominated_lo(1032000, 8, lo_count as u32), ProportionalLevelSize::default());*/
        let total: u32 = 124800 * 8;
        let lo_count = if i == 0 {
            1
        } else {
            (((i - 3i32).pow(3) + 27 + i * 20) * 400) as u32
        };
        to_file(
            &format!("optimal_dominated_{}", i),
            kv_dominated_lo(total, 8, lo_count),
            fp::OptimalLevelSize::default(),
        );
        to_file(
            &format!("prop90_dominated_{}", i),
            kv_dominated_lo(total, 8, lo_count),
            fp::ProportionalLevelSize::default(),
        );
        writeln!(
            meta_file,
            "{},{},{},{}",
            i,
            total,
            lo_count,
            total - 7 * lo_count
        )
        .unwrap();
    }
}

fn gen_equals() {
    for i in 2..=16 {
        to_file(
            &format!("optimal_equals_720720_{}", i),
            kv_equals(720720, i),
            fp::OptimalLevelSize::default(),
        );
        to_file(
            &format!("prop90_equals_720720_{}", i),
            kv_equals(720720, i),
            fp::ProportionalLevelSize::default(),
        );
    }
}

fn gen_bbmap_equals_plots_vs_size() {
    println!("bbmap speed equals plot");
    let filename_speed = "plot_data/seq_speed_equals_plot.dat";
    let filename_size = "plot_data/seq_size_equals_plot.dat";
    let mut file_speed = File::create(filename_speed).unwrap();
    let mut file_size = File::create(filename_size).unwrap();
    writeln!(
        file_speed,
        "size prop90_b1 optim_b1 prop90_b2 optim_b2 prop90_b3 optim_b3"
    )
    .unwrap();
    writeln!(
        file_size,
        "size prop90_b1 optim_b1 prop90_b2 optim_b2 prop90_b3 optim_b3"
    )
    .unwrap();
    for s in 1..=100 {
        //1..=100
        let size = 100000 * s; // 100000 * s
        write!(file_speed, "{} ", size).unwrap();
        write!(file_size, "{} ", size).unwrap();
        println!("{}/100\t{}", s, size);
        for b in 1..=3 {
            let (k, v) = kv_equals(size, 8);
            let r = BenchmarkResult::get_bbmap((&k, &v), b, fp::ProportionalLevelSize::default());
            write!(file_speed, "{} ", r.levels_per_query).unwrap();
            write!(file_size, "{} ", r.bits_per_entry).unwrap();
            let r = BenchmarkResult::get_bbmap((&k, &v), b, fp::OptimalLevelSize::default());
            write!(
                file_speed,
                "{}{}",
                r.levels_per_query,
                if b == 3 { '\n' } else { ' ' }
            )
            .unwrap();
            write!(
                file_size,
                "{}{}",
                r.bits_per_entry,
                if b == 3 { '\n' } else { ' ' }
            )
            .unwrap();
        }
    }
}

fn gen_bbmap2_equals_plots_vs_size() {
    println!("bbmap2 speed equals plot");
    let filename_speed = "plot_data/bbmap2_speed_equals_plot.dat";
    let filename_size = "plot_data/bbmap2_size_equals_plot.dat";
    let mut file_speed = File::create(filename_speed).unwrap();
    let mut file_size = File::create(filename_size).unwrap();
    writeln!(
        file_speed,
        "size optim_s4_gl5_b1 optim_s4_gl5_b2 optim_s4_gl5_b3"
    )
    .unwrap();
    writeln!(
        file_size,
        "size optim_s4_gl5_b1 optim_s4_gl5_b2 optim_s4_gl5_b3"
    )
    .unwrap();
    for s in 1..=100 {
        //1..=100
        let size = 100000 * s; // 100000 * s
        write!(file_speed, "{} ", size).unwrap();
        write!(file_size, "{} ", size).unwrap();
        println!("{}/100\t{}", s, size);
        for b in 1..=3 {
            let (k, v) = kv_equals(size, 8);
            let r = BenchmarkResult::get_bbmap2((&k, &v), b, 4, 5);
            write!(
                file_speed,
                "{}{}",
                r.levels_per_query,
                if b == 3 { '\n' } else { ' ' }
            )
            .unwrap();
            write!(
                file_size,
                "{}{}",
                r.bits_per_entry,
                if b == 3 { '\n' } else { ' ' }
            )
            .unwrap();
        }
    }
}

fn gen_bdzh_equals_plots_vs_size() {
    println!("bdzh speed equals plot");
    let mut file_speed = File::create("plot_data/bdzh_speed_equals_plot.dat").unwrap();
    let mut file_size = File::create("plot_data/bdzh_size_equals_plot.dat").unwrap();
    writeln!(file_speed, "size b1 b2 b3").unwrap();
    writeln!(file_size, "size b1 b2 b3").unwrap();
    for s in 1..=100 {
        //1..=100
        let size = 100000 * s; // 100000 * s
        write!(file_speed, "{} ", size).unwrap();
        write!(file_size, "{} ", size).unwrap();
        println!("{}/100\t{}", s, size);
        for b in 1..=3 {
            let (k, v) = kv_equals(size, 8);
            let r = BenchmarkResult::get_bdzh((&k, &v), b);
            write!(
                file_speed,
                "{}{}",
                r.levels_per_query,
                if b == 3 { '\n' } else { ' ' }
            )
            .unwrap();
            write!(
                file_size,
                "{}{}",
                r.bits_per_entry,
                if b == 3 { '\n' } else { ' ' }
            )
            .unwrap();
        }
    }
}

fn save_optimal_b_plot(
    (file_b, file_speed, file_size): &mut (File, File, File),
    r: &[BenchmarkResult],
) {
    let mut best_i = 0usize;
    for i in 1..r.len() {
        if r[i].bits_per_entry < r[best_i].bits_per_entry {
            best_i = i;
        }
    }
    let calculated_i = 1f64.max(r[0].entropy - 0.1).ceil() as usize - 1;
    let err = r[calculated_i].bits_per_entry - r[best_i].bits_per_entry;
    writeln!(
        file_b,
        "{} {} {} {} {}",
        r[0].entropy,
        best_i + 1,
        calculated_i + 1,
        err,
        err / r[best_i].bits_per_entry
    )
    .unwrap();
    write!(file_speed, "{}", r[0].entropy).unwrap();
    write!(file_size, "{}", r[0].entropy).unwrap();
    for i in 0..r.len() {
        write!(file_speed, " {}", r[i].levels_per_query).unwrap();
        write!(file_size, " {}", r[i].bits_per_entry).unwrap();
    }
    writeln!(
        file_speed,
        " {} {}",
        r[best_i].levels_per_query, r[calculated_i].levels_per_query
    )
    .unwrap();
    writeln!(
        file_size,
        " {} {}",
        r[best_i].bits_per_entry, r[calculated_i].bits_per_entry
    )
    .unwrap();
}

fn gen_bbmap_optimal_b_plot<LSC: fp::LevelSizeChooser + Copy>(
    files: &mut (File, File, File),
    (keys, values): (&Box<[u32]>, &Box<[u32]>),
    level_size_chooser: LSC,
) {
    let r: Vec<_> = (1..=8)
        .map(|b| BenchmarkResult::get_bbmap((&keys, &values), b, level_size_chooser))
        .collect();
    save_optimal_b_plot(files, &r);
}

fn gen_bbmap2_optimal_b_plot(
    files: &mut (File, File, File),
    (keys, values): (&Box<[u32]>, &Box<[u32]>),
    bits_per_group_seed: u8,
    bits_per_group_log2: u8,
) {
    let r: Vec<_> = (1..=8)
        .map(|b| {
            BenchmarkResult::get_bbmap2(
                (&keys, &values),
                b,
                bits_per_group_seed,
                bits_per_group_log2,
            )
        })
        .collect();
    save_optimal_b_plot(files, &r);
}

fn gen_bdzh_optimal_b_plot(
    files: &mut (File, File, File),
    (keys, values): (&Box<[u32]>, &Box<[u32]>),
) {
    let r: Vec<_> = (1..=8)
        .map(|b| BenchmarkResult::get_bdzh((&keys, &values), b))
        .collect();
    save_optimal_b_plot(files, &r);
}

fn create_plot_files(alg_name: &str, level_size_name: &str, dist_name: &str) -> (File, File, File) {
    println!(
        "creating plot files for: {} {} {}",
        alg_name, level_size_name, dist_name
    );
    let mut file_b = File::create(format!(
        "plot_data/{}_optimalb_{}_{}.dat",
        alg_name, level_size_name, dist_name
    ))
    .unwrap();
    let mut file_speed = File::create(format!(
        "plot_data/{}_speed_{}_{}.dat",
        alg_name, level_size_name, dist_name
    ))
    .unwrap();
    let mut file_size = File::create(format!(
        "plot_data/{}_size_{}_{}.dat",
        alg_name, level_size_name, dist_name
    ))
    .unwrap();
    writeln!(file_b, "entropy smaller_b calc_b err rel_err").unwrap();
    writeln!(file_speed, "entropy b1 b2 b3 b4 b5 b6 b7 b8 smaller calc").unwrap();
    writeln!(file_size, "entropy b1 b2 b3 b4 b5 b6 b7 b8 smaller calc").unwrap();
    (file_b, file_speed, file_size)
}*/

fn fpgo(file: &mut Option<File>, conf: &Conf, bits_per_seed: u8, bits_per_group: u8, fpconf: &FPGOConf) {
    let b_range = conf.bits_per_fragments();
    let goconf = fp::GOConf::bps_bpg(Bits(bits_per_seed), Bits(bits_per_group));
    match (fpconf.level_size, fpconf.level_size_proportional) {
        (0, true) => benchmark_all_functions(&conf, file, || {
            b_range.clone().map(|b| fp::GOCMapConf::groups_lsize_coding(goconf.clone(), fp::ProportionalLevelSize::default(), BuildMinimumRedundancy{ bits_per_fragment: b }))
        }),
        (level_size, true) => benchmark_all_functions(&conf, file, || {
            b_range.clone().map(|b| fp::GOCMapConf::groups_lsize_coding(goconf.clone(), fp::ProportionalLevelSize::with_percent(level_size), BuildMinimumRedundancy{ bits_per_fragment: b }))
        }),
        (0, false) => benchmark_all_functions(&conf, file, || {
            b_range.clone().map(|b| fp::GOCMapConf::groups_coding(goconf.clone(), BuildMinimumRedundancy{ bits_per_fragment: b }))
        }),
        (level_size, false) => benchmark_all_functions(&conf, file, || {
            b_range.clone().map(|b| fp::GOCMapConf::groups_lsize_coding(goconf.clone(), fp::ResizedLevel::new(level_size, fp::OptimalLevelSize::default()), BuildMinimumRedundancy{ bits_per_fragment: b }))
        }),
    }
}

fn fpgo_all<L: fp::LevelSizeChooser+Copy>(conf: &Conf, level_size: L) 
where fp::GOCMapConf<BuildMinimumRedundancy, L, Bits, Bits>: PrintParams
{
    let mut file = file(&conf, "fpgo_all", FPGO_HEADER);
    let b_range = conf.bits_per_fragments();
    benchmark_all_functions(&conf, &mut file, || {
        b_range.clone().flat_map(|b| {
            (1u8..=8u8).flat_map(move |bits_per_seed| {
                (2u8..=62u8).map(move |bits_per_group| {
                    let goconf = fp::GOConf::bps_bpg(Bits(bits_per_seed), Bits(bits_per_group));
                    fp::GOCMapConf::groups_lsize_coding(goconf.clone(), level_size.clone(), BuildMinimumRedundancy{ bits_per_fragment: b })
                })
            })
        })
    });
}

fn main() {
    let conf: Conf = Conf::parse();
    match conf.function {
        Function::CFPGO_all(ref fpconf) => {
            match (fpconf.level_size, fpconf.level_size_proportional) {
                (0, true) => fpgo_all(&conf, fp::ProportionalLevelSize::default()),
                (level_size, true) => fpgo_all(&conf, fp::ProportionalLevelSize::with_percent(level_size)),
                (0, false) => fpgo_all(&conf, fp::OptimalLevelSize::default()),
                (level_size, false) => fpgo_all(&conf, fp::ResizedLevel::new(level_size, fp::OptimalLevelSize::default())),
            }
        }
        Function::CFPGO(ref fpconf) => {
            let mut file = file(&conf, "cfpgo", FPGO_HEADER);
            match (fpconf.bits_per_group_seed, fpconf.group_size) {
                (None, None) => {
                    for (bits_per_group_seed, bits_per_group) in [(1, 8), (2, 16), (4, 16), (8, 32)] {
                        fpgo(&mut file, &conf, bits_per_group_seed, bits_per_group, fpconf);
                    }
                },
                (Some(bits_per_group_seed), Some(bits_per_group)) => fpgo(&mut file, &conf, bits_per_group_seed, bits_per_group, fpconf),
                (Some(1), None) | (None, Some(8)) => fpgo(&mut file, &conf, 1, 8, fpconf),
                (Some(2), None) => fpgo(&mut file, &conf, 2, 16, fpconf),
                (Some(4), None) => fpgo(&mut file, &conf, 4, 16, fpconf),
                (None, Some(16)) => {
                    fpgo(&mut file, &conf, 2, 16, fpconf);
                    fpgo(&mut file, &conf, 4, 16, fpconf);
                }
                (Some(8), None) | (None, Some(32)) => fpgo(&mut file, &conf, 8, 32, fpconf),
                _ => eprintln!("Cannot deduce for which pairs of (bits per group seed, group size) calculate.")
            }
        },
        Function::CFP(ref fpconf) => {
            let mut file = file(&conf, "cfp", CFP_HEADER);
            let b_range = conf.bits_per_fragments();
            match (fpconf.level_size, fpconf.level_size_proportional) {
                (0, true) => benchmark_all_functions(&conf, &mut file, || {
                    b_range.clone().map(|b| fp::CMapConf::lsize_coding(fp::ProportionalLevelSize::default(), BuildMinimumRedundancy{ bits_per_fragment: b }))
                }),
                (level_size, true) => benchmark_all_functions(&conf, &mut file, || {
                    b_range.clone().map(|b| fp::CMapConf::lsize_coding(fp::ProportionalLevelSize::with_percent(level_size), BuildMinimumRedundancy{ bits_per_fragment: b }))
                }),
                (0, false) => benchmark_all_functions(&conf, &mut file, || {
                    b_range.clone().map(|b| fp::CMapConf::coding(BuildMinimumRedundancy{ bits_per_fragment: b }))
                }),
                (level_size, false) => benchmark_all_functions(&conf, &mut file, || {
                    b_range.clone().map(|b| fp::CMapConf::lsize_coding(fp::ResizedLevel::new(level_size, fp::OptimalLevelSize::default()), BuildMinimumRedundancy{ bits_per_fragment: b }))
                }),
            }
        },
        Function::FP(ref fpconf) => {
            let mut file = file(&conf, "fp", FP_HEADER);
            match (fpconf.level_size, fpconf.level_size_proportional) {
                (0, true) => benchmark_all_functions(&conf, &mut file, || {
                    [fp::MapConf::lsize(fp::ProportionalLevelSize::default())]
                }),
                (level_size, true) => benchmark_all_functions(&conf, &mut file, || {
                    [fp::MapConf::lsize(fp::ProportionalLevelSize::with_percent(level_size))]
                }),
                (0, false) => benchmark_all_functions(&conf, &mut file, || {
                    [fp::MapConf::default()]
                }),
                (level_size, false) => benchmark_all_functions(&conf, &mut file, || {
                    [fp::MapConf::lsize(fp::ResizedLevel::new(level_size, fp::OptimalLevelSize::default()))]
                }),
            }
        },
        Function::CLS => {
            let mut file = file(&conf, "cls", CLS_HEADER);
            let b_range = conf.bits_per_fragments();
            benchmark_all_functions(&conf, &mut file, || {
                b_range.clone().map(|b| function::BuildLSCMap(b))
            });
        },
        Function::LS => {
            let mut file = file(&conf, "ls", "");
            benchmark_all_functions(&conf, &mut file, || { [function::BuildLSMap] });
        },
    }
}
