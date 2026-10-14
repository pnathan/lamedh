//! Issue #530: an integer LITERAL in a `float64` position elaborates as the
//! float constant `n as f64` — literal-only coercion, no general int→float
//! subtyping — in the native elaborator and the portable checker
//! (lib/46-hm-check.lisp) alike.
//!
//! The coercion is taken only where it is invisible: the evaluator's mixed
//! arithmetic (`apply_math_op`) and comparison (`as_f64`) promote every int
//! operand with `as f64`, so a compiled `(+ x 1)` is bit-for-bit the
//! interpreter's. `min`/`max` are `a -> a -> a` over one numeric `a`: the
//! interpreter returns a float whenever any argument is a float (`(max 0.5 1)`
//! is `1.0`), so the coercion is invisible there too.
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

/// `min`/`max` are `a -> a -> a` over one numeric `a`: when any argument is a
/// float the result is a float, so an int literal beside a float64 is
/// coerced (as for `+`) and the compiled result is the interpreter's.
#[test]
fn min_max_promote_to_float_and_coerce_int_literals() {
    lamedh::with_large_stack(|| {
        let e = Environment::with_stdlib();
        // Interpreter: the result type is the common numeric type.
        assert_eq!(ev(&e, "(max 0.5 1)"), "1.0");
        assert_eq!(ev(&e, "(min 2.5 1)"), "1.0");
        assert_eq!(ev(&e, "(max 1 0.5)"), "1.0");
        assert_eq!(ev(&e, "(max 0.5 -1)"), "0.5");
        assert_eq!(ev(&e, "(min 3 2 1.5)"), "1.5");
        assert_eq!(ev(&e, "(max 3 2 1.5)"), "3.0");
        assert_eq!(ev(&e, "(max 1 1.0)"), "1.0");
        // All-int stays int; all-float stays float (incl. -0.0).
        assert_eq!(ev(&e, "(min 1 1.0)"), "1.0");
        assert_eq!(ev(&e, "(max 1.0 1)"), "1.0");
        assert_eq!(ev(&e, "(max 1 2)"), "2");
        assert_eq!(ev(&e, "(min 7)"), "7");
        assert_eq!(ev(&e, "(max -0.0 -0.0)"), "-0.0");
        // Compiled == interpreted, both literal positions and variadic.
        let bodies = [
            "(max x 1)",
            "(min x 1)",
            "(max 1 x)",
            "(min 1 x)",
            "(max x 1 2)",
            "(min 3 x 2)",
            "(max (+ x 0.5) 1)",
            "(max x 9007199254740993)",
            "(min x 0)",
            "(max x (max 1 2.5))",
        ];
        let inputs = [
            "0.5",
            "-2.5",
            "1.0",
            "0.0",
            "-0.0",
            "7.25",
            "9007199254740992.0",
        ];
        for (i, body) in bodies.iter().enumerate() {
            let n = format!("m530_{i}");
            let def = format!("(defun-typed ({n} float64) ((x float64)) {body})");
            let r = ev(&e, &def);
            assert!(!r.starts_with("ERR"), "{def} => {r}");
            assert_eq!(ev(&e, &format!("(compiled-p '{n})")), "NATIVE", "{def}");
            for x in inputs {
                let want = ev(&e, &format!("(let ((x {x})) {body})"));
                assert_eq!(ev(&e, &format!("({n} {x})")), want, "{body} at x = {x}");
            }
        }
        assert_eq!(ev(&e, "(m530_0 0.5)"), "1.0");
        // Still literal-only: an int VARIABLE beside a float64 is a clash.
        for def in [
            "(defun-typed (bad530a float64) ((x float64) (n int64)) (max x n))",
            "(defun-typed (bad530b float64) ((x float64) (n int64)) (min n x))",
            "(defun-typed (bad530c float64) ((x float64) (n int64)) (max x 1 n))",
            // only a bare literal is coerced, not an int expression
            "(defun-typed (bad530d float64) ((x float64)) (max x (+ 1 2)))",
        ] {
            let r = ev(&e, def);
            assert!(r.starts_with("ERR"), "{def} should stay rejected, got {r}");
        }
        // Int-only min/max is untouched.
        ev(&e, "(defun-typed (i530 int64) ((n int64)) (max n 1))");
        assert_eq!(ev(&e, "(i530 -5)"), "1");
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
        // min/max: the same coercion, in checking mode and in the gate.
        assert_eq!(ev(&e, "(hm-check-expr '(max 1.5 1))"), "(CHECKED FLOAT64)");
        assert_eq!(
            ev(&e, "(hm-check-expr '(min 1 2 2.5))"),
            "(CHECKED FLOAT64)"
        );
        assert_eq!(ev(&e, "(hm-check-expr '(max 1 2))"), "(CHECKED INT64)");
        ev(&e, "(defun r530 (x) (max (+ x 0.5) 1))");
        ev(&e, "(defun t530 (x) (min 1 x))");
        assert_eq!(
            ev(
                &e,
                "(mapcar #'cadr (cdr (assoc 'members (typed-island '(r530)))))"
            ),
            "((-> (FLOAT64) FLOAT64))"
        );
        // With nothing pinning x to float64, the literal is just an int.
        assert_eq!(
            ev(
                &e,
                "(mapcar #'cadr (cdr (assoc 'members (typed-island '(t530)))))"
            ),
            "((-> (INT64) INT64))"
        );
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

/// `defun*` infers the signature: the float use of `x` pins it to float64
/// before the `max`, so the literal coerces and the body compiles. The
/// reverse order (int use first, then a float use) is order-dependent: the
/// int pin makes the float clash, and `defun*` falls back silently to the
/// dynamic lambda with the same value.
#[test]
fn defun_star_coerces_the_literal_when_the_float_use_comes_first() {
    lamedh::with_large_stack(|| {
        let e = Environment::with_stdlib();
        ev(&e, "(defun* f530 (x) (max (* x 1.0) 1))");
        assert_eq!(ev(&e, "(cadr (see-type 'f530))"), "(-> (FLOAT64) FLOAT64)");
        assert_eq!(ev(&e, "(car (see-type 'f530))"), "TYPED");
        for x in ["0.5", "-2.5", "0.0"] {
            let want = ev(&e, &format!("(let ((x {x})) (max (* x 1.0) 1))"));
            assert_eq!(ev(&e, &format!("(f530 {x})")), want, "x = {x}");
        }
        // Order-dependent fallback: the value is still the interpreter's.
        ev(&e, "(defun* h530 (x) (+ (max x 1) (* x 0.5)))");
        assert_ne!(ev(&e, "(car (see-type 'h530))"), "TYPED");
        assert_eq!(ev(&e, "(h530 0.5)"), "1.25");
    });
}

/// The typed island takes the coerced min/max bodies whole, and the kernel
/// reads back the same signature for every member (AGREE, nothing disputed).
#[test]
fn island_install_agrees_with_the_kernel_on_coerced_min_max() {
    lamedh::with_large_stack(|| {
        let e = Environment::with_stdlib();
        ev(&e, "(defun i530a (x) (max (+ x 0.5) 1))");
        ev(&e, "(defun i530b (x) (min 1 (* x 2.0)))");
        ev(&e, "(def isl530 (typed-island '(i530a i530b)))");
        ev(&e, "(def rep530 (island-install! isl530 'strict))");
        assert_eq!(ev(&e, "(island-agreement rep530)"), "((I530A I530B))");
        assert_eq!(ev(&e, "(cdr (island-agreement rep530))"), "()");
        assert_eq!(ev(&e, "(car (see-type 'i530a))"), "TYPED");
        assert_eq!(ev(&e, "(i530a 0.5)"), "1.0");
        assert_eq!(ev(&e, "(i530b 0.25)"), "0.5");
    });
}
