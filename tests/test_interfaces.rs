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
    // SELF for a row-typed concept substitutes to its closed record, and the
    // interface unifier is row-aware: the declared equality scheme over open
    // records unifies with it — CONFORMS, a real guarantee.
    assert!(report.contains("(EQUAL CONFORMS"), "got: {report}");
    assert!(report.starts_with("(T"), "got: {report}");
}

#[test]
fn interface_is_a_condensation_citizen() {
    // Seam B: definterface records through the condensation substrate, so the
    // interface shares CONDENSE-KIND / CONDENSE-TRACE with concepts instead of
    // a private key set.
    let env = env_with_stdlib();
    eval_line(
        "(definterface counter (:ops ((bump (-> (self) self)))))",
        &env,
    );
    assert_eq!(eval_line("(condense-kind 'counter)", &env), "INTERFACE");
    assert_eq!(eval_line("(interface-p 'counter)", &env), "T");
    assert_eq!(
        eval_line("(alist-get (condense-trace 'counter) 'kind)", &env),
        "INTERFACE"
    );
}

#[test]
fn implements_claims_are_fingerprinted_and_detect_drift() {
    // Seam B: a claim recorded once must not silently outlive the code it
    // vouched for. IMPLEMENTS! fingerprints the conforming methods, and
    // IMPLEMENTS-RECHECK! flags a later incompatible redefinition.
    let env = env_with_stdlib();
    eval_line(
        "(definterface greeter (:ops ((greet (-> (self) string)))))",
        &env,
    );
    eval_line("(defconcept goblin (:fields ((name string))))", &env);
    eval_line("(defun goblin-greet (self) (goblin-name self))", &env);
    eval_line("(implements! 'goblin 'greeter)", &env);
    // Fresh claim: it holds and nothing has drifted.
    let ok = eval_line("(implements-recheck! 'goblin)", &env);
    assert!(ok.contains("(CONFORMS . T)"), "got: {ok}");
    assert!(ok.contains("(DRIFT)"), "got: {ok}");
    // Break the method: now it returns an int, contradicting the signature.
    eval_line("(defun goblin-greet (self) 42)", &env);
    let drifted = eval_line("(implements-recheck! 'goblin)", &env);
    // The claim no longer conforms, and the drifted method is named.
    assert!(drifted.contains("(CONFORMS)"), "got: {drifted}");
    assert!(drifted.contains("(DRIFT GOBLIN-GREET)"), "got: {drifted}");
}
