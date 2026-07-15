//! Differential tests for the SIMD integer array-reduction family
//! (`array-sum`/`array-dot`, `Core::ArraySum`/`Core::ArrayDot`): wrapping,
//! int64-only reductions over `(array int64)`.
//!
//! Mirrors `test_simd_arrayops.rs`'s structure: drive the same call through
//! every typed tier (native-or-closure `compile_all`, the typed-core
//! `deoptimize_all`, and the tracing interpreter `trace_call`) and assert
//! they agree bit-for-bit — the native Cranelift SIMD lowering
//! (`src/jit/native.rs::Emitter::emit_array_sum`/`emit_array_dot`, a 2-lane
//! `I64X2` accumulator loop plus a horizontal `extractlane`/`iadd` reduction
//! and a scalar tail) must match the scalar reference
//! (`src/jit/runtime.rs::array_sum`/`array_dot`, shared by the Core
//! interpreter, the tracer, and the closure backend) exactly. Wrapping
//! int64 addition is associative, so a sequential fold and a vectorized
//! pairwise reduction are bit-identical by construction — that's the whole
//! point of the exercise, and these tests are the parity anchor for it
//! (the differential fuzzer does not generate these ops).

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
}
