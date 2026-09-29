//! #522: `quotient` (the evaluator's name for `/`), `remainder`/`rem`
//! (truncated remainder) and `let*` (nested `let`) compile. Differential:
//! every compiled result, OVERFLOW flag and error equals the interpreter's
//! for the same body, and the portable codegen gate agrees on admission.

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line};

fn env() -> Shared<Environment> {
    Environment::with_stdlib()
}

const MIN: i64 = i64::MIN;

/// (name, params, body, argument lists, expected portable verdict)
const CASES: &[(&str, &str, &str, &[&str], &str)] = &[
    (
        "qr-quot",
        "x y",
        "(quotient x (+ y 0))",
        &["7 2", "-7 2", "7 -2", "-7 -2", "0 5"],
        "(COMPILEABLE (-> (INT64 INT64) INT64))",
    ),
    (
        "qr-rem",
        "x y",
        "(remainder x (+ y 0))",
        &[
            "7 3",
            "-7 3",
            "7 -3",
            "-7 -3",
            "0 5",
            "9223372036854775807 -2",
        ],
        "(COMPILEABLE (-> (INT64 INT64) INT64))",
    ),
    (
        "qr-rem-alias",
        "x y",
        "(rem x (+ y 0))",
        &["7 3", "-7 3", "7 -3", "-7 -3"],
        "(COMPILEABLE (-> (INT64 INT64) INT64))",
    ),
    (
        "qr-fquot",
        "x",
        "(quotient x 2.0)",
        &["7.0", "-1.5"],
        "(COMPILEABLE (-> (FLOAT64) FLOAT64))",
    ),
    (
        "qr-letstar",
        "x",
        "(let* ((a (+ x 1)) (b (* a 2))) (- b a))",
        &["0", "4", "-9"],
        "(COMPILEABLE (-> (INT64) INT64))",
    ),
    (
        // Each init sees the binding before it, including a rebinding of a
        // parameter — the difference from parallel `let`.
        "qr-letstar-shadow",
        "x",
        "(let* ((y x) (x (* x 10)) (x (+ x y))) x)",
        &["1", "-3"],
        "(COMPILEABLE (-> (INT64) INT64))",
    ),
    (
        "qr-letstar-empty",
        "x",
        "(let* () (+ x 1))",
        &["41"],
        "(COMPILEABLE (-> (INT64) INT64))",
    ),
    (
        "qr-mixed",
        "n",
        "(let* ((a 0)) (for (i 1 n) (setq a (+ a (rem i 3) (quotient i 2)))) a)",
        &["0", "1", "20"],
        "(COMPILEABLE (-> (INT64) INT64))",
    ),
];

#[test]
fn quotient_remainder_and_let_star_compile_and_match_the_interpreter() {
    let e = env();
    for (name, ps, body, inputs, _) in CASES {
        eval_line(&format!("(defun {name} ({ps}) {body})"), &e);
        let ex = eval_line(&format!("(explain-compile '{name})"), &e);
        assert!(ex.starts_with("((TIER . COMPILED)"), "{name}: {ex}");
        for args in *inputs {
            let binds: Vec<String> = ps
                .split_whitespace()
                .zip(args.split_whitespace())
                .map(|(p, a)| format!("({p} {a})"))
                .collect();
            let want = eval_line(&format!("(let ({}) {body})", binds.join(" ")), &e);
            assert_eq!(
                eval_line(&format!("({name} {args})"), &e),
                want,
                "{name} {args}"
            );
        }
    }
}

#[test]
fn the_portable_gate_agrees() {
    let e = env();
    for (name, ps, body, _, want) in CASES {
        assert_eq!(
            eval_line(
                &format!("(hm-compile-lambda '{name} '({ps}) '({body}))"),
                &e
            ),
            *want,
            "{name}"
        );
    }
}

/// Run FORM after clearing OVERFLOW; return (value or error message, flag).
/// Only an error's first line is kept: the interpreter appends `in: REM`
/// backtrace lines because `rem` is a Lisp function, the compiled call does
/// not; the error itself must be identical.
fn run(e: &Shared<Environment>, form: &str) -> (String, String) {
    eval_line("(clear-flag 'overflow)", e);
    let v = eval_line(form, e);
    let v = v.lines().next().unwrap_or_default().to_string();
    (v, eval_line("(flag-set-p 'overflow)", e))
}

#[test]
fn flags_and_errors_match_the_interpreter() {
    let e = env();
    eval_line(
        "(defun-typed (tq int64) ((x int64) (y int64)) (quotient x y))",
        &e,
    );
    eval_line(
        "(defun-typed (tr int64) ((x int64) (y int64)) (remainder x y))",
        &e,
    );
    eval_line(
        "(defun-typed (tm int64) ((x int64) (y int64)) (rem x y))",
        &e,
    );
    for f in ["tq", "tr", "tm"] {
        let ex = eval_line(&format!("(see-type '{f})"), &e);
        assert!(ex.ends_with("COMPILED)"), "{f}: {ex}");
    }
    let rows = [
        ("tq", "quotient", format!("{MIN} -1")),
        ("tr", "remainder", format!("{MIN} -1")),
        ("tm", "rem", format!("{MIN} -1")),
        ("tq", "quotient", "1 0".to_string()),
        ("tr", "remainder", "1 0".to_string()),
        ("tm", "rem", "1 0".to_string()),
        ("tr", "remainder", "-7 3".to_string()),
    ];
    for (typed, op, args) in rows {
        let want = run(&e, &format!("({op} {args})"));
        let got = run(&e, &format!("({typed} {args})"));
        assert_eq!(got, want, "({op} {args})");
    }
    // The pinned expectations, so parity cannot pass vacuously.
    assert_eq!(
        run(&e, &format!("(tr {MIN} -1)")),
        ("0".to_string(), "T".to_string())
    );
    assert!(run(&e, "(tr 1 0)").0.contains("Division by zero"));
}

#[test]
fn remainder_is_int64_only_and_binary() {
    let e = env();
    // The evaluator's REMAINDER rejects floats, so the compiler must too.
    assert!(
        eval_line("(remainder 7.5 2.0)", &e).contains("expected numbers"),
        "evaluator REMAINDER is integer-only"
    );
    let r = eval_line("(defun-typed (fr float64) ((x float64)) (rem x 2.0))", &e);
    assert!(r.contains("`remainder` is int64-only"), "{r}");
    let r = eval_line("(hm-compile-lambda 'fr '(x) '((rem (* x 1.0) 2.0)))", &e);
    assert!(r.contains("`remainder` is int64-only"), "{r}");
    let r = eval_line("(defun-typed (ar int64) ((x int64)) (rem x))", &e);
    assert!(r.contains("requires exactly 2 arguments"), "{r}");
    let r = eval_line("(defun-typed (aq int64) ((x int64)) (quotient x 1 2))", &e);
    assert!(r.contains("requires exactly 2 arguments"), "{r}");
}

#[test]
fn let_star_rejects_the_shapes_the_interpreter_rejects() {
    let e = env();
    // `let*` bindings are `(name init)` pairs only; `let-typed`'s
    // `(name type init)` shape must not compile as `let*`.
    assert!(eval_line("(let* ((a int64 1)) a)", &e).contains("Error"));
    let r = eval_line(
        "(defun-typed (ls int64) ((x int64)) (let* ((a int64 x)) a))",
        &e,
    );
    assert!(r.contains("let* binding must be a (name init) pair"), "{r}");
    let r = eval_line("(hm-compile-lambda 'ls '(x) '((let* ((a int64 x)) a)))", &e);
    assert!(r.contains("let* binding must be a (name init) pair"), "{r}");
    let r = eval_line("(defun-typed (lb int64) ((x int64)) (let* ((a x))))", &e);
    assert!(r.contains("let* requires a binding list"), "{r}");
}
