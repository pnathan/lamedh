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
    // A derived equality carries a DECLARED record scheme. SELF is substituted
    // with INVOICE's *closed record type*, not the symbol INVOICE, so the
    // kernel's row unifier confirms the declared scheme subsumes it: CONFORMS.
    eval_line(
        "(definterface eq-able (:ops ((equal (-> (self self) bool)))))",
        &env,
    );
    eval_line(
        "(defconcept invoice (:fields ((id int64) (amount int64))) (:derive equality))",
        &env,
    );
    let report = eval_line("(implements? 'invoice 'eq-able)", &env);
    assert!(report.starts_with("(T"), "got: {report}");
    assert!(report.contains("(EQUAL CONFORMS"), "got: {report}");
}

#[test]
fn row_accessor_scheme_confirms_the_op_it_projects() {
    // The evidence-inversion fix: a method carrying an informative row scheme —
    // the *strongest* evidence the checker can produce — grades CONFORMS, not
    // MISMATCH. Before the fix, SELF was substituted with the concept symbol
    // and the record-typed verdict could never unify against it.
    let env = env_with_stdlib();
    eval_line(
        "(definterface named (:ops ((name (-> (self) string)))))",
        &env,
    );
    eval_line(
        "(defconcept goblin (:fields ((name string) (hp int64))))",
        &env,
    );
    let report = eval_line("(implements? 'goblin 'named)", &env);
    assert!(report.starts_with("(T"), "got: {report}");
    assert!(
        report.contains("(NAME CONFORMS GOBLIN-NAME"),
        "got: {report}"
    );
    eval_line("(implements! 'goblin 'named)", &env);
}

#[test]
fn scheme_subsumes_queries_the_kernel_row_unifier() {
    let env = env_with_stdlib();
    // A polymorphic row scheme subsumes a closed record that carries the field.
    assert_eq!(
        eval_line(
            "(scheme-subsumes? '(forall (a) (-> ((record ((hp int64)) a)) int64)) \
                              '(-> ((record ((name string) (hp int64)))) int64))",
            &env
        ),
        "T"
    );
    // Wrong return type does not subsume.
    assert_eq!(
        eval_line(
            "(scheme-subsumes? '(forall (a) (-> ((record ((hp int64)) a)) int64)) \
                              '(-> ((record ((hp int64)))) bool))",
            &env
        ),
        "()"
    );
    // A wanted record missing a field the scheme requires does not subsume.
    assert_eq!(
        eval_line(
            "(scheme-subsumes? '(forall (a) (-> ((record ((gold int64)) a)) int64)) \
                              '(-> ((record ((hp int64)))) int64))",
            &env
        ),
        "()"
    );
    // An unparseable wanted type is "not confirmed", never a crash.
    assert_eq!(
        eval_line(
            "(scheme-subsumes? '(-> (int64) int64) '(-> (nonesuch) int64))",
            &env
        ),
        "()"
    );
}
