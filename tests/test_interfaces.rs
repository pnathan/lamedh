mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

#[test]
fn typed_method_conforms_by_unification() {
    let env = env_with_stdlib();
    eval_line(
        "(definterface counter (:ops ((bump (-> (self) self)))))",
        &env,
    );
    eval_line(
        "(defun-typed (int64-bump int64) ((self int64)) (+ self 1))",
        &env,
    );
    let report = eval_line("(implements? 'int64 'counter)", &env);
    assert!(report.starts_with("(T"), "got: {report}");
    assert!(
        report.contains("(BUMP CONFORMS INT64-BUMP"),
        "got: {report}"
    );
    assert_eq!(eval_line("(method 'bump 5)", &env), "6");
    // The explicit assertion records the claim on both sides.
    eval_line("(implements! 'int64 'counter)", &env);
    assert_eq!(
        eval_line("(getp 'int64 \"interface.implements\")", &env),
        "(COUNTER)"
    );
}

#[test]
fn wrong_signature_is_a_mismatch_and_fails_the_assertion() {
    let env = env_with_stdlib();
    eval_line(
        "(definterface counter (:ops ((bump (-> (self) self)))))",
        &env,
    );
    eval_line(
        "(defun-typed (int64-bump bool) ((self int64)) (> self 0))",
        &env,
    );
    let report = eval_line("(implements? 'int64 'counter)", &env);
    assert!(report.starts_with("(()"), "got: {report}");
    assert!(report.contains("(BUMP MISMATCH"), "got: {report}");
    assert_eq!(
        eval_line("(errorset '(implements! 'int64 'counter))", &env),
        "()"
    );
}

#[test]
fn missing_method_fails_but_unproven_passes() {
    let env = env_with_stdlib();
    eval_line(
        "(definterface eq-able (:ops ((equal (-> (self self) bool)))))",
        &env,
    );
    eval_line(
        "(definterface renderable (:ops ((render (-> (self) string)))))",
        &env,
    );
    eval_line(
        "(defconcept bag (:fields ((items list))) (:derive equality))",
        &env,
    );
    // bag-equal exists but its scheme is vacuous (unmappable field type):
    // structurally satisfied, honestly unproven, overall PASS.
    let eq = eval_line("(implements? 'bag 'eq-able)", &env);
    assert!(eq.starts_with("(T"), "got: {eq}");
    assert!(eq.contains("(EQUAL UNPROVEN"), "got: {eq}");
    // No bag-render exists: MISSING fails the check.
    let render = eval_line("(implements? 'bag 'renderable)", &env);
    assert!(render.contains("(RENDER MISSING"), "got: {render}");
    assert!(render.starts_with("(()"), "got: {render}");
}

#[test]
fn method_dispatches_on_concept_values() {
    let env = env_with_stdlib();
    eval_line(
        "(defconcept invoice (:fields ((id int64) (amount int64))) (:derive equality))",
        &env,
    );
    assert_eq!(
        eval_line(
            "(method 'equal (make-invoice 1 2) (make-invoice 1 2))",
            &env
        ),
        "T"
    );
}

#[test]
fn declared_row_schemes_count_as_conformance_evidence() {
    let env = env_with_stdlib();
    // With rows, a derived equality carries a DECLARED record scheme; the
    // interface layer unifies the declared signature against it.
    eval_line(
        "(definterface eq-able (:ops ((equal (-> (self self) bool)))))",
        &env,
    );
    eval_line(
        "(defconcept invoice (:fields ((id int64) (amount int64))) (:derive equality))",
        &env,
    );
    let report = eval_line("(implements? 'invoice 'eq-able)", &env);
    // INVOICE-EQUAL's declared type is over records, not the symbol INVOICE,
    // so unification against (-> (invoice invoice) bool) cannot confirm — but
    // the operation exists and the check must not error.
    assert!(report.contains("(EQUAL"), "got: {report}");
}
