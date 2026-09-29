//! #397: compiled `abs`/`min`/`max` evaluate each argument exactly once
//! (bound to a temp slot rather than cloned into the test and branches),
//! and `min`/`max` compile at any arity, folding like lib/05-math.lisp.

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line};

fn env() -> Shared<Environment> {
    Environment::with_stdlib()
}

fn compiled(e: &Shared<Environment>, name: &str) {
    let ex = eval_line(&format!("(explain-compile '{name})"), e);
    assert!(ex.starts_with("((TIER . COMPILED)"), "{name}: {ex}");
}

#[test]
fn abs_argument_runs_once() {
    let e = env();
    // The side-effecting argument bumps I once; recomputation would bump it 2-3x.
    let body = "(let ((i 0)) (+ (abs (progn (setq i (+ i n)) (- 0 i))) (* 100 i)))";
    eval_line(&format!("(defun ab-once (n) {body})"), &e);
    compiled(&e, "ab-once");
    for n in [-3, 0, 1, 7] {
        let want = eval_line(&format!("(let ((n {n})) {body})"), &e);
        assert_eq!(eval_line(&format!("(ab-once {n})"), &e), want, "n={n}");
    }
    assert_eq!(eval_line("(ab-once 1)", &e), "101");
}

#[test]
fn variadic_min_max_compile_and_run_each_argument_once() {
    let e = env();
    let body = "(let ((i 0)) (+ (max (progn (setq i (+ i 1)) n) 3 \
                (progn (setq i (+ i 10)) 2)) (* 100 i)))";
    eval_line(&format!("(defun mx-once (n) {body})"), &e);
    compiled(&e, "mx-once");
    for n in [1, 3, 7] {
        let want = eval_line(&format!("(let ((n {n})) {body})"), &e);
        assert_eq!(eval_line(&format!("(mx-once {n})"), &e), want, "n={n}");
    }
    assert_eq!(eval_line("(mx-once 7)", &e), "1107");
}

#[test]
fn variadic_min_max_match_the_interpreter() {
    let e = env();
    eval_line("(defun mm1 (a) (+ (max (+ a 0)) (min (+ a 0))))", &e);
    eval_line("(defun mx4 (a b c d) (max (+ a 0) b c d))", &e);
    eval_line("(defun mn4 (a b c d) (min (+ a 0) b c d))", &e);
    eval_line("(defun fmx3 (a b c) (max (+ a 0.0) b c))", &e);
    eval_line("(defun fmn3 (a b c) (min (+ a 0.0) b c))", &e);
    for f in ["mm1", "mx4", "mn4", "fmx3", "fmn3"] {
        compiled(&e, f);
    }
    assert_eq!(eval_line("(mm1 4)", &e), "8");
    for args in ["1 5 3 2", "9 -1 9 0", "-4 -4 -8 -2"] {
        for (f, g) in [("mx4", "max"), ("mn4", "min")] {
            assert_eq!(
                eval_line(&format!("({f} {args})"), &e),
                eval_line(&format!("({g} {args})"), &e),
                "{f} {args}"
            );
        }
    }
    // Signed zeros: the right fold's tie-breaking is observable. Reference:
    // the same body, interpreted.
    for args in [
        ["0.0", "-0.0", "0.0"],
        ["-0.0", "0.0", "-0.0"],
        ["1.5", "-2.5", "0.25"],
    ] {
        for (f, body) in [
            ("fmx3", "(max (+ a 0.0) b c)"),
            ("fmn3", "(min (+ a 0.0) b c)"),
        ] {
            let [a, b, c] = args;
            assert_eq!(
                eval_line(&format!("({f} {a} {b} {c})"), &e),
                eval_line(&format!("(let ((a {a}) (b {b}) (c {c})) {body})"), &e),
                "{f} {args:?}"
            );
        }
    }
}

#[test]
fn the_portable_gate_agrees_on_variadic_min_max() {
    let e = env();
    assert_eq!(
        eval_line("(hm-compile-lambda 'mx '(a b c) '((max (+ a 0) b c)))", &e),
        "(COMPILEABLE (-> (INT64 INT64 INT64) INT64))"
    );
    assert_eq!(
        eval_line("(hm-compile-lambda 'mx '(a) '((min (+ a 0.0))))", &e),
        "(COMPILEABLE (-> (FLOAT64) FLOAT64))"
    );
}

/// #518: `(abs -0.0)` is `0.0` in both tiers (-0.0 is not `minusp`, so a
/// bare comparison-select returned it unchanged).
#[test]
fn abs_of_negative_zero_is_positive_zero_in_both_tiers() {
    let e = env();
    // Compiled tier: the float path of `elab_abs`.
    eval_line("(defun-typed (fab float64) ((a float64)) (abs a))", &e);
    assert_eq!(
        eval_line("(see-type 'fab)", &e),
        "(TYPED (-> (FLOAT64) FLOAT64) COMPILED)"
    );
    for (arg, want) in [
        ("-0.0", "0.0"),
        ("0.0", "0.0"),
        ("-2.5", "2.5"),
        ("3.0", "3.0"),
    ] {
        // Interpreter tier: lib/05-math.lisp.
        assert_eq!(eval_line(&format!("(abs {arg})"), &e), want, "arg={arg}");
        assert_eq!(eval_line(&format!("(fab {arg})"), &e), want, "arg={arg}");
    }
    assert_eq!(
        eval_line("(fab (/ 0.0 0.0))", &e),
        eval_line("(abs (/ 0.0 0.0))", &e)
    );
}

/// #518: the integer path is unchanged in both tiers, including the wrapping
/// abs of i64::MIN that sets OVERFLOW.
#[test]
fn abs_integer_path_unchanged_in_both_tiers() {
    let e = env();
    eval_line("(defun-typed (iab int64) ((a int64)) (abs a))", &e);
    assert_eq!(
        eval_line("(see-type 'iab)", &e),
        "(TYPED (-> (INT64) INT64) COMPILED)"
    );
    for f in ["abs", "iab"] {
        for (arg, want) in [("-7", "7"), ("0", "0"), ("5", "5")] {
            assert_eq!(eval_line(&format!("({f} {arg})"), &e), want, "{f} {arg}");
        }
        eval_line("(clear-flag 'overflow)", &e);
        assert_eq!(
            eval_line(&format!("({f} (- -9223372036854775807 1))"), &e),
            "-9223372036854775808",
            "{f}"
        );
        assert_eq!(eval_line("(flag-set-p 'overflow)", &e), "T", "{f}");
    }
}
