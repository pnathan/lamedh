//! #528: every `dotimes` iteration binds its variable afresh, as `dolist`
//! does, so a closure made in the body keeps its own iteration's value.
//! `for` reuses one counter slot, so `dotimes` rebinds (`(let ((var var))
//! ...)`) only when the macro-expanded body can build a closure; a
//! closure-free body keeps the plain `for` and still compiles natively.

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line};

fn env() -> Shared<Environment> {
    Environment::with_stdlib()
}

/// Bodies that capture the loop variable, directly or through a macro
/// (`flet`, `push`, a nested `dolist`) that expands to a closure.
const CAPTURING: &[(&str, &str)] = &[
    (
        "(let ((fs nil)) (dotimes (i n) (setq fs (cons (lambda () i) fs))) \
         (mapcar (lambda (f) (f)) (reverse fs)))",
        "(0 1 2)",
    ),
    (
        "(let ((fs nil)) (dotimes (i n) (push (lambda () i) fs)) \
         (mapcar (lambda (f) (f)) (reverse fs)))",
        "(0 1 2)",
    ),
    (
        "(let ((fs nil)) (dotimes (i n) (flet ((g () i)) (setq fs (cons g fs)))) \
         (mapcar (lambda (f) (f)) (reverse fs)))",
        "(0 1 2)",
    ),
    (
        "(let ((fs nil)) (dotimes (i n) (dolist (x '(a)) (push (lambda () (list x i)) fs))) \
         (mapcar (lambda (f) (f)) (reverse fs)))",
        "((A 0) (A 1) (A 2))",
    ),
    (
        "(let ((fs nil)) (dotimes (i 2) (dotimes (j n) (push (lambda () (list i j)) fs))) \
         (mapcar (lambda (f) (f)) (reverse fs)))",
        "((0 0) (0 1) (0 2) (1 0) (1 1) (1 2))",
    ),
];

#[test]
fn interpreted_dotimes_closures_see_their_own_iteration() {
    let e = env();
    for (body, want) in CAPTURING {
        assert_eq!(
            &eval_line(&format!("(let ((n 3)) {body})"), &e),
            want,
            "{body}"
        );
    }
}

#[test]
fn defun_dotimes_closures_see_their_own_iteration() {
    // A defun body runs through the compiled closure IR, not the top-level
    // tree-walker; it must agree.
    let e = env();
    for (k, (body, want)) in CAPTURING.iter().enumerate() {
        eval_line(&format!("(defun dtc-{k} (n) {body})"), &e);
        assert_eq!(&eval_line(&format!("(dtc-{k} 3)"), &e), want, "{body}");
    }
}

#[test]
fn dotimes_matches_dolist_capture() {
    let e = env();
    let via = |form: &str| {
        eval_line(
            &format!("(let ((fs nil)) {form} (mapcar (lambda (f) (f)) (reverse fs)))"),
            &e,
        )
    };
    assert_eq!(
        via("(dotimes (i 3) (push (lambda () i) fs))"),
        via("(dolist (i '(0 1 2)) (push (lambda () i) fs))")
    );
}

#[test]
fn result_form_still_sees_count() {
    let e = env();
    assert_eq!(
        eval_line(
            "(let ((fs nil)) (dotimes (i 3 (list i (length fs))) (push (lambda () i) fs)))",
            &e
        ),
        "(3 3)"
    );
}

#[test]
fn only_closing_bodies_are_rebound() {
    let e = env();
    // Closure-free: the plain FOR, no per-iteration LET.
    let plain = eval_line(
        "(macroexpand '(dotimes (i 3) (incf x i) (when (> i 1) (print i))))",
        &e,
    );
    assert!(!plain.contains("(LET ((I I))"), "{plain}");
    // DOLIST expands to a closure-free WHILE loop (#504), so a DOLIST body
    // that merely reads I is not a closure either.
    let dl = eval_line(
        "(macroexpand '(dotimes (i 3) (dolist (x l) (print i))))",
        &e,
    );
    assert!(!dl.contains("(LET ((I I))"), "{dl}");
    // A quoted LAMBDA is data, not a closure.
    let quoted = eval_line("(macroexpand '(dotimes (i 3) (print '(lambda () i))))", &e);
    assert!(!quoted.contains("(LET ((I I))"), "{quoted}");
    // A closure, direct or macro-produced, is rebound.
    for body in [
        "(push (lambda () i) fs)",
        "(flet ((g () i)) (g))",
        "(dolist (x l) (push (lambda () i) fs))",
    ] {
        let exp = eval_line(&format!("(macroexpand '(dotimes (i 3) {body}))"), &e);
        assert!(exp.contains("(LET ((I I))"), "{body}: {exp}");
    }
}

#[test]
fn closure_free_dotimes_still_compiles_natively() {
    let e = env();
    eval_line(
        "(defun dtn (n) (let ((acc 0)) (dotimes (i n) (setq acc (+ acc i))) acc))",
        &e,
    );
    assert_eq!(
        eval_line("(explain-compile 'dtn)", &e),
        "((TIER . COMPILED) (SIGNATURE -> (INT64) INT64))"
    );
    assert_eq!(eval_line("(dtn 10)", &e), "45");
}
