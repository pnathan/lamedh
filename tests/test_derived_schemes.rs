//! Derived schemes at call sites (#308): when the checker meets a call to an
//! unknown function that is bound to a plain lambda, it checks that lambda's
//! own body on demand (memoized per run, monotype assumption for recursion)
//! and uses the derived scheme — instead of degrading the call to `Any`.
//! This is what lets row types flow through helper functions with zero
//! `declare-type!` axioms.

mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

#[test]
fn rows_flow_through_one_helper_with_no_axioms() {
    let e = env_with_stdlib();
    eval_line("(defun the-hp (x) (record-ref x 'hp))", &e);
    eval_line("(defun wounded-p (x) (< (the-hp x) 5))", &e);
    assert_eq!(
        eval_line("(see-type 'wounded-p)", &e),
        "(CHECKED (FORALL (A) (-> ((RECORD ((HP INT64)) A)) BOOL)))"
    );
}

#[test]
fn rows_flow_through_two_layers() {
    let e = env_with_stdlib();
    eval_line("(defun the-hp (x) (record-ref x 'hp))", &e);
    eval_line("(defun half-hp (x) (/ (the-hp x) 2))", &e);
    eval_line("(defun weak-p (x) (< (half-hp x) 3))", &e);
    assert_eq!(
        eval_line("(see-type 'weak-p)", &e),
        "(CHECKED (FORALL (A) (-> ((RECORD ((HP INT64)) A)) BOOL)))"
    );
}

#[test]
fn misuse_is_rejected_through_the_helper() {
    let e = env_with_stdlib();
    eval_line("(defrecord disc (r int64))", &e);
    eval_line("(defun the-cost (x) (record-ref x 'cost))", &e);
    // The demand travels through the helper's derived scheme: a disc has no
    // cost, statically.
    let out = eval_line("(check-type (the-cost (make-disc 2)))", &e);
    assert!(
        out.contains("type error") && out.contains("cost"),
        "got: {out}"
    );
}

#[test]
fn self_recursion_terminates_with_a_monotype_assumption() {
    let e = env_with_stdlib();
    eval_line(
        "(defun count-down (n) (if (< n 1) 0 (count-down (- n 1))))",
        &e,
    );
    eval_line("(defun uses-it (x) (count-down (record-ref x 'n)))", &e);
    assert_eq!(
        eval_line("(see-type 'uses-it)", &e),
        "(CHECKED (FORALL (A) (-> ((RECORD ((N INT64)) A)) INT64)))"
    );
}

#[test]
fn mutual_recursion_terminates() {
    let e = env_with_stdlib();
    eval_line("(defun even-p* (n) (if (< n 1) t (odd-p* (- n 1))))", &e);
    eval_line("(defun odd-p* (n) (if (< n 1) nil (even-p* (- n 1))))", &e);
    // Checking one pulls the other in through the resolver; the in-flight
    // assumption breaks the cycle. No hang, and the scheme is informative.
    let out = eval_line("(see-type 'even-p*)", &e);
    assert!(out.starts_with("(CHECKED"), "got: {out}");
    assert!(out.contains("INT64"), "got: {out}");
}

#[test]
fn a_broken_callee_degrades_the_call_not_the_caller() {
    let e = env_with_stdlib();
    // The helper itself has a type error (car of an int64).
    eval_line("(defun bad-helper (x) (car (+ x 1)))", &e);
    eval_line("(defun caller (y) (bad-helper y))", &e);
    // The callee's error is reported at its own definition...
    let helper = eval_line("(see-type 'bad-helper)", &e);
    assert!(helper.starts_with("(TYPE-ERROR"), "got: {helper}");
    // ...while the caller stays gradual (the call degrades to Any) instead
    // of inheriting a confusing secondhand error.
    let caller = eval_line("(see-type 'caller)", &e);
    assert!(caller.starts_with("(CHECKED"), "got: {caller}");
}

#[test]
fn wrong_arity_against_a_derived_scheme_is_an_error() {
    let e = env_with_stdlib();
    eval_line("(defun two-args (a b) (+ a b))", &e);
    eval_line("(defun caller (x) (two-args x))", &e);
    let out = eval_line("(see-type 'caller)", &e);
    assert!(
        out.starts_with("(TYPE-ERROR") && out.contains("expects 2 args"),
        "got: {out}"
    );
}
