//! Differential array-index parity tests between the tree-walking evaluator
//! and every typed tier (issue #282).
//!
//! The evaluator's `FETCH`/`STORE` (`src/evaluator/apply.rs`, `ArrayFetch`/
//! `ArrayStore`) *error* on a negative index and on an out-of-bounds index.
//! The typed tiers used to bounds-check and then silently substitute (fetch →
//! 0, store → no-op), a #209-direction silent-wrong-value divergence. They now
//! record the evaluator's exact index message via the `pending_error` membrane
//! (issue #271's mechanism) and raise it once the call returns, while still
//! producing the memory-safe substitute inside the call so the rest of the
//! body stays panic-free.
//!
//! Each test drives the same out-of-range access through:
//!   * the plain evaluator (source of truth),
//!   * the compiled edition (`compile_all` — native Cranelift under `--features
//!     jit`, the closure tree otherwise),
//!   * the typed-core interpreter (`deoptimize_all` — `eval_core`), and
//!   * the tracing interpreter (`trace_call` — `eval_core_traced`),
//!
//! and asserts every tier raises the identical message.

use lamedh::environment::Environment;
use lamedh::eval_line;
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

/// Call `name(args)` on `j` through each typed tier, returning the per-tier
/// `Result` error string (or the `Ok` value's debug form) for
/// (compiled, deopt-interpreter, traced). All three must agree.
fn call_all_tiers(j: &Jit, name: &str, args: &[Value]) -> [Result<Value, String>; 3] {
    j.compile_all();
    let compiled = j.call(name, args);
    j.deoptimize_all();
    let interpreted = j.call(name, args);
    let traced = j.trace_call(name, args).map(|(v, _log)| v);
    [compiled, interpreted, traced]
}

/// Assert every typed tier errored with exactly `expected`, matching the
/// evaluator.
fn assert_all_tiers_error(j: &Jit, name: &str, args: &[Value], expected: &str) {
    let labels = ["compiled", "deopt-interpreter", "traced"];
    for (tier, r) in labels.iter().zip(call_all_tiers(j, name, args)) {
        match r {
            Err(msg) => assert_eq!(
                msg, expected,
                "{tier} tier: index error message must match the evaluator"
            ),
            Ok(v) => panic!("{tier} tier: expected error `{expected}`, got value {v:?}"),
        }
    }
}

const ARR3: &str = "(list->array (list 10 20 30))";

// ---- FETCH ----------------------------------------------------------------

#[test]
fn fetch_out_of_bounds_errors_in_evaluator() {
    let out = eval_line(&format!("(fetch {ARR3} 5)"), &Environment::with_stdlib());
    assert_eq!(out, "Error: fetch: index 5 out of bounds (length 3)");
}

#[test]
fn fetch_out_of_bounds_parity_all_tiers() {
    // Evaluator source of truth.
    let ev = eval_line(&format!("(fetch {ARR3} 5)"), &Environment::with_stdlib());
    assert_eq!(ev, "Error: fetch: index 5 out of bounds (length 3)");
    // Every typed tier: the same message, minus the CLI "Error: " prefix.
    let j = jit_with(&["(defun-typed (getit int64) ((a (array int64)) (i int64)) (fetch a i))"]);
    assert_all_tiers_error(
        &j,
        "getit",
        &[
            Value::Array(vec![Value::Int(10), Value::Int(20), Value::Int(30)]),
            Value::Int(5),
        ],
        "fetch: index 5 out of bounds (length 3)",
    );
}

#[test]
fn fetch_negative_index_parity_all_tiers() {
    let ev = eval_line(&format!("(fetch {ARR3} -1)"), &Environment::with_stdlib());
    assert_eq!(
        ev,
        "Error: FETCH: index must be a non-negative integer, got -1"
    );
    let j = jit_with(&["(defun-typed (getit int64) ((a (array int64)) (i int64)) (fetch a i))"]);
    assert_all_tiers_error(
        &j,
        "getit",
        &[
            Value::Array(vec![Value::Int(10), Value::Int(20), Value::Int(30)]),
            Value::Int(-1),
        ],
        "FETCH: index must be a non-negative integer, got -1",
    );
}

// ---- STORE ----------------------------------------------------------------

#[test]
fn store_out_of_bounds_parity_all_tiers() {
    let ev = eval_line(&format!("(store {ARR3} 5 99)"), &Environment::with_stdlib());
    assert_eq!(ev, "Error: store: index 5 out of bounds (length 3)");
    let j = jit_with(&[
        "(defun-typed (setit int64) ((a (array int64)) (i int64) (v int64)) (store a i v))",
    ]);
    assert_all_tiers_error(
        &j,
        "setit",
        &[
            Value::Array(vec![Value::Int(10), Value::Int(20), Value::Int(30)]),
            Value::Int(5),
            Value::Int(99),
        ],
        "store: index 5 out of bounds (length 3)",
    );
}

#[test]
fn store_negative_index_parity_all_tiers() {
    let ev = eval_line(
        &format!("(store {ARR3} -3 99)"),
        &Environment::with_stdlib(),
    );
    assert_eq!(
        ev,
        "Error: STORE: index must be a non-negative integer, got -3"
    );
    let j = jit_with(&[
        "(defun-typed (setit int64) ((a (array int64)) (i int64) (v int64)) (store a i v))",
    ]);
    assert_all_tiers_error(
        &j,
        "setit",
        &[
            Value::Array(vec![Value::Int(10), Value::Int(20), Value::Int(30)]),
            Value::Int(-3),
            Value::Int(99),
        ],
        "STORE: index must be a non-negative integer, got -3",
    );
}

// ---- In-bounds access is unaffected ---------------------------------------

#[test]
fn in_bounds_access_still_works_all_tiers() {
    let j = jit_with(&["(defun-typed (getit int64) ((a (array int64)) (i int64)) (fetch a i))"]);
    let args = [
        Value::Array(vec![Value::Int(10), Value::Int(20), Value::Int(30)]),
        Value::Int(1),
    ];
    for r in call_all_tiers(&j, "getit", &args) {
        assert_eq!(
            r.unwrap(),
            Value::Int(20),
            "in-bounds fetch must still work"
        );
    }
}

// ---- End-to-end through the membrane (matches the evaluator verbatim) ------

#[test]
fn typed_membrane_fetch_matches_evaluator_string() {
    // The exact same surface program, one as a plain evaluator call and one
    // through the typed membrane, must produce byte-identical output.
    let env_ev = Environment::with_stdlib();
    let ev = eval_line(&format!("(fetch {ARR3} 9)"), &env_ev);

    let env_ty = Environment::with_stdlib();
    eval_line(
        "(defun-typed (getit int64) ((a (array int64)) (i int64)) (fetch a i))",
        &env_ty,
    );
    let ty = eval_line(&format!("(getit {ARR3} 9)"), &env_ty);
    assert_eq!(
        ev, ty,
        "typed membrane must match the evaluator's fetch error"
    );
    assert_eq!(ev, "Error: fetch: index 9 out of bounds (length 3)");
}
