//! #404: `cond`/`when`/`unless`/`case` compile by desugaring to nested `if`.
//! Differential: every compiled result equals the interpreter's result for
//! the same body, and the portable codegen gate agrees on admission.

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line};

fn env() -> Shared<Environment> {
    Environment::with_stdlib()
}

/// (name, param, body, inputs, expected portable verdict)
const CASES: &[(&str, &str, &str, &[i64], &str)] = &[
    (
        "br-sgn",
        "x",
        "(cond ((< x 0) -1) ((> x 0) 1) (t 0))",
        &[-5, 0, 9],
        "(COMPILEABLE (-> (INT64) INT64))",
    ),
    (
        "br-when",
        "x",
        "(when (> x 0) (< x 100))",
        &[-1, 5, 500],
        "(COMPILEABLE (-> (INT64) BOOL))",
    ),
    (
        "br-unless",
        "x",
        "(unless (> x 0) (< x -10))",
        &[-20, -1, 5],
        "(COMPILEABLE (-> (INT64) BOOL))",
    ),
    (
        "br-cond-test-only",
        "x",
        "(cond ((< x 0)) (t (> x 10)))",
        &[-1, 3, 11],
        "(COMPILEABLE (-> (INT64) BOOL))",
    ),
    (
        "br-stmt",
        "n",
        "(let ((a 0)) (for (i 1 n) (when (= (mod i 3) 0) (setq a (+ a i))) \
         (unless (< i 5) (setq a (+ a 1)))) a)",
        &[0, 1, 20],
        "(COMPILEABLE (-> (INT64) INT64))",
    ),
    (
        "br-while",
        "n",
        "(let ((a 0) (i 0)) (while (< i n) (cond ((< i 3) (setq a (+ a 1))) \
         (t (setq a (+ a 2)))) (setq i (+ i 1))) a)",
        &[0, 2, 10],
        "(COMPILEABLE (-> (INT64) INT64))",
    ),
    (
        "br-case",
        "x",
        "(case x (1 10) ((2 3) 20) (otherwise 30))",
        &[1, 2, 3, 4],
        "(COMPILEABLE (-> (INT64) INT64))",
    ),
    (
        "br-case-stmt",
        "x",
        "(let ((a 0)) (case x (1 (setq a 7)) (t (setq a 9))) a)",
        &[1, 2],
        "(COMPILEABLE (-> (INT64) INT64))",
    ),
];

#[test]
fn branch_forms_compile_and_match_the_interpreter() {
    let e = env();
    for (name, p, body, inputs, _) in CASES {
        eval_line(&format!("(defun {name} ({p}) {body})"), &e);
        let ex = eval_line(&format!("(explain-compile '{name})"), &e);
        assert!(ex.starts_with("((TIER . COMPILED)"), "{name}: {ex}");
        for n in *inputs {
            let want = eval_line(&format!("(let (({p} {n})) {body})"), &e);
            assert_eq!(eval_line(&format!("({name} {n})"), &e), want, "{name} {n}");
        }
    }
}

#[test]
fn the_portable_gate_agrees() {
    let e = env();
    for (name, p, body, _, want) in CASES {
        assert_eq!(
            eval_line(&format!("(hm-compile-lambda '{name} '({p}) '({body}))"), &e),
            *want,
            "{name}"
        );
    }
}

#[test]
fn nil_on_miss_values_and_non_integer_case_keys_stay_interpreted() {
    let e = env();
    // A value-position `when` whose body is not bool yields NIL on a miss,
    // which native code cannot carry: not compiled, still correct.
    eval_line("(defun br-w (x) (when (> x 0) (+ x 1)))", &e);
    assert!(!eval_line("(explain-compile 'br-w)", &e).contains("COMPILED"));
    assert_eq!(eval_line("(br-w -1)", &e), "()");
    assert_eq!(eval_line("(br-w 1)", &e), "2");
    eval_line("(defun br-sym (x) (case x (a 10) (t 20)))", &e);
    let ex = eval_line("(explain-compile 'br-sym)", &e);
    assert!(ex.contains("only integer keys compile"), "{ex}");
    assert_eq!(eval_line("(br-sym 'a)", &e), "10");
}

#[test]
fn case_with_integer_keys_is_no_longer_a_false_type_error() {
    let e = env();
    eval_line("(defun br-c2 (x) (case x (1 10) (2 20)))", &e);
    let ex = eval_line("(explain-compile 'br-c2)", &e);
    assert!(!ex.contains("TYPE-ERROR"), "{ex}");
    assert_eq!(eval_line("(br-c2 3)", &e), "()");
}
