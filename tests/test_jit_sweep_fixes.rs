//! Regression tests for the typed-JIT bug sweep: each case compares the
//! compiled (`defun-typed`) result with what the evaluator produces for the
//! same operation, or with the documented contract.

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line};

fn env() -> Shared<Environment> {
    Environment::with_stdlib()
}

/// Unary `-` on a `float64` is IEEE negation: `(- 0.0)` is `-0.0`, exactly as
/// the evaluator's `(- 0.0)`. The compiled form used to be `0 - x`, which
/// loses the sign of zero (`0.0 - 0.0` is `+0.0`).
#[test]
fn float_unary_minus_keeps_the_sign_of_zero() {
    let e = env();
    eval_line("(defun-typed (neg float64) ((x float64)) (- x))", &e);
    for x in ["0.0", "-0.0", "1.5", "-1.5"] {
        let compiled = eval_line(&format!("(neg {x})"), &e);
        let evaluator = eval_line(&format!("(- {x})"), &e);
        assert_eq!(compiled, evaluator, "(- {x}): compiled vs evaluator");
    }
    assert_eq!(eval_line("(neg 0.0)", &e), "-0.0");
    assert_eq!(eval_line("(neg -0.0)", &e), "0.0");
}

fn is_error(out: &str) -> bool {
    out.starts_with("Error")
}

/// A redefinition the membrane rejects binds nothing (docs/typed-jit-design.md
/// §4a: "an ill-typed definition is rejected before it binds anything"). It
/// used to overwrite the registered signature while keeping the old body, so
/// `(inl 3)` reinterpreted the old integer result as a float
/// (`0.000…0004`) and `see-type` reported the rejected signature.
#[test]
fn rejected_redefinition_keeps_the_previous_signature() {
    let e = env();
    eval_line("(defun-typed (inl int64) ((x int64)) (+ x 1))", &e);
    eval_line("(defun-typed (use int64) ((x int64)) (* (inl x) 2))", &e);
    assert_eq!(eval_line("(inl 3)", &e), "4");

    let rejected = eval_line("(defun-typed (inl float64) ((x int64)) (+ x \"a\"))", &e);
    assert!(is_error(&rejected), "ill-typed body must be rejected");
    assert_eq!(eval_line("(inl 3)", &e), "4");
    assert_eq!(eval_line("(use 3)", &e), "8");
    assert_eq!(
        eval_line("(see-type 'inl)", &e),
        "(TYPED (-> (INT64) INT64) COMPILED)"
    );

    let rejected = eval_line("(defun-typed (inl int64) ((x float64)) (+ x \"a\"))", &e);
    assert!(is_error(&rejected), "ill-typed body must be rejected");
    assert_eq!(eval_line("(inl 3)", &e), "4");
    assert_eq!(
        eval_line("(see-type 'inl)", &e),
        "(TYPED (-> (INT64) INT64) COMPILED)"
    );
}

/// The same, when the rejected redefinition also changes the arity: the
/// previous definition must still be callable and still compiled.
#[test]
fn rejected_arity_changing_redefinition_keeps_the_previous_definition() {
    let e = env();
    eval_line("(defun-typed (base int64) ((x int64)) (+ x 1))", &e);
    eval_line("(defun-typed (caller int64) ((x int64)) (base x))", &e);
    let rejected = eval_line(
        "(defun-typed (base int64) ((x int64) (y int64)) (+ x \"a\"))",
        &e,
    );
    assert!(is_error(&rejected), "ill-typed body must be rejected");
    assert_eq!(eval_line("(base 3)", &e), "4");
    assert_eq!(eval_line("(caller 3)", &e), "4");
    assert_eq!(
        eval_line("(see-type 'base)", &e),
        "(TYPED (-> (INT64) INT64) COMPILED)"
    );
}
/// Redefining a typed function with the same arity but different parameter or
/// return *types* leaves compiled callers holding the old ABI: a caller built
/// against `inl : int64 -> int64` read the new `float64` result word as an
/// integer (`(use 3)` printed 9218868437227405312). Callers of a function
/// whose signature changed must stop running instead (docs/typed-jit-design.md
/// §3.5); redefining the caller against the new signature revives it.
#[test]
fn return_type_change_invalidates_compiled_callers() {
    let e = env();
    eval_line("(defun-typed (inl int64) ((x int64)) (+ x 1))", &e);
    eval_line("(defun-typed (use int64) ((x int64)) (* (inl x) 2))", &e);
    eval_line("(defun-typed (top int64) ((x int64)) (+ (use x) 1))", &e);
    assert_eq!(eval_line("(use 3)", &e), "8");
    assert_eq!(eval_line("(top 3)", &e), "9");

    eval_line("(defun-typed (inl float64) ((x int64)) 1.5)", &e);
    let stale = eval_line("(use 3)", &e);
    assert!(is_error(&stale), "stale caller must error, got: {stale}");
    assert!(
        stale.contains("INL") && stale.contains("signature"),
        "error should name the redefined callee: {stale}"
    );
    let stale_top = eval_line("(top 3)", &e);
    assert!(is_error(&stale_top), "transitive caller: {stale_top}");
    assert_eq!(eval_line("(inl 3)", &e), "1.5");

    eval_line(
        "(defun-typed (use float64) ((x int64)) (* (inl x) 2.0))",
        &e,
    );
    assert_eq!(eval_line("(use 3)", &e), "3.0");
}

#[test]
fn parameter_type_change_invalidates_compiled_callers() {
    let e = env();
    eval_line("(defun-typed (inl int64) ((x int64)) (+ x 1))", &e);
    eval_line("(defun-typed (use int64) ((x int64)) (inl x))", &e);
    assert_eq!(eval_line("(use 41)", &e), "42");

    eval_line("(defun-typed (inl int64) ((x float64)) 7)", &e);
    let stale = eval_line("(use 41)", &e);
    assert!(is_error(&stale), "stale caller must error, got: {stale}");
    assert_eq!(eval_line("(inl 1.5)", &e), "7");
}

/// A redefinition that keeps the signature leaves callers valid and live.
#[test]
fn same_signature_redefinition_keeps_callers() {
    let e = env();
    eval_line("(defun-typed (inl int64) ((x int64)) (+ x 1))", &e);
    eval_line("(defun-typed (use int64) ((x int64)) (* (inl x) 2))", &e);
    eval_line("(defun-typed (inl int64) ((x int64)) (+ x 100))", &e);
    assert_eq!(eval_line("(use 3)", &e), "206");
}

/// An unannotated `defun` that was compiled against the old callee signature
/// keeps working: its auto-typed membrane declines and the dynamic closure
/// runs against the callee's new behavior.
#[test]
fn auto_typed_caller_falls_back_to_its_dynamic_closure() {
    let e = env();
    eval_line("(defun-typed (inl int64) ((x int64)) (+ x 1))", &e);
    eval_line("(defun auto (x) (* (inl x) 2))", &e);
    assert_eq!(eval_line("(auto 3)", &e), "8");
    eval_line("(defun-typed (inl float64) ((x int64)) 1.5)", &e);
    assert_eq!(eval_line("(auto 3)", &e), "3.0");
}
