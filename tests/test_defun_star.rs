//! `defun*` — unified smart function definition — and `check-type` on
//! arbitrary expressions.
//!
//! `defun*` tries HM type inference (compiling natively when the body is a
//! typed island) and falls back to an ordinary lambda otherwise. The fallback
//! is transparent, so every case below must still *run* correctly regardless of
//! whether it compiled. Inference notes are emitted on stderr, so they do not
//! appear in `eval_line`'s returned (stdout/value) string.

mod test_helpers;
use lamedh::eval_line;
use test_helpers::env_with_stdlib;

#[test]
fn defun_star_infers_fully_untyped() {
    let env = env_with_stdlib();
    eval_line("(defun* sq (x) (* x x))", &env);
    assert_eq!(eval_line("(sq 9)", &env), "81");
}

#[test]
fn defun_star_bare_symbol_params() {
    let env = env_with_stdlib();
    eval_line("(defun* add3 a b c (+ a b c))", &env);
    assert_eq!(eval_line("(add3 1 2 3)", &env), "6");
}

#[test]
fn defun_star_with_typed_params() {
    let env = env_with_stdlib();
    eval_line("(defun* scale (x int64) (y int64) (* x y))", &env);
    assert_eq!(eval_line("(scale 6 7)", &env), "42");
}

#[test]
fn defun_star_mixed_typed_and_inferred() {
    let env = env_with_stdlib();
    // x pinned to int64, y inferred.
    eval_line("(defun* mix (x int64) (y) (+ x y))", &env);
    assert_eq!(eval_line("(mix 10 32)", &env), "42");
}

#[test]
fn defun_star_with_return_type() {
    let env = env_with_stdlib();
    eval_line("(defun* dot (x int64) (y int64) int64 (* x y))", &env);
    assert_eq!(eval_line("(dot 3 4)", &env), "12");
}

#[test]
fn defun_star_recursive_inference() {
    let env = env_with_stdlib();
    eval_line(
        "(defun* fib (n) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2)))))",
        &env,
    );
    assert_eq!(eval_line("(fib 10)", &env), "55");
}

#[test]
fn defun_star_falls_back_to_lambda_when_untyped() {
    let env = env_with_stdlib();
    // A body that touches untyped Lisp (string ops, cons) cannot compile, but
    // must still run as a plain lambda.
    eval_line("(defun* mk (a b) (cons a b))", &env);
    assert_eq!(eval_line("(mk 1 2)", &env), "(1 . 2)");
}

#[test]
fn defun_star_float_inference() {
    let env = env_with_stdlib();
    eval_line("(defun* addf (x) (+ x 1.0))", &env);
    assert_eq!(eval_line("(addf 2.5)", &env), "3.5");
}

// --- check-type on arbitrary expressions -----------------------------------

#[test]
fn check_type_of_integer_literal() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(check-type 10)", &env), "\"int64\"");
}

#[test]
fn check_type_of_float_literal() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(check-type 10.0)", &env), "\"float64\"");
}

#[test]
fn check_type_of_integer_arithmetic() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(check-type (+ 10 1))", &env), "\"int64\"");
}

#[test]
fn check_type_reports_a_mismatch() {
    let env = env_with_stdlib();
    let out = eval_line("(check-type (+ 10 1.0))", &env);
    assert!(out.contains("type error"), "got: {out}");
}

#[test]
fn check_type_still_looks_up_functions() {
    let env = env_with_stdlib();
    eval_line("(defun* inc (n) (+ n 1))", &env);
    let out = eval_line("(check-type inc)", &env);
    assert!(out.contains("int64"), "got: {out}");
}

#[test]
fn check_type_is_not_stale_after_redefinition() {
    // Regression: redefining a compiled typed function as an untyped one must
    // not leave check-type reporting the stale typed signature. The live value
    // cell (what a call runs) is authoritative.
    let env = env_with_stdlib();
    eval_line("(defun* f (x) (+ x 1))", &env);
    assert_eq!(eval_line("(f 5)", &env), "6");
    // Redefine as an untyped function returning a pair.
    eval_line("(defun* f (x) (cons x x))", &env);
    assert_eq!(eval_line("(f 5)", &env), "(5 . 5)");
    let out = eval_line("(check-type f)", &env);
    // Must NOT claim the old "(X int64) -> int64" signature.
    assert!(
        !out.contains("int64"),
        "stale typed signature leaked after redefinition: {out}"
    );
}

#[test]
fn defun_star_variadic_falls_back_to_lambda() {
    // A &rest parameter is outside the monomorphic typed core, so defun* must
    // fall back to an ordinary variadic lambda rather than mis-compiling.
    let env = env_with_stdlib();
    eval_line("(defun* f (a &rest b) (cons a b))", &env);
    assert_eq!(eval_line("(f 1 2 3)", &env), "(1 2 3)");
}

#[test]
fn defun_star_typed_array_param() {
    let env = env_with_stdlib();
    eval_line(
        "(defun* sum2 (a (array int64)) (+ (fetch a 0) (fetch a 1)))",
        &env,
    );
    assert_eq!(eval_line("(sum2 (list->array (list 10 20)))", &env), "30");
}

#[test]
fn defun_star_return_type_mismatch_falls_back_but_still_runs() {
    // Declared float64 return but the body is int64: cannot compile, but the
    // fallback lambda must still produce the right value.
    let env = env_with_stdlib();
    eval_line("(defun* f (x int64) float64 (+ x 1))", &env);
    assert_eq!(eval_line("(f 5)", &env), "6");
}

#[test]
fn defun_star_contradicted_pin_falls_back() {
    // x pinned int64 but used with a float literal: inference fails, lambda runs.
    let env = env_with_stdlib();
    eval_line("(defun* f (x int64) (+ x 1.0))", &env);
    assert_eq!(eval_line("(f 5)", &env), "6.0");
}
