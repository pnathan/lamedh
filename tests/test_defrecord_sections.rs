mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

#[test]
fn defrecord_generates_basic_operations() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line(
            "(defrecord invoice (:fields ((id int64) (amount int64) (status symbol))))",
            &env
        ),
        "INVOICE"
    );

    // One body (#308): concept values are branded records, printed in the
    // readable #S form that round-trips through the reader.
    assert_eq!(
        eval_line("(make-invoice 7 125 'draft)", &env),
        "#S(INVOICE 7 125 DRAFT)"
    );
    assert_eq!(
        eval_line("(invoice-p (make-invoice 7 125 'draft))", &env),
        "T"
    );
    assert_eq!(
        eval_line("(invoice-id (make-invoice 7 125 'draft))", &env),
        "7"
    );
    assert_eq!(
        eval_line("(invoice-status (make-invoice 7 125 'draft))", &env),
        "DRAFT"
    );
}

#[test]
fn defrecord_validator_checks_invariant() {
    let env = env_with_stdlib();
    eval_line(
        "(defrecord invoice (:fields ((id int64) (amount int64) (status symbol))) (:invariant (>= amount 0)))",
        &env,
    );

    assert_eq!(
        eval_line("(validate-invoice (make-invoice 1 10 'draft))", &env),
        "T"
    );
    // 0.3: make- ENFORCES the invariant at construction.
    let out = eval_line("(make-invoice 1 -10 'draft)", &env);
    assert!(out.contains("invariant violated"), "got: {out}");
    // The validator still judges values that bypass the make- door.
    assert_eq!(
        eval_line(
            "(validate-invoice (record-with (make-invoice 1 10 'draft) 'amount -10))",
            &env
        ),
        "()"
    );
}

#[test]
fn defrecord_records_trace_metadata() {
    let env = env_with_stdlib();
    eval_line(
        "(defrecord invoice (:fields ((id int64) (amount int64) (status symbol))) (:invariant (>= amount 0)))",
        &env,
    );

    assert_eq!(eval_line("(condense-kind 'invoice)", &env), "RECORD");
    assert_eq!(
        eval_line("(cdr (assoc 'kind (condense-trace 'invoice)))", &env),
        "RECORD"
    );
    assert_eq!(
        eval_line("(condense-generated 'invoice)", &env),
        "(MAKE-INVOICE INVOICE-P VALIDATE-INVOICE INVOICE-ID INVOICE-AMOUNT INVOICE-STATUS)"
    );
}

#[test]
fn defrecord_requires_fields() {
    let env = env_with_stdlib();
    let out = eval_line("(defrecord empty)", &env);
    assert!(out.contains("Error"), "got: {out}");
    assert!(out.contains("at least one field"), "got: {out}");
}
