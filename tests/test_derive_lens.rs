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
fn defconcept_inline_derive_builds_the_whole_bundle_from_one_seed() {
    let env = env_with_stdlib();
    eval_line(
        "(defconcept invoice (:fields ((id int64) (amount int64))) (:invariant (>= amount 0)) (:derive equality lens))",
        &env,
    );
    assert_eq!(
        eval_line(
            "(invoice-equal (make-invoice 1 2) (make-invoice 1 2))",
            &env
        ),
        "T"
    );
    assert_eq!(
        eval_line("(invoice-lens-roundtrip (make-invoice 1 2))", &env),
        "T"
    );
    let derivations = eval_line("(cdr (assoc 'derivations (condense-trace 'invoice)))", &env);
    assert!(derivations.contains("EQUALITY"), "got: {derivations}");
    assert!(derivations.contains("LENS"), "got: {derivations}");
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
