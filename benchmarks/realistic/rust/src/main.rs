//! Mixed "realistic" workload benchmark, matching
//! benchmarks/realistic/realistic-array.lisp's run-arr-once / bench-arr:
//!
//!   1. array processing (n=300): square 1..n, sum evens, +2n
//!   2. key/value lookup: build {1:3,...,250:750}, 1500 lookups, sum
//!   3. naive recursive fibonacci(23)
//!   4. tail-recursive sum 1..300
//!   5. ackermann(2, 50)
//!   6. record processing (n=300): categorize into 3 buckets, totals+count+max
//!
//! run_once sums all six; the benchmark repeats run_once 10 times and sums
//! those into a checksum.

use std::time::Instant;

const ARR_N: i64 = 300;
const KV_N: i64 = 250;
const KV_REPS: i64 = 1500;
const REC_N: i64 = 300;

fn bench_array_lists(n: i64) -> i64 {
    let arr: Vec<i64> = (1..=n).map(|i| i * i).collect();
    let total: i64 = arr.iter().filter(|&&v| v % 2 == 0).sum();
    total + 2 * n
}

fn bench_kv_lookup(n: i64, reps: i64) -> i64 {
    let mut h = vec![0i64; (n + 1) as usize];
    for i in 1..=n {
        h[i as usize] = i * 3;
    }
    let mut total = 0i64;
    for i in 0..reps {
        let key = 1 + (i % n);
        total += h[key as usize];
    }
    total
}

fn fib(n: i64) -> i64 {
    if n < 2 {
        n
    } else {
        fib(n - 1) + fib(n - 2)
    }
}

fn tsum(n: i64, acc: i64) -> i64 {
    if n == 0 {
        acc
    } else {
        tsum(n - 1, acc + n)
    }
}

fn ackermann(m: i64, n: i64) -> i64 {
    if m == 0 {
        n + 1
    } else if n == 0 {
        ackermann(m - 1, 1)
    } else {
        ackermann(m - 1, ackermann(m, n - 1))
    }
}

fn process_records(n: i64) -> i64 {
    let mut t1 = 0i64;
    let mut t2 = 0i64;
    let mut t3 = 0i64;
    let mut mx = 0i64;
    for i in 0..n {
        let amt = ((i + 1) * 7) % 100;
        let cat = 1 + ((i + 1) % 3);
        if amt > mx {
            mx = amt;
        }
        match cat {
            1 => t1 += amt,
            2 => t2 += amt,
            _ => t3 += amt,
        }
    }
    t1 + t2 + t3 + n + mx
}

fn run_once() -> i64 {
    bench_array_lists(ARR_N)
        + bench_kv_lookup(KV_N, KV_REPS)
        + fib(23)
        + tsum(300, 0)
        + ackermann(2, 50)
        + process_records(REC_N)
}

fn main() {
    let reps = 10;
    let mut checksum = 0i64;

    let start = Instant::now();
    for _ in 0..reps {
        checksum += run_once();
    }
    let elapsed = start.elapsed();
    let ms = elapsed.as_secs_f64() * 1000.0;

    println!("result={} time={:.1} ms ({} reps)", checksum, ms, reps);
}
