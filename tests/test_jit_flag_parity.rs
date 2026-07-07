//! Differential OVERFLOW / division-error parity tests between the
//! tree-walking evaluator and the typed JIT membrane (issue #210).
//!
//! The brutal_correctness suite compares result *values* across tiers but its
//! oracle bakes wrapping arithmetic and div-by-zero-is-0 into itself, so the
//! OVERFLOW flag and the Division-by-zero error contract were unchecked for
//! parity. Each test here runs the same arithmetic boundary case through (1)
//! the plain evaluator and (2) an equivalent `defun-typed` call, and asserts
//! the observable outcome — the `OVERFLOW` flag via `FLAG-SET-P`, the wrapped
//! result value, and error-vs-value behavior — is identical.

use lamedh::environment::Environment;
use lamedh::eval_line;

/// Evaluate `expr` in a fresh env, returning (result-or-error-string,
/// overflow-flag-afterwards). `expr` must be a single form.
fn run_evaluator(expr: &str) -> (String, bool) {
    let env = Environment::new_with_builtins();
    eval_line("(clear-all-flags)", &env);
    let result = eval_line(expr, &env);
    let flag = eval_line("(flag-set-p 'OVERFLOW)", &env);
    (result, flag == "T")
}

/// Define `defun-typed` form(s), then call `call`, returning the same
/// (result-or-error-string, overflow-flag) pair through the typed membrane.
fn run_typed(defs: &[&str], call: &str) -> (String, bool) {
    let env = Environment::new_with_builtins();
    for d in defs {
        let out = eval_line(d, &env);
        assert!(
            !out.to_lowercase().contains("error"),
            "typed definition failed: {d} -> {out}"
        );
    }
    eval_line("(clear-all-flags)", &env);
    let result = eval_line(call, &env);
    let flag = eval_line("(flag-set-p 'OVERFLOW)", &env);
    (result, flag == "T")
}

const MAX: &str = "9223372036854775807";
const MIN: &str = "-9223372036854775808";

#[test]
fn overflow_flag_parity_on_addition() {
    let (ev_val, ev_flag) = run_evaluator(&format!("(+ {MAX} 1)"));
    let (ty_val, ty_flag) = run_typed(
        &["(defun-typed (tadd int64) ((x int64)) (+ x 1))"],
        &format!("(tadd {MAX})"),
    );
    assert_eq!(ev_flag, ty_flag, "OVERFLOW flag parity on MAX+1");
    assert!(ev_flag, "evaluator must set OVERFLOW on MAX+1");
    assert_eq!(ev_val, ty_val, "wrapped value parity on MAX+1");
}

#[test]
fn overflow_flag_parity_on_subtraction() {
    let (ev_val, ev_flag) = run_evaluator(&format!("(- {MIN} 1)"));
    let (ty_val, ty_flag) = run_typed(
        &["(defun-typed (tsub int64) ((x int64)) (- x 1))"],
        &format!("(tsub {MIN})"),
    );
    assert_eq!(ev_flag, ty_flag, "OVERFLOW flag parity on MIN-1");
    assert!(ev_flag, "evaluator must set OVERFLOW on MIN-1");
    assert_eq!(ev_val, ty_val, "wrapped value parity on MIN-1");
}

#[test]
fn overflow_flag_parity_on_multiplication() {
    let (ev_val, ev_flag) = run_evaluator(&format!("(* {MAX} 2)"));
    let (ty_val, ty_flag) = run_typed(
        &["(defun-typed (tmul int64) ((x int64)) (* x 2))"],
        &format!("(tmul {MAX})"),
    );
    assert_eq!(ev_flag, ty_flag, "OVERFLOW flag parity on MAX*2");
    assert!(ev_flag, "evaluator must set OVERFLOW on MAX*2");
    assert_eq!(ev_val, ty_val, "wrapped value parity on MAX*2");
}

#[test]
fn overflow_flag_parity_on_min_div_negative_one() {
    let (ev_val, ev_flag) = run_evaluator(&format!("(/ {MIN} -1)"));
    let (ty_val, ty_flag) = run_typed(
        &["(defun-typed (tdiv int64) ((x int64) (y int64)) (/ x y))"],
        &format!("(tdiv {MIN} -1)"),
    );
    assert_eq!(ev_flag, ty_flag, "OVERFLOW flag parity on MIN/-1");
    assert!(ev_flag, "evaluator must set OVERFLOW on MIN/-1");
    assert_eq!(ev_val, ty_val, "wrapped value parity on MIN/-1");
}

#[test]
fn overflow_flag_parity_on_min_mod_negative_one() {
    // MOD is Euclidean in both worlds (#280): MIN % -1 is exactly 0 and
    // sets NO flag — the OVERFLOW flag #228/#268 gave the typed MOD had
    // assumed the truncated-remainder semantics of REMAINDER.
    let (ev_val, ev_flag) = run_evaluator(&format!("(mod {MIN} -1)"));
    let (ty_val, ty_flag) = run_typed(
        &["(defun-typed (tmod int64) ((x int64) (y int64)) (mod x y))"],
        &format!("(tmod {MIN} -1)"),
    );
    assert_eq!(ev_flag, ty_flag, "OVERFLOW flag parity on MIN%-1");
    assert!(
        !ev_flag,
        "MIN%-1 is exactly 0 under Euclidean MOD — no flag"
    );
    assert_eq!(ev_val, ty_val, "value parity on MIN%-1");
    assert_eq!(ev_val, "0");
}

#[test]
fn mod_value_parity_on_negative_operands() {
    let (ev_val, ev_flag) = run_evaluator("(mod -7 3)");
    let (ty_val, ty_flag) = run_typed(
        &["(defun-typed (tmod int64) ((x int64) (y int64)) (mod x y))"],
        "(tmod -7 3)",
    );
    assert_eq!(ev_val, ty_val, "MOD value parity on (-7, 3)");
    assert_eq!(ev_val, "2", "evaluator MOD is Euclidean");
    assert_eq!(ev_flag, ty_flag, "no flags on ordinary mod");
}

#[test]
fn division_by_zero_errors_in_both_worlds() {
    let (ev_val, _) = run_evaluator("(/ 1 0)");
    let (ty_val, _) = run_typed(
        &["(defun-typed (tdiv int64) ((x int64) (y int64)) (/ x y))"],
        "(tdiv 1 0)",
    );
    assert!(
        ev_val.contains("Division by zero"),
        "evaluator div-by-zero must error, got: {ev_val}"
    );
    assert!(
        ty_val.contains("Division by zero"),
        "typed div-by-zero must error identically, got: {ty_val}"
    );
}

#[test]
fn mod_by_zero_errors_in_both_worlds() {
    let (ev_val, _) = run_evaluator("(mod 1 0)");
    let (ty_val, _) = run_typed(
        &["(defun-typed (tmod int64) ((x int64) (y int64)) (mod x y))"],
        "(tmod 1 0)",
    );
    assert!(
        ev_val.to_lowercase().contains("zero"),
        "evaluator mod-by-zero must error, got: {ev_val}"
    );
    assert!(
        ty_val.to_lowercase().contains("zero"),
        "typed mod-by-zero must error identically, got: {ty_val}"
    );
}

#[test]
fn no_flag_on_clean_typed_arithmetic() {
    // The typed membrane must not leak spurious flags for non-overflowing work.
    let (ty_val, ty_flag) = run_typed(
        &["(defun-typed (tclean int64) ((x int64)) (+ (* x 2) 1))"],
        "(tclean 20)",
    );
    assert_eq!(ty_val, "41");
    assert!(!ty_flag, "clean typed arithmetic must not set OVERFLOW");
}

#[test]
fn overflow_flag_parity_before_div_error() {
    // Issue #268: when a single call both overflows and divides by zero, the
    // OVERFLOW flag must be recorded before the division error surfaces —
    // in both worlds.
    let (ev_val, ev_flag) = run_evaluator(&format!("(/ (+ {MAX} 1) 0)"));
    let (ty_val, ty_flag) = run_typed(
        &["(defun-typed (tboth int64) ((x int64)) (/ (+ x 1) 0))"],
        &format!("(tboth {MAX})"),
    );
    assert!(
        ev_val.contains("Division by zero") && ty_val.contains("Division by zero"),
        "both worlds must raise the division error (ev: {ev_val}, ty: {ty_val})"
    );
    assert_eq!(
        ev_flag, ty_flag,
        "OVERFLOW-before-div-error ordering parity"
    );
    assert!(ev_flag, "evaluator must keep the OVERFLOW flag set");
}
