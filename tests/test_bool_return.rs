//! #505: a `defun*` returning `t`/`nil` was generalized to `∀A`, so an
//! arithmetic caller was reported CHECKED and then failed at runtime:
//!
//!   (defun* f1 (x) (if (= x 1) t nil))   ; was (FORALL (A) (-> (INT64) A))
//!   (defun* f4 (x) (+ 1 (f1 x)))         ; was CHECKED, then
//!   (f4 1)                               ; "Math functions only accept numbers, got T"
//!
//! The free symbol `t` typed as the gradual `any`, which `unify` absorbs
//! without binding, so the function's return variable stayed free and was
//! quantified. Now `t` is `bool`, a `nil` branch or clause meeting `bool` is
//! that boolean's false, and an `any` body is an `any` return rather than a
//! `∀`. Every verdict is asserted for BOTH the native checker (`see-type`)
//! and the portable one (`hm-see-type`, lib/46-hm-check.lisp), which must
//! agree.

mod test_helpers;

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line};
use test_helpers::env_with_stdlib;

/// `(native portable)` verdicts for NAME.
fn verdicts(e: &Shared<Environment>, name: &str) -> String {
    eval_line(
        &format!("(list (see-type '{name}) (hm-see-type '{name}))"),
        e,
    )
}

/// Assert both checkers give exactly VERDICT for NAME.
fn both(e: &Shared<Environment>, name: &str, verdict: &str) {
    assert_eq!(
        verdicts(e, name),
        format!("({verdict} {verdict})"),
        "{name}"
    );
}

#[test]
fn a_t_nil_function_is_bool_not_generalized() {
    let e = env_with_stdlib();
    eval_line("(defun* f1 (x) (if (= x 1) t nil))", &e);
    both(&e, "f1", "(CHECKED (-> (INT64) BOOL))");
    assert_eq!(eval_line("(f1 1)", &e), "T");
    assert_eq!(eval_line("(f1 2)", &e), "()");
}

#[test]
fn an_arithmetic_caller_of_a_t_nil_function_is_not_checked() {
    let e = env_with_stdlib();
    eval_line("(defun* f1 (x) (if (= x 1) t nil))", &e);
    eval_line("(defun* f4 (x) (+ 1 (f1 x)))", &e);
    let out = verdicts(&e, "f4");
    assert!(
        out.starts_with("((TYPE-ERROR") && out.contains(") (TYPE-ERROR"),
        "f4 must be a TYPE-ERROR in both checkers: {out}"
    );
    // The verdict now matches what running it does.
    assert!(eval_line("(f4 1)", &e).contains("got T"));
}

#[test]
fn every_spelling_of_a_boolean_function_is_bool() {
    let e = env_with_stdlib();
    // nil first, cond.
    eval_line("(defun q1 (x) (cond ((= x 1) nil) (t t)))", &e);
    both(&e, "q1", "(CHECKED (-> (INT64) BOOL))");
    // nil at the tail of a progn branch.
    eval_line("(defun q2 (x) (if (= x 1) (progn (princ \"\") nil) t))", &e);
    both(&e, "q2", "(CHECKED (-> (INT64) BOOL))");
    // case.
    eval_line("(defun q3 (x) (case x (1 t) (otherwise nil)))", &e);
    both(&e, "q3", "(CHECKED (FORALL (A) (-> (A) BOOL)))");
}

#[test]
fn a_recursive_boolean_helper_is_bool_not_a_list() {
    // The N-Queens shape the issue reports as typed `(LIST A)`.
    let e = env_with_stdlib();
    eval_line(
        "(defun safe-p (q qs d) \
           (cond ((null qs) t) \
                 ((= q (car qs)) nil) \
                 ((= (abs (- q (car qs))) d) nil) \
                 (t (safe-p q (cdr qs) (+ d 1)))))",
        &e,
    );
    both(
        &e,
        "safe-p",
        "(CHECKED (-> (INT64 (LIST INT64) INT64) BOOL))",
    );
    assert_eq!(eval_line("(safe-p 1 '(3 5) 1)", &e), "T");
    assert_eq!(eval_line("(safe-p 1 '(2) 1)", &e), "()");
}

#[test]
fn an_any_return_is_any_not_a_quantified_variable() {
    let e = env_with_stdlib();
    eval_line("(defun g (x) (eval x))", &e);
    both(&e, "g", "(CHECKED (FORALL (A) (-> (A) ANY)))");
    // An `any` branch is not lost to the other branch, in either order.
    eval_line("(defun g1 (p x) (if p nil (eval x)))", &e);
    both(&e, "g1", "(CHECKED (FORALL (A B) (-> (A B) ANY)))");
    eval_line("(defun g2 (p x) (if p (eval x) nil))", &e);
    both(&e, "g2", "(CHECKED (FORALL (A B) (-> (A B) ANY)))");
}

#[test]
fn a_genuine_list_or_nil_function_keeps_its_list_type() {
    let e = env_with_stdlib();
    eval_line("(defun lon (p x) (if p (list x) nil))", &e);
    both(&e, "lon", "(CHECKED (FORALL (A B) (-> (A B) (LIST B))))");
}
