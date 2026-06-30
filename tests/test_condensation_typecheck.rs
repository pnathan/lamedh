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
    assert!(
        out.contains("CHECKED") || out.contains("TYPE-ERROR"),
        "got: {out}"
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
