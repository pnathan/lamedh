mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

#[test]
fn condense_put_and_get_metadata() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("(condense-put 'invoice \"condense.kind\" 'concept)", &env),
        "T"
    );
    assert_eq!(
        eval_line("(condense-get 'invoice \"condense.kind\")", &env),
        "CONCEPT"
    );
    assert_eq!(eval_line("(condense-kind 'invoice)", &env), "CONCEPT");
}

#[test]
fn condense_record_sets_core_trace_fields() {
    let env = env_with_stdlib();
    eval_line(
        "(condense-record! 'invoice 'concept '(defconcept invoice) '(progn) '(make-invoice invoice-p))",
        &env,
    );

    assert_eq!(eval_line("(condense-kind 'invoice)", &env), "CONCEPT");
    assert_eq!(
        eval_line("(condense-source 'invoice)", &env),
        "(DEFCONCEPT INVOICE)"
    );
    assert_eq!(eval_line("(condense-expansion 'invoice)", &env), "(PROGN)");
    assert_eq!(
        eval_line("(condense-generated 'invoice)", &env),
        "(MAKE-INVOICE INVOICE-P)"
    );
}

#[test]
fn condense_trace_returns_inspectable_alist() {
    let env = env_with_stdlib();
    eval_line(
        "(condense-record! 'invoice 'concept '(defconcept invoice) '(progn) '(make-invoice invoice-p))",
        &env,
    );

    assert_eq!(
        eval_line("(cdr (assoc 'kind (condense-trace 'invoice)))", &env),
        "CONCEPT"
    );
    assert_eq!(
        eval_line("(cdr (assoc 'generated (condense-trace 'invoice)))", &env),
        "(MAKE-INVOICE INVOICE-P)"
    );
    assert_eq!(
        eval_line(
            "(cdr (assoc 'check-status (condense-trace 'invoice)))",
            &env
        ),
        "()"
    );
}
