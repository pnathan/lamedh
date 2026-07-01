mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

fn env_with_concepts() -> lamedh::Shared<lamedh::environment::Environment> {
    let env = env_with_stdlib();
    eval_line(
        "(defconcept invoice (:fields ((id int64) (amount int64))) (:invariant (>= amount 0)))",
        &env,
    );
    eval_line(
        "(defconcept receipt (:fields ((id int64) (total int64))))",
        &env,
    );
    eval_line("(derive invoice equality)", &env);
    eval_line("(derive receipt equality)", &env);
    env
}

#[test]
fn embedded_stdlib_loads_intent_surface_and_standard_classes() {
    let env = lamedh::environment::Environment::with_stdlib();
    assert_eq!(eval_line("(typeclass-kind 'eqv)", &env), "TYPECLASS");
    assert_eq!(eval_line("(typeclass-kind 'show)", &env), "TYPECLASS");
    assert_eq!(eval_line("(typeclass-kind 'lens)", &env), "TYPECLASS");
    assert_eq!(eval_line("(intent-registry)", &env), "()");
}

#[test]
fn defintent_records_subject_means_outcome() {
    let env = env_with_concepts();
    assert_eq!(
        eval_line(
            "(defintent same-invoice (:subject invoice) (:means (eqv eqv)))",
            &env
        ),
        "SAME-INVOICE"
    );
    assert_eq!(eval_line("(intent-p 'same-invoice)", &env), "T");
    assert_eq!(eval_line("(intent-subject 'same-invoice)", &env), "INVOICE");
    assert_eq!(eval_line("(intent-means 'same-invoice)", &env), "(EQV EQV)");
    let trace = eval_line("(intent-trace 'same-invoice)", &env);
    assert!(trace.contains("(GROUND . T)"), "got: {trace}");
    assert!(
        trace.contains("(REALIZED)"),
        "not realized yet, got: {trace}"
    );
}

#[test]
fn subject_type_dispatch_covers_concepts_and_ground_types() {
    let env = env_with_concepts();
    assert_eq!(
        eval_line("(intent-subject-type (make-invoice 1 5))", &env),
        "INVOICE"
    );
    assert_eq!(eval_line("(intent-subject-type 5)", &env), "INT64");
    assert_eq!(eval_line("(intent-subject-type 5.0)", &env), "FLOAT64");
    assert_eq!(eval_line("(intent-subject-type \"x\")", &env), "STRING");
    assert_eq!(eval_line("(intent-subject-type 'x)", &env), "SYMBOL");
    assert_eq!(eval_line("(intent-subject-type '(1 2))", &env), "LIST");
}

#[test]
fn polymorphic_intent_dispatches_by_runtime_subject_type() {
    let env = env_with_concepts();
    eval_line(
        "(defintent same-thing (:subject a) (:means (eqv eqv)))",
        &env,
    );
    assert_eq!(
        eval_line(
            "(intent-apply 'same-thing (make-invoice 1 5) (make-invoice 1 5))",
            &env
        ),
        "T"
    );
    assert_eq!(
        eval_line(
            "(intent-apply 'same-thing (make-receipt 2 9) (make-receipt 2 9))",
            &env
        ),
        "T"
    );
    assert_eq!(
        eval_line(
            "(intent-apply 'same-thing (make-invoice 1 5) (make-invoice 8 5))",
            &env
        ),
        "()"
    );
}

#[test]
fn intents_sharing_a_means_are_discoverable() {
    let env = env_with_concepts();
    eval_line(
        "(defintent same-thing (:subject a) (:means (eqv eqv)))",
        &env,
    );
    eval_line(
        "(defintent same-invoice (:subject invoice) (:means (eqv eqv)))",
        &env,
    );
    assert_eq!(
        eval_line("(intents-for-means '(eqv eqv))", &env),
        "(SAME-THING SAME-INVOICE)"
    );
    assert_eq!(
        eval_line("(intents-for-subject 'invoice)", &env),
        "(SAME-INVOICE)"
    );
}

#[test]
fn ground_intent_realizes_to_a_direct_function() {
    let env = env_with_concepts();
    eval_line(
        "(defintent same-invoice (:subject invoice) (:means (eqv eqv)))",
        &env,
    );
    assert_eq!(
        eval_line("(intent-realize same-invoice)", &env),
        "SAME-INVOICE"
    );
    // The realized function calls the concrete method directly — no dictionary.
    let source = eval_line("(see-source 'same-invoice)", &env);
    assert!(source.contains("INVOICE-EQUAL"), "got: {source}");
    assert!(!source.contains("TYPECLASS"), "got: {source}");
    assert_eq!(
        eval_line("(same-invoice (make-invoice 1 5) (make-invoice 1 5))", &env),
        "T"
    );
    // Realization joins the condensation registry with checker status recorded.
    assert_eq!(
        eval_line("(cdr (assoc 'kind (condense-trace 'same-invoice)))", &env),
        "INTENT"
    );
    let status = eval_line(
        "(cdr (assoc 'check-status (condense-trace 'same-invoice)))",
        &env,
    );
    assert!(status.contains("SAME-INVOICE"), "got: {status}");
    let trace = eval_line("(intent-trace 'same-invoice)", &env);
    assert!(trace.contains("(REALIZED . T)"), "got: {trace}");
}

#[test]
fn realize_rejects_polymorphic_subjects_and_missing_instances() {
    let env = env_with_concepts();
    eval_line(
        "(defintent same-thing (:subject a) (:means (eqv eqv)))",
        &env,
    );
    assert_eq!(
        eval_line("(errorset '(intent-realize same-thing))", &env),
        "()"
    );
    // string is a ground type but has no EQV instance: a realize-time error,
    // the dynamic analogue of "missing ground instance is a type error".
    eval_line(
        "(defintent same-string (:subject string) (:means (eqv eqv)))",
        &env,
    );
    assert_eq!(
        eval_line("(errorset '(intent-realize same-string))", &env),
        "()"
    );
}

#[test]
fn outcome_contracts_are_enforced_on_apply_and_realized_paths() {
    let env = env_with_concepts();
    eval_line(
        "(defintent normalize (:subject invoice) (:means (lens view)) (:outcome (consp *result*)))",
        &env,
    );
    eval_line("(derive invoice lens)", &env);
    assert_eq!(
        eval_line("(intent-apply 'normalize (make-invoice 1 5))", &env),
        "((ID . 1) (AMOUNT . 5))"
    );
    // A means whose result violates the outcome contract fails loudly.
    eval_line(
        "(defintent bad-outcome (:subject invoice) (:means (eqv eqv)) (:outcome (consp *result*)))",
        &env,
    );
    assert_eq!(
        eval_line(
            "(errorset '(intent-apply 'bad-outcome (make-invoice 1 5) (make-invoice 1 5)))",
            &env
        ),
        "()"
    );
    // Realized form carries the same contract.
    eval_line("(intent-realize bad-outcome)", &env);
    assert_eq!(
        eval_line(
            "(errorset '(bad-outcome (make-invoice 1 5) (make-invoice 1 5)))",
            &env
        ),
        "()"
    );
}

#[test]
fn subject_mismatch_is_an_error() {
    let env = env_with_concepts();
    eval_line(
        "(defintent same-invoice (:subject invoice) (:means (eqv eqv)))",
        &env,
    );
    assert_eq!(
        eval_line(
            "(errorset '(intent-apply 'same-invoice (make-receipt 2 9) (make-receipt 2 9)))",
            &env
        ),
        "()"
    );
}
