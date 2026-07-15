//! Differential tests for the SIMD elementwise array-op family
//! (`array-add!`/`array-sub!`/`array-mul!`, `Core::ArrayMap2`): out-param,
//! wrapping, elementwise `+`/`-`/`*` over `(array int64)`/`(array float64)`,
//! iterating `min(len out, len a, len b)`.
//!
//! Mirrors `test_jit_array_parity.rs`'s structure: drive the same call
//! through every typed tier (native-or-closure `compile_all`, the typed-core
//! `deoptimize_all`, and the tracing interpreter `trace_call`) and assert
//! they agree bit-for-bit — the native Cranelift SIMD lowering
//! (`src/jit/native.rs::Emitter::emit_array_map2`, a 2-lane vector loop plus
//! a scalar tail) must match the scalar reference
//! (`src/jit/runtime.rs::array_map2`, shared by the Core interpreter, the
//! tracer, and the closure backend) exactly. Elementwise ops have no
//! reduction/reassociation, so bit-identical results are the expected (not
//! just "close enough") outcome for both int and float.

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

const IADD: &str = "(defun-typed (iadd! (array int64)) ((o (array int64)) (a (array int64)) (b (array int64))) (array-add! o a b))";
const ISUB: &str = "(defun-typed (isub! (array int64)) ((o (array int64)) (a (array int64)) (b (array int64))) (array-sub! o a b))";
const IMUL: &str = "(defun-typed (imul! (array int64)) ((o (array int64)) (a (array int64)) (b (array int64))) (array-mul! o a b))";
const FADD: &str = "(defun-typed (fadd! (array float64)) ((o (array float64)) (a (array float64)) (b (array float64))) (array-add! o a b))";
const FSUB: &str = "(defun-typed (fsub! (array float64)) ((o (array float64)) (a (array float64)) (b (array float64))) (array-sub! o a b))";
const FMUL: &str = "(defun-typed (fmul! (array float64)) ((o (array float64)) (a (array float64)) (b (array float64))) (array-mul! o a b))";

fn ints(xs: &[i64]) -> Value {
    Value::Array(xs.iter().map(|x| Value::Int(*x)).collect())
}
fn floats(xs: &[f64]) -> Value {
    Value::Array(xs.iter().map(|x| Value::Float(*x)).collect())
}

/// Drive `name(args)` through every typed tier: the "compiled" edition
/// (native Cranelift under `--features jit`, the TCO closure tree
/// otherwise), the typed-core interpreter (`deoptimize_all`), and the
/// tracing interpreter. Returns `(return_value, updated_out_param)` per
/// tier via `call_with_array_writeback` (argument 0 is always `out`).
fn call_all_tiers_wb(j: &Jit, name: &str, args: &[Value]) -> [(Value, Value); 3] {
    j.compile_all();
    let (rv, upd, _flags) = j
        .call_with_array_writeback(name, args)
        .unwrap_or_else(|e| panic!("{name} compiled: {e}"));
    let compiled = (rv, upd[0].clone().expect("out param must write back"));

    j.deoptimize_all();
    let (rv, upd, _flags) = j
        .call_with_array_writeback(name, args)
        .unwrap_or_else(|e| panic!("{name} deopt: {e}"));
    let interpreted = (rv, upd[0].clone().expect("out param must write back"));

    let (rv, _log) = j
        .trace_call(name, args)
        .unwrap_or_else(|e| panic!("{name} traced: {e}"));
    // `trace_call` doesn't report write-back separately; re-derive it from a
    // fresh `call_with_array_writeback` immediately after (tracing doesn't
    // change any tier's compiled/interpreted state) so the tail element
    // count matches — the value itself is what we assert on, not identity.
    let (_rv2, upd, _flags) = j
        .call_with_array_writeback(name, args)
        .unwrap_or_else(|e| panic!("{name} traced-writeback: {e}"));
    let traced = (rv, upd[0].clone().expect("out param must write back"));

    [compiled, interpreted, traced]
}

fn assert_all_tiers_agree(
    j: &Jit,
    name: &str,
    args: &[Value],
    expect_ret: &Value,
    expect_out: &Value,
) {
    let labels = ["compiled", "deopt-interpreter", "traced"];
    for (tier, (rv, out)) in labels.iter().zip(call_all_tiers_wb(j, name, args)) {
        assert_eq!(&rv, expect_ret, "{tier}: return value (= out) must match");
        assert_eq!(&out, expect_out, "{tier}: written-back `out` must match");
    }
}

// ---- int64: basic elementwise, odd length (exercises the scalar tail) -----

#[test]
fn int_add_odd_length_all_tiers() {
    let j = jit_with(&[IADD]);
    let o = ints(&[0, 0, 0, 0, 0]);
    let a = ints(&[1, 2, 3, 4, 5]);
    let b = ints(&[10, 20, 30, 40, 50]);
    let expect = ints(&[11, 22, 33, 44, 55]);
    assert_all_tiers_agree(&j, "iadd!", &[o, a, b], &expect, &expect);
}

#[test]
fn int_sub_odd_length_all_tiers() {
    let j = jit_with(&[ISUB]);
    let o = ints(&[0, 0, 0, 0, 0]);
    let a = ints(&[10, 20, 30, 40, 50]);
    let b = ints(&[1, 2, 3, 4, 5]);
    let expect = ints(&[9, 18, 27, 36, 45]);
    assert_all_tiers_agree(&j, "isub!", &[o, a, b], &expect, &expect);
}

#[test]
fn int_mul_odd_length_all_tiers() {
    let j = jit_with(&[IMUL]);
    let o = ints(&[0, 0, 0, 0, 0]);
    let a = ints(&[1, 2, 3, 4, 5]);
    let b = ints(&[10, 20, 30, 40, 50]);
    let expect = ints(&[10, 40, 90, 160, 250]);
    assert_all_tiers_agree(&j, "imul!", &[o, a, b], &expect, &expect);
}

// ---- int64: even length (pure vectorized loop, no scalar tail) ------------

#[test]
fn int_add_even_length_all_tiers() {
    let j = jit_with(&[IADD]);
    let o = ints(&[0, 0, 0, 0]);
    let a = ints(&[1, 2, 3, 4]);
    let b = ints(&[10, 20, 30, 40]);
    let expect = ints(&[11, 22, 33, 44]);
    assert_all_tiers_agree(&j, "iadd!", &[o, a, b], &expect, &expect);
}

// ---- differing lengths: iterate min(len out, len a, len b) ----------------

#[test]
fn int_add_differing_lengths_uses_min_len_all_tiers() {
    let j = jit_with(&[IADD]);
    // len(o) = 6, len(a) = 3, len(b) = 6 -> min = 3; indices 3..6 of `o`
    // stay at their initial values (untouched).
    let o = ints(&[-1, -1, -1, -1, -1, -1]);
    let a = ints(&[1, 2, 3]);
    let b = ints(&[100, 200, 300, 400, 500, 600]);
    let expect = ints(&[101, 202, 303, -1, -1, -1]);
    assert_all_tiers_agree(&j, "iadd!", &[o, a, b], &expect, &expect);
}

// ---- wrapping semantics: no panic, no OVERFLOW flag, matches wrapping_* ---

#[test]
fn int_add_wraps_at_i64_max_all_tiers() {
    let j = jit_with(&[IADD]);
    let o = ints(&[0, 0]);
    let a = ints(&[i64::MAX, i64::MIN]);
    let b = ints(&[1, -1]);
    let expect = ints(&[i64::MAX.wrapping_add(1), i64::MIN.wrapping_add(-1)]);
    assert_eq!(expect, ints(&[i64::MIN, i64::MAX]));
    assert_all_tiers_agree(&j, "iadd!", &[o, a, b], &expect, &expect);
}

#[test]
fn int_sub_wraps_all_tiers() {
    let j = jit_with(&[ISUB]);
    let o = ints(&[0, 0]);
    let a = ints(&[i64::MIN, i64::MAX]);
    let b = ints(&[1, -1]);
    let expect = ints(&[i64::MIN.wrapping_sub(1), i64::MAX.wrapping_sub(-1)]);
    assert_all_tiers_agree(&j, "isub!", &[o, a, b], &expect, &expect);
}

#[test]
fn int_mul_wraps_all_tiers() {
    let j = jit_with(&[IMUL]);
    let o = ints(&[0, 0]);
    let a = ints(&[i64::MAX, i64::MIN]);
    let b = ints(&[2, 2]);
    let expect = ints(&[i64::MAX.wrapping_mul(2), i64::MIN.wrapping_mul(2)]);
    assert_all_tiers_agree(&j, "imul!", &[o, a, b], &expect, &expect);
}

#[test]
fn int_add_does_not_set_overflow_flag() {
    // A bulk vector op has no per-lane overflow flag, so the whole family is
    // defined as wrapping-with-no-flag (unlike scalar `+`, which sets
    // OVERFLOW). Assert the flag stays clear even when a lane wraps.
    let j = jit_with(&[IADD]);
    let o = ints(&[0]);
    let a = ints(&[i64::MAX]);
    let b = ints(&[1]);
    j.compile_all();
    let (_rv, _upd, flags) = j
        .call_with_array_writeback("iadd!", &[o.clone(), a.clone(), b.clone()])
        .unwrap();
    assert!(
        !flags.overflow,
        "compiled: array-add! must never set OVERFLOW"
    );
    j.deoptimize_all();
    let (_rv, _upd, flags) = j.call_with_array_writeback("iadd!", &[o, a, b]).unwrap();
    assert!(
        !flags.overflow,
        "interpreted: array-add! must never set OVERFLOW"
    );
}

// ---- float64: basic elementwise, odd length --------------------------------

#[test]
fn float_add_odd_length_all_tiers() {
    let j = jit_with(&[FADD]);
    let o = floats(&[0.0, 0.0, 0.0, 0.0, 0.0]);
    let a = floats(&[1.0, 2.0, 3.0, 4.0, 5.0]);
    let b = floats(&[0.5, 0.5, 0.5, 0.5, 0.5]);
    let expect = floats(&[1.5, 2.5, 3.5, 4.5, 5.5]);
    assert_all_tiers_agree(&j, "fadd!", &[o, a, b], &expect, &expect);
}

#[test]
fn float_sub_odd_length_all_tiers() {
    let j = jit_with(&[FSUB]);
    let o = floats(&[0.0, 0.0, 0.0, 0.0, 0.0]);
    let a = floats(&[1.0, 2.0, 3.0, 4.0, 5.0]);
    let b = floats(&[0.5, 0.5, 0.5, 0.5, 0.5]);
    let expect = floats(&[0.5, 1.5, 2.5, 3.5, 4.5]);
    assert_all_tiers_agree(&j, "fsub!", &[o, a, b], &expect, &expect);
}

#[test]
fn float_mul_odd_length_all_tiers() {
    let j = jit_with(&[FMUL]);
    let o = floats(&[0.0, 0.0, 0.0, 0.0, 0.0]);
    let a = floats(&[1.0, 2.0, 3.0, 4.0, 5.0]);
    let b = floats(&[2.0, 2.0, 2.0, 2.0, 2.0]);
    let expect = floats(&[2.0, 4.0, 6.0, 8.0, 10.0]);
    assert_all_tiers_agree(&j, "fmul!", &[o, a, b], &expect, &expect);
}

#[test]
fn float_add_even_length_all_tiers() {
    let j = jit_with(&[FADD]);
    let o = floats(&[0.0, 0.0, 0.0, 0.0]);
    let a = floats(&[1.5, 2.5, 3.5, 4.5]);
    let b = floats(&[0.25, 0.25, 0.25, 0.25]);
    let expect = floats(&[1.75, 2.75, 3.75, 4.75]);
    assert_all_tiers_agree(&j, "fadd!", &[o, a, b], &expect, &expect);
}

// ---- zero / empty ranges ---------------------------------------------------

#[test]
fn int_add_empty_arrays_all_tiers() {
    let j = jit_with(&[IADD]);
    let o = ints(&[]);
    let a = ints(&[]);
    let b = ints(&[]);
    let expect = ints(&[]);
    assert_all_tiers_agree(&j, "iadd!", &[o, a, b], &expect, &expect);
}

#[test]
fn int_add_single_element_all_tiers() {
    // Length 1: vec_end = 0, entirely handled by the scalar tail.
    let j = jit_with(&[IADD]);
    let o = ints(&[0]);
    let a = ints(&[7]);
    let b = ints(&[35]);
    let expect = ints(&[42]);
    assert_all_tiers_agree(&j, "iadd!", &[o, a, b], &expect, &expect);
}
