use std::env;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::time::{Duration, Instant};

fn levenshtein_distance(str1: &str, str2: &str) -> usize {
    let (str1, str2) = if str2.len() < str1.len() {
        (str2, str1)
    } else {
        (str1, str2)
    };

    let m = str1.len();
    let n = str2.len();

    let mut prev = vec![0; m + 1];
    let mut curr = vec![0; m + 1];

    for i in 0..=m {
        prev[i] = i;
    }

    for (i, c2) in str2.chars().enumerate() {
        curr[0] = i + 1;
        for (j, c1) in str1.chars().enumerate() {
            let cost = if c1 == c2 { 0 } else { 1 };
            curr[j + 1] = (prev[j + 1] + 1)
                .min(curr[j] + 1)
                .min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[m]
}

fn calculate_distances(words: &[String]) -> Vec<usize> {
    let mut results = Vec::new();
    for i in 0..words.len() - 1 {
        for j in i + 1..words.len() {
            let distance = levenshtein_distance(&words[i], &words[j]);
            results.push(distance);
        }
    }
    results
}

fn bench<F, T>(run_ms: u64, mut func: F) -> BenchResult<T>
where
    F: FnMut() -> T,
{
    let mut times = Vec::new();
    let mut result = None;
    let run_duration = Duration::from_millis(run_ms);
    let mut elapsed_total = Duration::ZERO;

    while elapsed_total < run_duration {
        let start = Instant::now();
        let res = func();
        let elapsed = start.elapsed();

        elapsed_total += elapsed;
        times.push(elapsed);
        result = Some(res);

        // Print progress dots
        if run_ms > 1 && elapsed_total.as_secs() > (elapsed_total - elapsed).as_secs() {
            eprint!(".");
        }
    }

    if run_ms > 1 {
        eprintln!();
    }

    BenchResult { times, result }
}

struct BenchResult<T> {
    times: Vec<Duration>,
    result: Option<T>,
}

impl<T> BenchResult<T> {
    fn format_output(&self, result_value: impl std::fmt::Display) -> String {
        let times_ms: Vec<f64> = self
            .times
            .iter()
            .map(|d| d.as_nanos() as f64 / 1_000_000.0)
            .collect();

        let mean = times_ms.iter().sum::<f64>() / times_ms.len() as f64;
        let min = times_ms.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max = times_ms.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));

        let variance = times_ms
            .iter()
            .map(|&x| {
                let diff = x - mean;
                diff * diff
            })
            .sum::<f64>()
            / (times_ms.len() - 1) as f64;
        let std_dev = variance.sqrt();

        format!(
            "{:.6},{:.6},{:.6},{:.6},{},{}",
            mean,
            std_dev,
            min,
            max,
            times_ms.len(),
            result_value
        )
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 4 {
        eprintln!("Usage: {} <run_ms> <warmup_ms> <input_file>", args[0]);
        std::process::exit(1);
    }

    let run_ms: u64 = args[1].parse().expect("run_ms must be a number");
    let warmup_ms: u64 = args[2].parse().expect("warmup_ms must be a number");
    let input_file = &args[3];

    // Read words from file
    let file = File::open(input_file).expect("Failed to open input file");
    let reader = BufReader::new(file);
    let words: Vec<String> = reader
        .lines()
        .filter_map(|line| line.ok())
        .filter(|line| !line.trim().is_empty())
        .collect();

    let benchmark_func = || {
        let distances = calculate_distances(&words);
        distances.iter().sum::<usize>()
    };

    // Warmup
    if warmup_ms > 0 {
        bench(warmup_ms, &benchmark_func);
    }

    // Actual benchmark
    if run_ms > 0 {
        let bench_result = bench(run_ms, &benchmark_func);
        let output = bench_result.format_output(bench_result.result.unwrap());
        println!("{}\n", output);
    }
}
