use std::env;
use std::time::{Duration, Instant};

fn fibonacci(n: u32) -> u64 {
    match n {
        0 => 0,
        1 => 1,
        _ => fibonacci(n - 1) + fibonacci(n - 2),
    }
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
        let times_ms: Vec<f64> = self.times.iter().map(|d| d.as_nanos() as f64 / 1_000_000.0).collect();

        let mean = times_ms.iter().sum::<f64>() / times_ms.len() as f64;
        let min = times_ms.iter().fold(f64::INFINITY, |a, &b| a.min(b));
        let max = times_ms.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));

        let variance = times_ms.iter()
            .map(|&x| {
                let diff = x - mean;
                diff * diff
            })
            .sum::<f64>() / (times_ms.len() - 1) as f64;
        let std_dev = variance.sqrt();

        format!("{:.6},{:.6},{:.6},{:.6},{},{}",
            mean, std_dev, min, max, times_ms.len(), result_value)
    }
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() != 4 {
        eprintln!("Usage: {} <run_ms> <warmup_ms> <n>", args[0]);
        std::process::exit(1);
    }

    let run_ms: u64 = args[1].parse().expect("run_ms must be a number");
    let warmup_ms: u64 = args[2].parse().expect("warmup_ms must be a number");
    let n: u32 = args[3].parse().expect("n must be a number");

    let benchmark_func = || {
        let mut result = 0u64;
        for i in 1..n {
            result += fibonacci(i);
        }
        result
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
