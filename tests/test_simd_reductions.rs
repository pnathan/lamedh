//! Differential tests for the SIMD array-reduction family
//! (`array-sum`/`array-dot`, `Core::ArraySum`/`Core::ArrayDot`) over
//! `(array int64)` (wrapping) and `(array float64)` (issue #392).
//!
//! int64: wrapping addition is associative, so every tier (native SIMD in
//! `src/jit/native.rs::Emitter::emit_array_reduce`, the scalar reference
//! `src/jit/runtime.rs::array_sum`/`array_dot` shared by the Core
//! interpreter, tracer and closure backend) must agree exactly.
//!
//! float64: the contract follows Fortran's `SUM` — addition order is
//! unspecified — so float tests check a tolerance against a sequential sum;
//! bitwise tier agreement is asserted only in tests labelled
//! implementation-level.

use lamedh::environment::Environment;
use lamedh::jit::{Jit, Value};
use lamedh::reader::read;

/// Build a fresh `Jit` with `defs` (each a `defun-typed` source string).
fn jit_with(defs: &[&str]) -> Jit {
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    for src in defs {
        let form = read(src, &env).unwrap_or_else(|e| panic!("read failed for `{src}`: {e}"));
        j.define(&form)
            .unwrap_or_else(|e| panic!("define failed for `{src}`: {e}"));
    }
    j
}

const SUM: &str = "(defun-typed (s int64) ((a (array int64))) (array-sum a))";
const DOT: &str = "(defun-typed (d int64) ((a (array int64)) (b (array int64))) (array-dot a b))";

fn ints(xs: &[i64]) -> Value {
    Value::Array(xs.iter().map(|x| Value::Int(*x)).collect())
}

/// Drive `name(args)` through every typed tier: the "compiled" edition
/// (native Cranelift under `--features jit`, the TCO closure tree
/// otherwise), the typed-core interpreter (`deoptimize_all`), and the
/// tracing interpreter. Returns the scalar result per tier.
fn call_all_tiers(j: &Jit, name: &str, args: &[Value]) -> [Value; 3] {
    j.compile_all();
    let compiled = j
        .call(name, args)
        .unwrap_or_else(|e| panic!("{name} compiled: {e}"));

    j.deoptimize_all();
    let interpreted = j
        .call(name, args)
        .unwrap_or_else(|e| panic!("{name} deopt: {e}"));

    let (traced, _log) = j
        .trace_call(name, args)
        .unwrap_or_else(|e| panic!("{name} traced: {e}"));

    [compiled, interpreted, traced]
}

fn assert_all_tiers_agree(j: &Jit, name: &str, args: &[Value], expect: &Value) {
    let labels = ["compiled", "deopt-interpreter", "traced"];
    for (tier, rv) in labels.iter().zip(call_all_tiers(j, name, args)) {
        assert_eq!(&rv, expect, "{tier}: return value must match");
    }
}

// ---- array-sum --------------------------------------------------------

#[test]
fn sum_empty_array_all_tiers() {
    let j = jit_with(&[SUM]);
    let a = ints(&[]);
    assert_all_tiers_agree(&j, "s", &[a], &Value::Int(0));
}

#[test]
fn sum_single_element_all_tiers() {
    // Length 1: vec_end = 0, entirely handled by the scalar tail.
    let j = jit_with(&[SUM]);
    let a = ints(&[42]);
    assert_all_tiers_agree(&j, "s", &[a], &Value::Int(42));
}

#[test]
fn sum_even_length_all_tiers() {
    // Pure vectorized loop, no scalar tail.
    let j = jit_with(&[SUM]);
    let a = ints(&[1, 2, 3, 4]);
    assert_all_tiers_agree(&j, "s", &[a], &Value::Int(10));
}

#[test]
fn sum_odd_length_all_tiers() {
    // Exercises the scalar tail after a full vectorized pass.
    let j = jit_with(&[SUM]);
    let a = ints(&[1, 2, 3, 4, 5]);
    assert_all_tiers_agree(&j, "s", &[a], &Value::Int(15));
}

#[test]
fn sum_negative_elements_all_tiers() {
    let j = jit_with(&[SUM]);
    let a = ints(&[-1, -2, -3, 4, 5]);
    assert_all_tiers_agree(&j, "s", &[a], &Value::Int(3));
}

#[test]
fn sum_wraps_at_i64_max_all_tiers() {
    let j = jit_with(&[SUM]);
    let a = ints(&[i64::MAX, i64::MAX]);
    let expect = i64::MAX.wrapping_add(i64::MAX);
    assert_eq!(expect, -2);
    assert_all_tiers_agree(&j, "s", &[a], &Value::Int(expect));
}

#[test]
fn sum_wraps_odd_length_all_tiers() {
    // Wraparound landing in the scalar tail element.
    let j = jit_with(&[SUM]);
    let a = ints(&[i64::MAX, i64::MAX, i64::MAX]);
    let expect = i64::MAX.wrapping_add(i64::MAX).wrapping_add(i64::MAX);
    assert_all_tiers_agree(&j, "s", &[a], &Value::Int(expect));
}

#[test]
fn sum_does_not_set_overflow_flag() {
    // A bulk vector reduction has no per-lane overflow flag, so the whole
    // family is defined as wrapping-with-no-flag (unlike scalar `+`, which
    // sets OVERFLOW). Assert the flag stays clear even when the sum wraps.
    let j = jit_with(&[SUM]);
    let a = ints(&[i64::MAX, 1]);
    j.compile_all();
    let (_rv, _upd, flags) = j
        .call_with_array_writeback("s", std::slice::from_ref(&a))
        .unwrap();
    assert!(
        !flags.overflow,
        "compiled: array-sum must never set OVERFLOW"
    );
    j.deoptimize_all();
    let (_rv, _upd, flags) = j.call_with_array_writeback("s", &[a]).unwrap();
    assert!(
        !flags.overflow,
        "interpreted: array-sum must never set OVERFLOW"
    );
}

// ---- array-dot ----------------------------------------------------------

#[test]
fn dot_empty_arrays_all_tiers() {
    let j = jit_with(&[DOT]);
    let a = ints(&[]);
    let b = ints(&[]);
    assert_all_tiers_agree(&j, "d", &[a, b], &Value::Int(0));
}

#[test]
fn dot_single_element_all_tiers() {
    let j = jit_with(&[DOT]);
    let a = ints(&[6]);
    let b = ints(&[7]);
    assert_all_tiers_agree(&j, "d", &[a, b], &Value::Int(42));
}

#[test]
fn dot_even_length_all_tiers() {
    let j = jit_with(&[DOT]);
    let a = ints(&[1, 2, 3, 4]);
    let b = ints(&[10, 20, 30, 40]);
    // 10 + 40 + 90 + 160 = 300
    assert_all_tiers_agree(&j, "d", &[a, b], &Value::Int(300));
}

#[test]
fn dot_odd_length_all_tiers() {
    let j = jit_with(&[DOT]);
    let a = ints(&[1, 2, 3, 4, 5]);
    let b = ints(&[10, 20, 30, 40, 50]);
    // 10 + 40 + 90 + 160 + 250 = 550
    assert_all_tiers_agree(&j, "d", &[a, b], &Value::Int(550));
}

#[test]
fn dot_differing_lengths_uses_min_len_all_tiers() {
    // len(a) = 3, len(b) = 6 -> min = 3; the trailing elements of `b` never
    // participate.
    let j = jit_with(&[DOT]);
    let a = ints(&[1, 2, 3]);
    let b = ints(&[100, 200, 300, 400, 500, 600]);
    // 100 + 400 + 900 = 1400
    assert_all_tiers_agree(&j, "d", &[a, b], &Value::Int(1400));
}

#[test]
fn dot_differing_lengths_other_order_all_tiers() {
    // len(a) = 6, len(b) = 3 -> min = 3 too, exercising the other operand
    // being the shorter one.
    let j = jit_with(&[DOT]);
    let a = ints(&[100, 200, 300, 400, 500, 600]);
    let b = ints(&[1, 2, 3]);
    assert_all_tiers_agree(&j, "d", &[a, b], &Value::Int(1400));
}

#[test]
fn dot_product_wraps_all_tiers() {
    // The per-element product overflows i64 and wraps.
    let j = jit_with(&[DOT]);
    let a = ints(&[i64::MAX, 1]);
    let b = ints(&[2, 1]);
    let expect = i64::MAX.wrapping_mul(2).wrapping_add(1);
    assert_all_tiers_agree(&j, "d", &[a, b], &Value::Int(expect));
}

#[test]
fn dot_sum_wraps_all_tiers() {
    // The running sum itself overflows i64 and wraps, even though no
    // individual product does.
    let j = jit_with(&[DOT]);
    let a = ints(&[i64::MAX, i64::MAX]);
    let b = ints(&[1, 1]);
    let expect = i64::MAX.wrapping_add(i64::MAX);
    assert_all_tiers_agree(&j, "d", &[a, b], &Value::Int(expect));
}

#[test]
fn dot_negative_elements_all_tiers() {
    let j = jit_with(&[DOT]);
    let a = ints(&[-1, 2, -3]);
    let b = ints(&[4, -5, 6]);
    // -4 - 10 - 18 = -32
    assert_all_tiers_agree(&j, "d", &[a, b], &Value::Int(-32));
}

#[test]
fn dot_does_not_set_overflow_flag() {
    let j = jit_with(&[DOT]);
    let a = ints(&[i64::MAX, i64::MAX]);
    let b = ints(&[2, 2]);
    j.compile_all();
    let (_rv, _upd, flags) = j
        .call_with_array_writeback("d", &[a.clone(), b.clone()])
        .unwrap();
    assert!(
        !flags.overflow,
        "compiled: array-dot must never set OVERFLOW"
    );
    j.deoptimize_all();
    let (_rv, _upd, flags) = j.call_with_array_writeback("d", &[a, b]).unwrap();
    assert!(
        !flags.overflow,
        "interpreted: array-dot must never set OVERFLOW"
    );
}

// ---- explain-compile / disassembly smoke test ----------------------------

#[test]
fn array_sum_and_dot_compile_natively() {
    let j = jit_with(&[SUM, DOT]);
    j.compile_all();
    let dis_s = j.disassemble("s").expect("disassemble s");
    assert!(
        dis_s.contains("vsum"),
        "array-sum disassembly should mention vsum: {dis_s}"
    );
    let dis_d = j.disassemble("d").expect("disassemble d");
    assert!(
        dis_d.contains("vdot"),
        "array-dot disassembly should mention vdot: {dis_d}"
    );
    let jf = jit_with(&[SUMF, DOTF]);
    jf.compile_all();
    assert!(
        jf.disassemble("sf")
            .expect("disassemble sf")
            .contains("vsum")
    );
    assert!(
        jf.disassemble("df")
            .expect("disassemble df")
            .contains("vdot")
    );
}

// ---- float64 array-sum / array-dot (issue #392) ---------------------------
//
// CONTRACT (Fortran `SUM`-aligned): for float64 the result approximates the
// mathematical sum; the order of additions is unspecified. The contract
// tests below therefore compare against a sequential left fold with a
// Higham-style tolerance of `n * eps * sum|x|`, never bit-for-bit.
//
// IMPLEMENTATION-LEVEL tests (labelled as such) additionally assert that
// every tier of THIS implementation agrees bit-for-bit, because they all use
// the same 8-lane reduction shape. That is not a language guarantee.

const SUMF: &str = "(defun-typed (sf float64) ((a (array float64))) (array-sum a))";
const DOTF: &str =
    "(defun-typed (df float64) ((a (array float64)) (b (array float64))) (array-dot a b))";
const FLOAT_SIZES: &[usize] = &[0, 1, 7, 8, 9, 15, 16, 17, 1000];

fn floats(xs: &[f64]) -> Value {
    Value::Array(xs.iter().map(|x| Value::Float(*x)).collect())
}

/// Deterministic mixed-sign, mixed-magnitude data (so reassociation
/// actually changes rounding).
fn gen_floats(n: usize, seed: u64) -> Vec<f64> {
    let mut s = seed
        .wrapping_mul(6364136223846793005)
        .wrapping_add(1442695040888963407);
    (0..n)
        .map(|_| {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            let m = ((s >> 11) as f64) / ((1u64 << 53) as f64) - 0.5;
            let e = ((s >> 3) % 12) as i32 - 6;
            m * 10f64.powi(e)
        })
        .collect()
}

fn as_float(v: &Value) -> f64 {
    match v {
        Value::Float(f) => *f,
        other => panic!("expected a float, got {other:?}"),
    }
}

/// Assert `got` is within the Higham-style bound of the sequential sum of
/// `terms`.
fn assert_close_to_sequential(got: f64, terms: &[f64], what: &str) {
    let seq: f64 = terms.iter().fold(0.0, |a, x| a + x);
    let abs: f64 = terms.iter().map(|x| x.abs()).sum();
    let tol = (terms.len().max(1) as f64) * f64::EPSILON * abs;
    assert!(
        (got - seq).abs() <= tol,
        "{what}: {got} vs sequential {seq} exceeds tolerance {tol}"
    );
}

#[test]
fn float_sum_within_tolerance_all_tiers() {
    let j = jit_with(&[SUMF]);
    for &n in FLOAT_SIZES {
        let xs = gen_floats(n, n as u64 + 1);
        for (tier, rv) in
            ["compiled", "deopt", "traced"]
                .iter()
                .zip(call_all_tiers(&j, "sf", &[floats(&xs)]))
        {
            assert_close_to_sequential(as_float(&rv), &xs, &format!("sum n={n} {tier}"));
        }
    }
}

#[test]
fn float_dot_within_tolerance_all_tiers() {
    let j = jit_with(&[DOTF]);
    for &n in FLOAT_SIZES {
        let xs = gen_floats(n, 2 * n as u64 + 3);
        let ys = gen_floats(n, 3 * n as u64 + 5);
        let prods: Vec<f64> = xs.iter().zip(&ys).map(|(x, y)| x * y).collect();
        for (tier, rv) in ["compiled", "deopt", "traced"].iter().zip(call_all_tiers(
            &j,
            "df",
            &[floats(&xs), floats(&ys)],
        )) {
            assert_close_to_sequential(as_float(&rv), &prods, &format!("dot n={n} {tier}"));
        }
    }
}

#[test]
fn float_sum_exact_for_small_integer_values_all_tiers() {
    // Small integer-valued floats sum exactly in any order.
    let j = jit_with(&[SUMF, DOTF]);
    for &n in FLOAT_SIZES {
        let xs: Vec<f64> = (0..n).map(|i| (i % 13) as f64 - 6.0).collect();
        let expect: f64 = xs.iter().sum();
        assert_all_tiers_agree(&j, "sf", &[floats(&xs)], &Value::Float(expect));
        let dexpect: f64 = xs.iter().map(|x| x * x).sum();
        assert_all_tiers_agree(
            &j,
            "df",
            &[floats(&xs), floats(&xs)],
            &Value::Float(dexpect),
        );
    }
}

#[test]
fn float_empty_sum_is_positive_zero() {
    let j = jit_with(&[SUMF]);
    for rv in call_all_tiers(&j, "sf", &[floats(&[])]) {
        let f = as_float(&rv);
        assert!(f == 0.0 && f.is_sign_positive(), "empty sum: {f}");
    }
}

#[test]
fn float_nan_and_inf_propagate_all_tiers() {
    let j = jit_with(&[SUMF, DOTF]);
    for &n in &[1usize, 8, 9, 17] {
        for pos in [0, n - 1, n / 2] {
            let mut xs = vec![1.0; n];
            xs[pos] = f64::NAN;
            for rv in call_all_tiers(&j, "sf", &[floats(&xs)]) {
                assert!(as_float(&rv).is_nan(), "NaN at {pos}/{n}");
            }
            xs[pos] = f64::INFINITY;
            for rv in call_all_tiers(&j, "sf", &[floats(&xs)]) {
                assert_eq!(as_float(&rv), f64::INFINITY, "inf at {pos}/{n}");
            }
            // inf * 0 = NaN in the dot.
            let zs = vec![0.0; n];
            for rv in call_all_tiers(&j, "df", &[floats(&xs), floats(&zs)]) {
                assert!(as_float(&rv).is_nan(), "inf*0 at {pos}/{n}");
            }
        }
        let mut xs = vec![1.0; n];
        xs[0] = f64::INFINITY;
        xs[n - 1] = f64::NEG_INFINITY;
        if n > 1 {
            for rv in call_all_tiers(&j, "sf", &[floats(&xs)]) {
                assert!(as_float(&rv).is_nan(), "inf + -inf, n={n}");
            }
        }
    }
}

#[test]
fn float_dot_mismatched_lengths_uses_min_len_all_tiers() {
    let j = jit_with(&[DOTF]);
    let a = floats(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0]);
    let b = floats(&[1.0, 1.0, 1.0]);
    assert_all_tiers_agree(&j, "df", &[a.clone(), b.clone()], &Value::Float(6.0));
    assert_all_tiers_agree(&j, "df", &[b, a], &Value::Float(6.0));
}

#[test]
fn mixed_element_types_do_not_elaborate() {
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    let src = "(defun-typed (m float64) ((a (array int64)) (b (array float64))) (array-dot a b))";
    let form = read(src, &env).unwrap();
    assert!(
        j.define(&form).is_err(),
        "array-dot over (array int64) x (array float64) must be a type error"
    );
    let src = "(defun-typed (m2 int64) ((a (array float64))) (array-sum a))";
    let form = read(src, &env).unwrap();
    assert!(
        j.define(&form).is_err(),
        "array-sum over (array float64) returns float64, not int64"
    );
}

/// IMPLEMENTATION-LEVEL (not a language guarantee): every tier of this
/// implementation uses the same 8-lane shape, so they agree bit-for-bit.
#[test]
fn impl_float_tiers_agree_bitwise() {
    let j = jit_with(&[SUMF, DOTF]);
    for &n in FLOAT_SIZES {
        let xs = gen_floats(n, 7 * n as u64 + 11);
        let ys = gen_floats(n, 5 * n as u64 + 13);
        let s = call_all_tiers(&j, "sf", &[floats(&xs)]);
        let d = call_all_tiers(&j, "df", &[floats(&xs), floats(&ys)]);
        for r in [s, d] {
            let bits: Vec<u64> = r.iter().map(|v| as_float(v).to_bits()).collect();
            assert!(
                bits.iter().all(|b| *b == bits[0]),
                "n={n}: tiers differ {r:?}"
            );
        }
    }
}

/// Tree-walker float path: contract (tolerance) plus, implementation-level,
/// bitwise agreement with the typed JIT tiers.
#[test]
fn tree_walker_float_sum_and_dot() {
    lamedh::with_large_stack(|| {
        let env = Environment::with_stdlib();
        let j = jit_with(&[SUMF, DOTF]);
        let lit = |xs: &[f64]| {
            let body: Vec<String> = xs.iter().map(|x| format!("{x:?}")).collect();
            format!("(list->array '({}))", body.join(" "))
        };
        let get = |src: &str| match lamedh::eval_str(src, &env).expect(src) {
            lamedh::LispVal::Float(f) => f,
            other => panic!("{src}: expected float, got {other:?}"),
        };
        for &n in &[1usize, 7, 8, 9, 15, 16, 17, 200] {
            let xs: Vec<f64> = gen_floats(n, n as u64 + 99)
                .iter()
                .map(|x| (x * 1e6).round() / 1e3 + 0.5)
                .collect();
            let ys: Vec<f64> = xs.iter().rev().copied().collect();
            let s = get(&format!("(array-sum {})", lit(&xs)));
            assert_close_to_sequential(s, &xs, &format!("tree sum n={n}"));
            let prods: Vec<f64> = xs.iter().zip(&ys).map(|(x, y)| x * y).collect();
            let d = get(&format!("(array-dot {} {})", lit(&xs), lit(&ys)));
            assert_close_to_sequential(d, &prods, &format!("tree dot n={n}"));
            // Implementation-level: matches the typed tiers bit for bit.
            j.compile_all();
            let js = as_float(&j.call("sf", &[floats(&xs)]).unwrap());
            let jd = as_float(&j.call("df", &[floats(&xs), floats(&ys)]).unwrap());
            assert_eq!(s.to_bits(), js.to_bits(), "tree vs jit sum n={n}");
            assert_eq!(d.to_bits(), jd.to_bits(), "tree vs jit dot n={n}");
        }
        // Mixed int/float elements promote to float64; all-int stays int.
        assert_eq!(get("(array-sum (list->array '(1 2.5 3)))"), 6.5);
        assert_eq!(
            get("(array-dot (list->array '(1 2)) (list->array '(0.5 0.25 9.0)))"),
            1.0
        );
        assert!(matches!(
            lamedh::eval_str("(array-sum (list->array '(1 2 3)))", &env).unwrap(),
            lamedh::LispVal::Number(6)
        ));
        assert!(lamedh::eval_str("(array-sum (list->array '(1 a)))", &env).is_err());
    });
}
