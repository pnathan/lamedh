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
    // No generated function here is a checker *error*. In particular
    // `invoice-equal` (a 3-arg `(and ...)`) is the exact regression issue
    // #202 named: variadic `and`/`or` used to be rejected as a type error
    // even though the evaluator runs it fine.
    assert!(
        !out.contains("TYPE-ERROR"),
        "expected no type errors, got: {out}"
    );
    // With experimental rows, concept operations over mappable field types
    // carry DECLARED row schemes — generator-backed axioms, reported
    // distinctly from CHECKED (derived) and never blended into "verified by
    // the body".
    assert!(
        out.contains("(MAKE-INVOICE DECLARED"),
        "expected MAKE-INVOICE to carry a declared row scheme, got: {out}"
    );
    assert!(
        out.contains("(INVOICE-EQUAL DECLARED"),
        "expected INVOICE-EQUAL to carry a declared row scheme, got: {out}"
    );

    let stored = eval_line(
        "(cdr (assoc 'check-status (condense-trace 'invoice)))",
        &env,
    );
    assert!(stored.contains("INVOICE-P"), "got: {stored}");
}

#[test]
fn vacuous_schemes_join_the_unproven_frontier() {
    let env = env_with_stdlib();
    // `list` is not a row-mappable field type, so this concept gets NO
    // declared schemes: its accessors infer vacuous schemes and must sit on
    // the frontier rather than count as verified.
    eval_line("(defconcept bag (:fields ((items list))))", &env);
    eval_line("(condense-check-type 'bag)", &env);
    let frontier = eval_line(
        "(cdr (assoc 'dynamic-frontier (condense-trace 'bag)))",
        &env,
    );
    assert!(frontier.contains("BAG-ITEMS VACUOUS"), "got: {frontier}");
}

#[test]
fn fully_derived_row_concept_has_an_empty_frontier() {
    let env = env_with_stdlib();
    eval_line(
        "(defconcept invoice (:fields ((id int64) (amount int64))) (:derive equality lens))",
        &env,
    );
    assert_eq!(
        eval_line(
            "(cdr (assoc 'dynamic-frontier (condense-trace 'invoice)))",
            &env
        ),
        "()"
    );
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
        frontier.contains("MISSING-GENERATED-FUNCTION DYNAMIC"),
        "got: {frontier}"
    );
}

#[test]
fn condense_check_type_can_check_a_single_symbol() {
    let env = env_with_stdlib();
    eval_line("(defun id (x) x)", &env);
    let out = eval_line("(condense-check-type 'id)", &env);
    // Identity's scheme (FORALL (A) (-> (A) A)) constrains the result by the
    // argument, so it is informative — CHECKED, not VACUOUS.
    assert!(out.contains("(ID CHECKED"), "got: {out}");
    assert!(out.contains("FORALL"), "got: {out}");
}

#[test]
fn see_type_reports_structured_verdicts() {
    let env = env_with_stdlib();
    eval_line("(defun inc (x) (+ x 1))", &env);
    assert_eq!(
        eval_line("(see-type 'inc)", &env),
        "(CHECKED (-> (INT64) INT64))"
    );
    eval_line("(defun bad (x) (+ 1 \"s\"))", &env);
    let bad = eval_line("(see-type 'bad)", &env);
    assert!(bad.starts_with("(TYPE-ERROR"), "got: {bad}");
    let dynamic = eval_line("(see-type 'car)", &env);
    assert!(dynamic.starts_with("(DYNAMIC"), "got: {dynamic}");
}
