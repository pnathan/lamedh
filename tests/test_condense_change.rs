mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

#[test]
fn condense_diff_localizes_a_single_changed_leaf() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line(
            "(condense-diff '(defun f (x) (+ x 1)) '(defun f (x) (+ x 2)))",
            &env
        ),
        "(((3 2) 1 2))"
    );
}

#[test]
fn condense_diff_of_equal_forms_is_empty() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("(condense-diff '(a (b c)) '(a (b c)))", &env),
        "()"
    );
}

#[test]
fn condense_diff_reports_shape_changes_as_a_whole_node() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("(condense-diff '(a b) '(a b c))", &env),
        "((() (A B) (A B C)))"
    );
}

#[test]
fn fresh_concept_has_no_stale_definitions() {
    let env = env_with_stdlib();
    eval_line(
        "(defconcept invoice (:fields ((id int64) (amount int64))))",
        &env,
    );
    assert_eq!(eval_line("(condense-stale 'invoice)", &env), "()");
}

#[test]
fn hand_edited_generated_function_is_flagged_stale() {
    let env = env_with_stdlib();
    eval_line(
        "(defconcept invoice (:fields ((id int64) (amount int64))))",
        &env,
    );
    eval_line("(defun invoice-p (self) t)", &env);
    assert_eq!(eval_line("(condense-stale 'invoice)", &env), "(INVOICE-P)");
    let drift = eval_line("(condense-drift 'invoice)", &env);
    assert!(drift.contains("INVOICE-P"), "got: {drift}");
    let trace = eval_line("(cdr (assoc 'stale (condense-trace 'invoice)))", &env);
    assert_eq!(trace, "(INVOICE-P)");
}

#[test]
fn concept_redefinition_records_a_structural_diff() {
    let env = env_with_stdlib();
    eval_line(
        "(defconcept invoice (:fields ((id int64) (amount int64))) (:invariant (>= amount 0)))",
        &env,
    );
    assert_eq!(
        eval_line("(condense-get 'invoice \"condense.last-diff\")", &env),
        "()"
    );
    eval_line(
        "(defconcept invoice (:fields ((id int64) (amount int64))) (:invariant (>= amount 1)))",
        &env,
    );
    let diff = eval_line("(condense-get 'invoice \"condense.last-diff\")", &env);
    assert!(
        diff.contains("0 1"),
        "diff should pinpoint 0 -> 1, got: {diff}"
    );
    // The diff localizes to a path, not the whole expansion.
    assert!(
        !diff.contains("PROGN"),
        "diff should not be whole-tree, got: {diff}"
    );
}

#[test]
fn recheck_reports_staleness_examples_and_checker_status() {
    let env = env_with_stdlib();
    eval_line(
        "(defconcept invoice (:fields ((id int64) (amount int64))) (:invariant (>= amount 0)))",
        &env,
    );
    eval_line(
        "(example valid-invoice (:for invoice) (:given (make-invoice 1 100)) (:expect (validate-invoice *it*)))",
        &env,
    );
    let report = eval_line("(condense-recheck! 'invoice)", &env);
    assert!(report.contains("(STALE)"), "got: {report}");
    assert!(
        report.contains("CHECKS T"),
        "examples should pass, got: {report}"
    );
    assert!(report.contains("CHECK-STATUS"), "got: {report}");

    eval_line("(defun validate-invoice (self) nil)", &env);
    let report = eval_line("(condense-recheck! 'invoice)", &env);
    assert!(report.contains("(STALE VALIDATE-INVOICE)"), "got: {report}");
}
