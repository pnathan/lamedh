use lamedh::environment::Environment;
use lamedh::{LispVal, eval_str, evaluator, reader};
use std::env;
use std::time::{Duration, Instant};

struct BenchResult {
    times: Vec<Duration>,
    result: LispVal,
}

fn bench<F>(run_ms: u64, mut func: F) -> BenchResult
where
    F: FnMut() -> LispVal,
{
    let mut times = Vec::new();
    let mut result = LispVal::Nil;
    let run_duration = Duration::from_millis(run_ms);
    let mut elapsed_total = Duration::ZERO;

    while elapsed_total < run_duration {
        let start = Instant::now();
        result = func();
        let elapsed = start.elapsed();
        elapsed_total += elapsed;
        times.push(elapsed);
    }

    BenchResult { times, result }
}

fn format_bench(data: &BenchResult) -> String {
    let times_ms: Vec<f64> = data
        .times
        .iter()
        .map(|d| d.as_nanos() as f64 / 1_000_000.0)
        .collect();
    let mean = times_ms.iter().sum::<f64>() / times_ms.len() as f64;
    let min = times_ms.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let max = times_ms.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    let std_dev = if times_ms.len() < 2 {
        0.0
    } else {
        let variance = times_ms
            .iter()
            .map(|&x| {
                let diff = x - mean;
                diff * diff
            })
            .sum::<f64>()
            / (times_ms.len() - 1) as f64;
        variance.sqrt()
    };

    format!(
        "{mean:.6},{std_dev:.6},{min:.6},{max:.6},{},{}",
        times_ms.len(),
        lamedh::printer::print(&data.result)
    )
}

fn run() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 4 && args.len() != 5 {
        eprintln!("Usage: {} <run_ms> <warmup_ms> <n> [plain|opt]", args[0]);
        std::process::exit(1);
    }

    let run_ms: u64 = args[1].parse().expect("run_ms must be a number");
    let warmup_ms: u64 = args[2].parse().expect("warmup_ms must be a number");
    let n: i64 = args[3].parse().expect("n must be a number");
    let optimized = args.get(4).is_some_and(|mode| mode == "opt");
    let definer = if optimized {
        "deffun-typed-opt"
    } else {
        "deffun-typed"
    };

    let env = Environment::with_stdlib();
    let definitions = format!(
        r#"
        (progn
          ({definer} (fib int64) ((n int64))
            (if (< n 2)
                n
                (+ (fib (- n 1)) (fib (- n 2)))))

          ({definer} (fib-sum-from int64) ((i int64) (n int64) (acc int64))
            (if (< i n)
                (fib-sum-from (+ i 1) n (+ acc (fib i)))
                acc))

          ({definer} (fib-sum int64) ((n int64))
            (fib-sum-from 1 n 0)))
        "#
    );
    eval_str(&definitions, &env)
    .expect("typed Fibonacci definitions should load");

    let call = reader::read(&format!("(fib-sum {n})"), &env).expect("call form should parse");
    let mut run_call = || evaluator::eval(&call, &env).expect("warm typed call should succeed");

    if warmup_ms > 0 {
        bench(warmup_ms, &mut run_call);
    }
    if run_ms > 0 {
        let result = bench(run_ms, &mut run_call);
        println!("{}", format_bench(&result));
    }
}

fn main() {
    lamedh::with_large_stack(run);
}
