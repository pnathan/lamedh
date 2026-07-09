//! (explain-compile 'f): the tier verdict plus the CONCRETE blocker keeping
//! a definition off the native tier — "why is this slow" as a dialogue.
//! Side-effect-free: explaining never installs anything.

mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

#[test]
fn compiled_functions_report_their_signature() {
    let e = env_with_stdlib();
    eval_line("(defun inc (x) (+ x 1))", &e);
    assert_eq!(
        eval_line("(explain-compile 'inc)", &e),
        "((TIER . COMPILED) (SIGNATURE -> (INT64) INT64))"
    );
}

#[test]
fn checked_functions_name_the_blocker() {
    let e = env_with_stdlib();
    // Ambiguous numeric type: checkable, not monomorphizable.
    eval_line("(defun sq (x) (* x x))", &e);
    let out = eval_line("(explain-compile 'sq)", &e);
    assert!(out.contains("(TIER . CHECKED)"), "got: {out}");
    assert!(out.contains("cannot infer operand type"), "got: {out}");
    // List-typed code is checked but not natively storable.
    eval_line("(defun hd (xs) (car xs))", &e);
    let out = eval_line("(explain-compile 'hd)", &e);
    assert!(out.contains("(TIER . CHECKED)"), "got: {out}");
    assert!(
        out.contains("(SCHEME FORALL (A) (-> ((LIST A)) A))"),
        "got: {out}"
    );
    assert!(out.contains("BLOCKER"), "got: {out}");
}

#[test]
fn pinned_and_dynamic_and_type_errors_are_distinct() {
    let e = env_with_stdlib();
    eval_line("(defun pinned (x) (declare (no-compile)) (+ x 1))", &e);
    let out = eval_line("(explain-compile 'pinned)", &e);
    assert!(out.contains("pinned to the interpreter"), "got: {out}");
    eval_line("(defun vary (&rest xs) xs)", &e);
    let out = eval_line("(explain-compile 'vary)", &e);
    assert!(out.contains("(TIER . DYNAMIC)"), "got: {out}");
    eval_line("(defun broken (x) (car (+ x 1)))", &e);
    let out = eval_line("(explain-compile 'broken)", &e);
    assert!(out.contains("(TIER . TYPE-ERROR)"), "got: {out}");
}

#[test]
fn explaining_is_side_effect_free() {
    let e = env_with_stdlib();
    // A compileable-but-uncompiled lambda (bound via DEF, not DEFUN, so the
    // one-door hook never ran): explain reports eligibility WITHOUT
    // installing a typed edition.
    eval_line("(def plain (lambda (x) (+ x 1)))", &e);
    let out = eval_line("(explain-compile 'plain)", &e);
    assert!(out.contains("natively compileable"), "got: {out}");
    let again = eval_line("(explain-compile 'plain)", &e);
    assert_eq!(out, again);
    // Still a plain lambda, still works.
    assert_eq!(eval_line("(plain 4)", &e), "5");
}
