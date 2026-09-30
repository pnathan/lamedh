//! #525: the native backend lowers `array-div!`/`array-scale!`/`array-fma!`/
//! `array-neg!` (`Core::ArrayOp`) to a 2-lane Cranelift SIMD loop plus a
//! scalar tail (`src/jit/native.rs::Emitter::emit_array_op`) instead of
//! calling the shared scalar reference (`src/jit/runtime.rs::array_op`). The
//! interpreting tiers still call that reference, so every tier must agree
//! **bit-for-bit** with it and with a Rust model of the contract: int64
//! wraps, float64 rounds per IEEE 754, `array-fma!` is fused
//! (`f64::mul_add`, one rounding).
//!
//! `Value`'s `PartialEq` treats `-0.0 == 0.0` and `NaN != NaN`, so these
//! tests compare raw bit patterns.

use lamedh::environment::Environment;
use lamedh::jit::{Jit, Value};
use lamedh::reader::read;

const FDIV: &str = "(defun-typed (fdiv! (array float64)) ((o (array float64)) (a (array float64)) (b (array float64))) (array-div! o a b))";
const FSCALE: &str = "(defun-typed (fscale! (array float64)) ((o (array float64)) (a (array float64)) (s float64)) (array-scale! o a s))";
const ISCALE: &str = "(defun-typed (iscale! (array int64)) ((o (array int64)) (a (array int64)) (s int64)) (array-scale! o a s))";
const FFMA: &str = "(defun-typed (ffma! (array float64)) ((o (array float64)) (a (array float64)) (b (array float64)) (c (array float64))) (array-fma! o a b c))";
const IFMA: &str = "(defun-typed (ifma! (array int64)) ((o (array int64)) (a (array int64)) (b (array int64)) (c (array int64))) (array-fma! o a b c))";
const FNEG: &str = "(defun-typed (fneg! (array float64)) ((o (array float64)) (a (array float64))) (array-neg! o a))";
const INEG: &str =
    "(defun-typed (ineg! (array int64)) ((o (array int64)) (a (array int64))) (array-neg! o a))";
/// `out` aliases both inputs.
const FFMA_ALIAS: &str = "(defun-typed (ffma-alias! (array float64)) ((a (array float64)) (c (array float64))) (array-fma! a a a c))";

fn jit_with(defs: &[&str]) -> Jit {
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    for src in defs {
        let form = read(src, &env).unwrap_or_else(|e| panic!("read `{src}`: {e}"));
        j.define(&form)
            .unwrap_or_else(|e| panic!("define `{src}`: {e}"));
    }
    j
}

fn ints(xs: &[i64]) -> Value {
    Value::Array(xs.iter().map(|x| Value::Int(*x)).collect())
}
fn floats(xs: &[f64]) -> Value {
    Value::Array(xs.iter().map(|x| Value::Float(*x)).collect())
}

/// The raw 64-bit words of an int64/float64 array.
fn bits(v: &Value) -> Vec<u64> {
    match v {
        Value::Array(xs) => xs
            .iter()
            .map(|x| match x {
                Value::Int(i) => *i as u64,
                Value::Float(f) => f.to_bits(),
                other => panic!("not a numeric element: {other:?}"),
            })
            .collect(),
        other => panic!("not an array: {other:?}"),
    }
}

/// Run `name(args)` compiled (native under `--features jit`, the closure
/// tree otherwise), through the typed-core interpreter, and traced, and
/// assert each tier's return value and written-back `out` (argument 0) have
/// exactly the bits `want`.
fn assert_tiers_bits(j: &Jit, name: &str, args: &[Value], want: &[u64]) {
    j.compile_all();
    let (rv, upd, _) = j.call_with_array_writeback(name, args).unwrap();
    assert_eq!(bits(&rv), want, "{name} compiled: return");
    assert_eq!(bits(upd[0].as_ref().unwrap()), want, "{name} compiled: out");

    j.deoptimize_all();
    let (rv, upd, _) = j.call_with_array_writeback(name, args).unwrap();
    assert_eq!(bits(&rv), want, "{name} interpreted: return");
    assert_eq!(
        bits(upd[0].as_ref().unwrap()),
        want,
        "{name} interpreted: out"
    );

    let (rv, _log) = j.trace_call(name, args).unwrap();
    assert_eq!(bits(&rv), want, "{name} traced: return");
}

/// Float edge values: signed zeros, infinities, NaN, subnormals, extremes.
const EDGE: &[f64] = &[
    0.0,
    -0.0,
    1.0,
    -1.5,
    0.1,
    f64::INFINITY,
    f64::NEG_INFINITY,
    f64::NAN,
    f64::MIN_POSITIVE,
    5e-324,
    -5e-324,
    f64::MAX,
    f64::MIN,
    1e308,
    3.0,
];

/// Deterministic pseudo-random floats spanning many exponents.
fn lcg_floats(n: usize, seed: u64) -> Vec<f64> {
    let mut x = seed;
    (0..n)
        .map(|_| {
            x = x
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let m = (x >> 11) as f64 / (1u64 << 53) as f64 - 0.5;
            let e = ((x >> 3) % 80) as i32 - 40;
            m * 2f64.powi(e)
        })
        .collect()
}

fn lcg_ints(n: usize, seed: u64) -> Vec<i64> {
    let mut x = seed;
    (0..n)
        .map(|_| {
            x = x
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            x as i64
        })
        .collect()
}

/// Every length 0..=9 (empty, tail-only, vector-only, vector + tail) plus a
/// long pseudo-random run, for each op.
const LENS: &[usize] = &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 1001];

fn fcase(n: usize, seed: u64) -> Vec<f64> {
    if n <= EDGE.len() {
        // Rotate the edge values so every lane position sees each of them.
        (0..n)
            .map(|i| EDGE[(i + seed as usize) % EDGE.len()])
            .collect()
    } else {
        lcg_floats(n, seed)
    }
}

#[test]
fn fdiv_all_tiers_bit_identical() {
    let j = jit_with(&[FDIV]);
    for &n in LENS {
        for seed in 0..EDGE.len() as u64 {
            let (a, b) = (fcase(n, seed), fcase(n, seed * 7 + 3));
            let want: Vec<u64> = a.iter().zip(&b).map(|(x, y)| (x / y).to_bits()).collect();
            let o = floats(&vec![9.0; n]);
            assert_tiers_bits(&j, "fdiv!", &[o, floats(&a), floats(&b)], &want);
        }
    }
}

#[test]
fn fscale_all_tiers_bit_identical() {
    let j = jit_with(&[FSCALE]);
    for &n in LENS {
        for &s in EDGE {
            let a = fcase(n, 5);
            let want: Vec<u64> = a.iter().map(|x| (x * s).to_bits()).collect();
            let o = floats(&vec![9.0; n]);
            assert_tiers_bits(&j, "fscale!", &[o, floats(&a), Value::Float(s)], &want);
        }
    }
}

#[test]
fn iscale_all_tiers_wrap() {
    let j = jit_with(&[ISCALE]);
    for &n in LENS {
        for s in [0, 1, -1, 3, i64::MAX, i64::MIN] {
            let a = lcg_ints(n, 11);
            let want: Vec<u64> = a.iter().map(|x| x.wrapping_mul(s) as u64).collect();
            let o = ints(&vec![7; n]);
            assert_tiers_bits(&j, "iscale!", &[o, ints(&a), Value::Int(s)], &want);
        }
    }
}

#[test]
fn ffma_all_tiers_fused_and_bit_identical() {
    let j = jit_with(&[FFMA]);
    for &n in LENS {
        for seed in 0..EDGE.len() as u64 {
            let (a, b, c) = (fcase(n, seed), fcase(n, seed + 4), fcase(n, seed + 9));
            let want: Vec<u64> = (0..n).map(|i| a[i].mul_add(b[i], c[i]).to_bits()).collect();
            let o = floats(&vec![9.0; n]);
            let args = [o, floats(&a), floats(&b), floats(&c)];
            assert_tiers_bits(&j, "ffma!", &args, &want);
        }
    }
}

#[test]
fn ffma_is_fused_in_the_vector_body_and_the_tail() {
    // 0.1 * 10.0 - 1.0 is 0.0 unfused but 2^-54 fused (0.1 is inexact); at
    // length 3 elements 0 and 1 run in the vector body, element 2 in the tail.
    let j = jit_with(&[FFMA]);
    let fused = 0.1f64.mul_add(10.0, -1.0);
    assert_ne!(fused, 0.1 * 10.0 - 1.0);
    let want = vec![fused.to_bits(); 3];
    let args = [
        floats(&[0.0; 3]),
        floats(&[0.1; 3]),
        floats(&[10.0; 3]),
        floats(&[-1.0; 3]),
    ];
    assert_tiers_bits(&j, "ffma!", &args, &want);
}

#[test]
fn ifma_all_tiers_wrap() {
    let j = jit_with(&[IFMA]);
    for &n in LENS {
        let (a, b, c) = (lcg_ints(n, 1), lcg_ints(n, 2), lcg_ints(n, 3));
        let want: Vec<u64> = (0..n)
            .map(|i| a[i].wrapping_mul(b[i]).wrapping_add(c[i]) as u64)
            .collect();
        let o = ints(&vec![7; n]);
        assert_tiers_bits(&j, "ifma!", &[o, ints(&a), ints(&b), ints(&c)], &want);
    }
}

#[test]
fn fneg_all_tiers_flip_the_sign_bit() {
    let j = jit_with(&[FNEG]);
    for &n in LENS {
        let a = fcase(n, 2);
        let want: Vec<u64> = a.iter().map(|x| (-x).to_bits()).collect();
        let o = floats(&vec![9.0; n]);
        assert_tiers_bits(&j, "fneg!", &[o, floats(&a)], &want);
    }
}

#[test]
fn ineg_all_tiers_wrap() {
    let j = jit_with(&[INEG]);
    for &n in LENS {
        let mut a = lcg_ints(n, 4);
        if let Some(x) = a.first_mut() {
            *x = i64::MIN;
        }
        let want: Vec<u64> = a.iter().map(|x| x.wrapping_neg() as u64).collect();
        let o = ints(&vec![7; n]);
        assert_tiers_bits(&j, "ineg!", &[o, ints(&a)], &want);
    }
}

#[test]
fn min_len_leaves_the_tail_of_out_untouched() {
    // len(out) = 7, len(a) = 5, len(b) = 6: five elements written (two
    // vector iterations + the scalar tail), out[5..] keeps its values.
    let j = jit_with(&[FDIV, FFMA]);
    let a = [1.0f64, 2.0, 3.0, 4.0, 5.0];
    let b = [2.0, 4.0, 8.0, 16.0, 32.0, 64.0];
    let mut want: Vec<u64> = a.iter().zip(&b).map(|(x, y)| (x / y).to_bits()).collect();
    want.extend([(-7.0f64).to_bits(); 2]);
    let o = floats(&[-7.0; 7]);
    assert_tiers_bits(&j, "fdiv!", &[o, floats(&a), floats(&b)], &want);

    // `out` shortest: only its own length is written.
    let want: Vec<u64> = (0..3).map(|i| a[i].mul_add(b[i], 1.0).to_bits()).collect();
    let args = [floats(&[0.0; 3]), floats(&a), floats(&b), floats(&[1.0; 9])];
    assert_tiers_bits(&j, "ffma!", &args, &want);
}

#[test]
fn out_may_alias_every_input() {
    let j = jit_with(&[FFMA_ALIAS]);
    let a = lcg_floats(9, 21);
    let c = lcg_floats(9, 22);
    let want: Vec<u64> = (0..9).map(|i| a[i].mul_add(a[i], c[i]).to_bits()).collect();
    assert_tiers_bits(&j, "ffma-alias!", &[floats(&a), floats(&c)], &want);
}
