mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

#[test]
fn derive_generates_printer_from_concept_metadata() {
    let env = env_with_stdlib();
    eval_line(
        "(defrecord invoice (:fields ((id int64) (amount int64) (status symbol))))",
        &env,
    );
    assert_eq!(eval_line("(derive invoice printer)", &env), "INVOICE");
    assert_eq!(
        eval_line("(invoice->plist (make-invoice 7 125 'draft))", &env),
        "((ID . 7) (AMOUNT . 125) (STATUS . DRAFT))"
    );
}

#[test]
fn derive_generates_equality_from_concept_metadata() {
    let env = env_with_stdlib();
    eval_line(
        "(defrecord invoice (:fields ((id int64) (amount int64) (status symbol))))",
        &env,
    );
    assert_eq!(eval_line("(derive invoice equality)", &env), "INVOICE");
    assert_eq!(
        eval_line(
            "(invoice-equal (make-invoice 7 125 'draft) (make-invoice 7 125 'draft))",
            &env
        ),
        "T"
    );
    assert_eq!(
        eval_line(
            "(invoice-equal (make-invoice 7 125 'draft) (make-invoice 8 125 'draft))",
            &env
        ),
        "()"
    );
}

#[test]
fn derive_updates_trace_metadata() {
    let env = env_with_stdlib();
    eval_line(
        "(defrecord invoice (:fields ((id int64) (amount int64) (status symbol))))",
        &env,
    );
    eval_line("(derive invoice printer equality)", &env);

    assert_eq!(
        eval_line("(cdr (assoc 'derivations (condense-trace 'invoice)))", &env),
        "(PRINTER EQUALITY)"
    );
    assert_eq!(
        eval_line("(condense-generated 'invoice)", &env),
        "(MAKE-INVOICE INVOICE-P VALIDATE-INVOICE INVOICE-ID INVOICE-AMOUNT INVOICE-STATUS INVOICE->PLIST INVOICE-EQUAL)"
    );
}

#[test]
fn derive_rerun_does_not_duplicate_generated_metadata() {
    let env = env_with_stdlib();
    eval_line(
        "(defrecord invoice (:fields ((id int64) (amount int64) (status symbol))))",
        &env,
    );
    eval_line("(derive invoice printer)", &env);
    eval_line("(derive invoice printer equality)", &env);

    assert_eq!(
        eval_line("(cdr (assoc 'derivations (condense-trace 'invoice)))", &env),
        "(PRINTER EQUALITY)"
    );
    assert_eq!(
        eval_line("(condense-generated 'invoice)", &env),
        "(MAKE-INVOICE INVOICE-P VALIDATE-INVOICE INVOICE-ID INVOICE-AMOUNT INVOICE-STATUS INVOICE->PLIST INVOICE-EQUAL)"
    );
}

#[test]
fn derive_requires_record_metadata() {
    let env = env_with_stdlib();
    let out = eval_line("(derive missing printer)", &env);
    assert!(out.contains("Error"), "got: {out}");
    assert!(out.contains("record"), "got: {out}");
}
