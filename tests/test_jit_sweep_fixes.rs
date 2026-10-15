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
