//! Levenshtein distance benchmark: two-row DP, "kitten" -> "sitting".

use std::hint::black_box;
use std::time::Instant;

fn levenshtein(a: &[u8], b: &[u8]) -> usize {
    let (a, b) = if b.len() < a.len() { (b, a) } else { (a, b) };

    let mut prev: Vec<usize> = (0..=a.len()).collect();
    let mut curr: Vec<usize> = vec![0; a.len() + 1];

    for (i, &cb) in b.iter().enumerate() {
        curr[0] = i + 1;
        for (j, &ca) in a.iter().enumerate() {
            let cost = if ca == cb { 0 } else { 1 };
            curr[j + 1] = (prev[j + 1] + 1).min(curr[j] + 1).min(prev[j] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    prev[a.len()]
}

fn main() {
    let s1 = black_box(b"kitten".as_slice());
    let s2 = black_box(b"sitting".as_slice());
    let iters = 10_000;
    let mut result = 0usize;

    let start = Instant::now();
    for _ in 0..iters {
        result = black_box(levenshtein(black_box(s1), black_box(s2)));
    }
    let elapsed = start.elapsed();
    let ms = elapsed.as_secs_f64() * 1000.0;

    println!("result={} time={:.1} ms ({} iters)", result, ms, iters);
}
