//! Issue #530: an integer LITERAL in a `float64` position elaborates as the
//! float constant `n as f64` — literal-only coercion, no general int→float
//! subtyping — in the native elaborator and the portable checker
//! (lib/46-hm-check.lisp) alike.
//!
//! The coercion is taken only where it is invisible: the evaluator's mixed
//! arithmetic (`apply_math_op`) and comparison (`as_f64`) promote every int
//! operand with `as f64`, so a compiled `(+ x 1)` is bit-for-bit the
//! interpreter's. `min`/`max` are NOT such a position: they return the
//! selected argument itself, so the interpreter's `(max 0.5 1)` is the
//! integer `1`; a float constant would change the observable result, so the
//! literal stays int64 there and the clash says why.
#![cfg(feature = "jit")]

use lamedh::environment::Environment;
use lamedh::{Shared, eval_str};

fn ev(e: &Shared<Environment>, src: &str) -> String {
    match eval_str(src, e) {
        Ok(v) => lamedh::printer::print(&v),
        Err(err) => format!("ERR: {err}"),
    }
}

/// Every body compiles NATIVE under a float64 parameter, and its result on
/// each input equals the interpreter evaluating the same body directly.
#[test]
fn int_literals_in_float_arithmetic_and_comparison_compile_and_match_the_interpreter() {
    lamedh::with_large_stack(|| {
        let e = Environment::with_stdlib();
        let cases: &[(&str, &str)] = &[
            ("float64", "(+ x 1)"),
            ("float64", "(+ 1 x)"),
            ("float64", "(+ 1 2 x)"),
            ("float64", "(+ x 1 2)"),
            ("float64", "(* 2 x 3)"),
            ("float64", "(- x 1)"),
            ("float64", "(- 1 x)"),
            ("float64", "(/ x 2)"),
            ("float64", "(/ 1 x)"),
            ("float64", "(/ x 0)"),
            ("float64", "(+ x 9007199254740993)"),
            ("bool", "(< x 1)"),
            ("bool", "(>= 0 x)"),
            ("bool", "(= x 9007199254740993)"),
            ("bool", "(/= 1 x)"),
        ];
        let inputs = ["0.5", "-2.5", "1.0", "0.0", "9007199254740992.0"];
        for (i, (ret, body)) in cases.iter().enumerate() {
            let name = format!("lit530-{i}");
            let def = format!("(defun-typed ({name} {ret}) ((x float64)) {body})");
            let r = ev(&e, &def);
            assert!(!r.starts_with("ERR"), "{def} => {r}");
            assert_eq!(ev(&e, &format!("(compiled-p '{name})")), "NATIVE", "{def}");
            for x in inputs {
                assert_eq!(
                    ev(&e, &format!("({name} {x})")),
                    ev(&e, &format!("(let ((x {x})) {body})")),
                    "{body} at x = {x}"
                );
            }
        }
    });
}

/// Only a literal is coerced: an int64 variable meeting a float stays a clash.
/// And only where the evaluator's arithmetic is the compiled fold: its N-ary
/// `-` is `a - (b + c)`, so `-` coerces at arity 2 only; `mod` never.
#[test]
fn coercion_is_literal_only_and_only_where_it_is_invisible() {
    lamedh::with_large_stack(|| {
        let e = Environment::with_stdlib();
        for def in [
            "(defun-typed (n530a float64) ((x float64) (n int64)) (+ x n))",
            "(defun-typed (n530b bool) ((x float64) (n int64)) (< x n))",
            "(defun-typed (n530c float64) ((x float64)) (- x 1 2))",
            "(defun-typed (n530d float64) ((x float64)) (mod x 2))",
            "(defun-typed (n530e float64) ((x float64)) (if (< x 0.0) 0 x))",
        ] {
            let r = ev(&e, def);
            assert!(r.starts_with("ERR"), "{def} should stay rejected, got {r}");
        }
        // Int-only arithmetic is untouched.
        ev(&e, "(defun-typed (n530f int64) ((n int64)) (+ n 1))");
        assert_eq!(ev(&e, "(n530f 41)"), "42");
    });
}

/// `(max x 1)`: the interpreter returns the integer literal itself whenever
/// it is the larger, so coercing it would change the result. It stays a
/// clash, and the message says why and what to write instead.
#[test]
fn min_max_do_not_coerce_because_the_interpreter_returns_the_int() {
    lamedh::with_large_stack(|| {
        let e = Environment::with_stdlib();
        assert_eq!(ev(&e, "(max 0.5 1)"), "1");
        assert_eq!(ev(&e, "(min 2.5 1)"), "1");
        let r = ev(&e, "(defun-typed (m530 float64) ((x float64)) (max x 1))");
        assert!(r.starts_with("ERR"), "{r}");
        assert!(r.contains("integer literal is not coerced"), "{r}");
        // The float spelling compiles.
        ev(
            &e,
            "(defun-typed (m530b float64) ((x float64)) (max x 1.0))",
        );
        assert_eq!(ev(&e, "(compiled-p 'm530b)"), "NATIVE");
        assert_eq!(ev(&e, "(m530b 0.5)"), "1.0");
    });
}

/// The portable checker (lib/46-hm-check.lisp) mirrors the kernel, in
/// checking mode and in the codegen-mode gate the typed island runs.
#[test]
fn portable_checker_mirrors_the_literal_coercion() {
    lamedh::with_large_stack(|| {
        let e = Environment::with_stdlib();
        assert_eq!(ev(&e, "(hm-check-expr '(+ 1.5 1))"), "(CHECKED FLOAT64)");
        assert_eq!(ev(&e, "(hm-check-expr '(+ 1 2 2.5))"), "(CHECKED FLOAT64)");
        assert_eq!(ev(&e, "(hm-check-expr '(/ 1 2.5))"), "(CHECKED FLOAT64)");
        assert_eq!(ev(&e, "(hm-check-expr '(< 1 2.5))"), "(CHECKED BOOL)");
        assert_eq!(ev(&e, "(hm-check-expr '(+ 1 2))"), "(CHECKED INT64)");
        assert!(ev(&e, "(car (hm-check-expr '(mod 2.5 1)))").contains("TYPE-ERROR"));
        assert!(ev(&e, "(car (hm-check-expr '(- 1 2 2.5)))").contains("TYPE-ERROR"));

        // Codegen-mode gate: the island takes a coerced body whole, with the
        // same signature the kernel gives it.
        ev(&e, "(defun p530 (x) (* 2 (+ x 0.5)))");
        ev(&e, "(defun q530 (x) (< 1 (+ x 0.5)))");
        assert_eq!(
            ev(
                &e,
                "(mapcar #'cadr (cdr (assoc 'members (typed-island '(p530 q530)))))"
            ),
            "((-> (FLOAT64) FLOAT64) (-> (FLOAT64) BOOL))"
        );
        assert_eq!(
            ev(&e, "(cdr (assoc 'rejected (typed-island '(p530 q530))))"),
            "()"
        );
        // min/max: blocked in the gate too, with the reason.
        ev(&e, "(defun r530 (x) (max (+ x 0.5) 1))");
        let v = ev(&e, "(hm-compile-verdict 'r530)");
        assert!(v.starts_with("(BLOCKED"), "{v}");
        assert!(v.contains("integer literal is not coerced"), "{v}");
    });
}

/// `quotient` is the evaluator's name for `/` (BinOp::Div natively), so an
/// int literal beside a float64 operand coerces at arity 2 in the portable
/// gate exactly as it does for `/` — the typed island, the native checker
/// and the interpreter agree on both operand orders.
#[test]
fn quotient_coerces_int_literals_like_divide() {
    lamedh::with_large_stack(|| {
        let e = Environment::with_stdlib();
        let bodies = ["(quotient (+ x 0.5) 2)", "(quotient 2 (+ x 0.5))"];
        let inputs = ["0.5", "-2.5", "1.0", "0.0"];
        for (i, body) in bodies.iter().enumerate() {
            // Portable codegen-mode gate.
            let g = format!("g530q{i}");
            ev(&e, &format!("(defun {g} (x) {body})"));
            assert_eq!(
                ev(
                    &e,
                    &format!("(mapcar #'cadr (cdr (assoc 'members (typed-island '({g})))))")
                ),
                "((-> (FLOAT64) FLOAT64))",
                "{body}"
            );
            assert_eq!(
                ev(
                    &e,
                    &format!("(cdr (assoc 'rejected (typed-island '({g}))))")
                ),
                "()",
                "{body}"
            );
            // Native checker compiles the same body.
            let n = format!("n530q{i}");
            let def = format!("(defun-typed ({n} float64) ((x float64)) {body})");
            let r = ev(&e, &def);
            assert!(!r.starts_with("ERR"), "{def} => {r}");
            assert_eq!(ev(&e, &format!("(compiled-p '{n})")), "NATIVE", "{def}");
            for x in inputs {
                let want = ev(&e, &format!("(let ((x {x})) {body})"));
                assert_eq!(ev(&e, &format!("({n} {x})")), want, "{body} at x = {x}");
                assert_eq!(ev(&e, &format!("({g} {x})")), want, "{body} at x = {x}");
            }
        }
    });
}
