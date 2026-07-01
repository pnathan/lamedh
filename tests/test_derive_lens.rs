mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

#[test]
fn derive_lens_generates_view_and_build_directions() {
    let env = env_with_stdlib();
    eval_line(
        "(defconcept invoice (:fields ((id int64) (amount int64) (status symbol))))",
        &env,
    );
    assert_eq!(eval_line("(derive invoice lens)", &env), "INVOICE");
    assert_eq!(
        eval_line("(invoice->plist (make-invoice 7 125 'draft))", &env),
        "((ID . 7) (AMOUNT . 125) (STATUS . DRAFT))"
    );
    assert_eq!(
        eval_line(
            "(plist->invoice (invoice->plist (make-invoice 7 125 'draft)))",
            &env
        ),
        "(INVOICE 7 125 DRAFT)"
    );
}

#[test]
fn derive_lens_attaches_a_round_trip_law() {
    let env = env_with_stdlib();
    eval_line(
        "(defconcept invoice (:fields ((id int64) (amount int64))))",
        &env,
    );
    eval_line("(derive invoice lens)", &env);
    assert_eq!(
        eval_line("(invoice-lens-roundtrip (make-invoice 1 100))", &env),
        "T"
    );
    let laws = eval_line("(cdr (assoc 'laws (condense-trace 'invoice)))", &env);
    assert!(laws.contains("INVOICE-LENS-ROUNDTRIP"), "got: {laws}");
}

#[test]
fn derive_installs_typeclass_instances() {
    let env = env_with_stdlib();
    eval_line(
        "(defconcept invoice (:fields ((id int64) (amount int64))))",
        &env,
    );
    eval_line("(derive invoice lens equality printer)", &env);
    assert_eq!(
        eval_line("(typeclass-op 'eqv 'invoice :eqv)", &env),
        "INVOICE-EQUAL"
    );
    assert_eq!(
        eval_line("(typeclass-op 'show 'invoice :show)", &env),
        "INVOICE->PLIST"
    );
    assert_eq!(
        eval_line(
            "(typeclass-call 'lens 'invoice :view (make-invoice 1 2))",
            &env
        ),
        "((ID . 1) (AMOUNT . 2))"
    );
    let instances = eval_line("(cdr (assoc 'instances (condense-trace 'invoice)))", &env);
    assert!(instances.contains("LENS"), "got: {instances}");
    assert!(instances.contains("EQV"), "got: {instances}");
    assert!(instances.contains("SHOW"), "got: {instances}");
}

#[test]
fn derive_equality_instance_dispatches_through_typeclass_call() {
    let env = env_with_stdlib();
    eval_line(
        "(defconcept invoice (:fields ((id int64) (amount int64))))",
        &env,
    );
    eval_line("(derive invoice equality)", &env);
    assert_eq!(
        eval_line(
            "(typeclass-call 'eqv 'invoice :eqv (make-invoice 1 2) (make-invoice 1 2))",
            &env
        ),
        "T"
    );
    assert_eq!(
        eval_line(
            "(typeclass-call 'eqv 'invoice :eqv (make-invoice 1 2) (make-invoice 9 2))",
            &env
        ),
        "()"
    );
}

#[test]
fn lens_generated_symbols_join_the_condensation_registry() {
    let env = env_with_stdlib();
    eval_line(
        "(defconcept invoice (:fields ((id int64) (amount int64))))",
        &env,
    );
    eval_line("(derive invoice lens)", &env);
    let generated = eval_line("(condense-generated 'invoice)", &env);
    assert!(generated.contains("PLIST->INVOICE"), "got: {generated}");
    assert!(generated.contains("INVOICE->PLIST"), "got: {generated}");
    assert!(
        generated.contains("INVOICE-LENS-ROUNDTRIP"),
        "got: {generated}"
    );
}
