mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

#[test]
fn condense_check_type_records_generated_function_results() {
    let env = env_with_stdlib();
    eval_line(
        "(defconcept invoice (:fields ((id int64) (amount int64) (status symbol))) (:invariant (>= amount 0)))",
        &env,
    );
    eval_line("(derive invoice equality)", &env);

    let out = eval_line("(condense-check-type 'invoice)", &env);
    assert!(out.contains("MAKE-INVOICE"), "got: {out}");
    // Every generated function here is on the healthy path; assert CHECKED
    // specifically (not just "one of CHECKED or TYPE-ERROR", which passes
    // whether the derivation is healthy or broken). In particular
    // `invoice-equal` (derived by `equality` as a 3-arg
    // `(and (invoice-p a) (invoice-p b) (equal a b))`, lib/20-condensation.lisp)
    // is the exact regression issue #202 named: variadic `and`/`or` (3+
    // operands) used to be rejected as a checker type error even though the
    // evaluator runs it fine.
    assert!(
        !out.contains("TYPE-ERROR"),
        "expected no type errors, got: {out}"
    );
    assert!(
        out.contains("(INVOICE-EQUAL CHECKED"),
        "expected INVOICE-EQUAL to check cleanly (issue #202: variadic AND/OR), got: {out}"
    );

    let stored = eval_line(
        "(cdr (assoc 'check-status (condense-trace 'invoice)))",
        &env,
    );
    assert!(stored.contains("INVOICE-P"), "got: {stored}");
}

#[test]
fn condense_check_type_marks_unchecked_or_dynamic_frontier() {
    let env = env_with_stdlib();
    eval_line(
        "(defconcept invoice (:fields ((id int64) (amount int64) (status symbol))) (:invariant (>= amount 0)))",
        &env,
    );
    eval_line(
        "(condense-put 'invoice \"condense.generated\" (append (condense-generated 'invoice) '(missing-generated-function)))",
        &env,
    );

    eval_line("(condense-check-type 'invoice)", &env);
    let frontier = eval_line(
        "(cdr (assoc 'dynamic-frontier (condense-trace 'invoice)))",
        &env,
    );
    assert!(
        frontier.contains("TYPE-ERROR") || frontier.contains("DYNAMIC"),
        "got: {frontier}"
    );
}

#[test]
fn condense_check_type_can_check_a_single_symbol() {
    let env = env_with_stdlib();
    eval_line("(defun id (x) x)", &env);
    let out = eval_line("(condense-check-type 'id)", &env);
    assert!(out.contains("ID"), "got: {out}");
    assert!(out.contains("CHECKED"), "got: {out}");
    assert!(out.contains("forall"), "got: {out}");
}
