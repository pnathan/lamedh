mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

#[test]
fn embedded_stdlib_loads_typeclass_surface() {
    let env = lamedh::environment::Environment::with_stdlib();

    assert_eq!(
        eval_line(
            "(deftypeclass eqv (a) (:ops ((eqv (-> (a a) bool)))))",
            &env
        ),
        "EQV"
    );
    assert_eq!(eval_line("(typeclass-kind 'eqv)", &env), "TYPECLASS");
}

#[test]
fn deftypeclass_records_trace_metadata() {
    let env = env_with_stdlib();

    assert_eq!(
        eval_line(
            "(deftypeclass eqv (a) (:ops ((eqv (-> (a a) bool)))))",
            &env
        ),
        "EQV"
    );
    assert_eq!(eval_line("(typeclass-kind 'eqv)", &env), "TYPECLASS");
    assert_eq!(
        eval_line("(cdr (assoc 'params (typeclass-trace 'eqv)))", &env),
        "(A)"
    );
    assert_eq!(
        eval_line("(cdr (assoc 'ops (typeclass-trace 'eqv)))", &env),
        "((EQV (-> (A A) BOOL)))"
    );
}

#[test]
fn definstance_resolves_keyword_operation() {
    let env = env_with_stdlib();
    eval_line(
        "(deftypeclass eqv (a) (:ops ((eqv (-> (a a) bool)))))",
        &env,
    );
    eval_line("(defun invoice-equal (a b) (equal a b))", &env);

    assert_eq!(
        eval_line("(definstance eqv invoice (:eqv invoice-equal))", &env),
        "INVOICE"
    );
    assert_eq!(
        eval_line("(resolve-instance 'eqv 'invoice)", &env),
        "(INVOICE (EQV . INVOICE-EQUAL))"
    );
    assert_eq!(
        eval_line("(typeclass-op 'eqv 'invoice :eqv)", &env),
        "INVOICE-EQUAL"
    );
    assert_eq!(
        eval_line(
            "(typeclass-call 'eqv 'invoice :eqv '(invoice 1) '(invoice 1))",
            &env
        ),
        "T"
    );
}

#[test]
fn definstance_replaces_existing_instance() {
    let env = env_with_stdlib();
    eval_line(
        "(deftypeclass show (a) (:ops ((show (-> (a) string)))))",
        &env,
    );
    eval_line("(defun show-old (x) \"old\")", &env);
    eval_line("(defun show-new (x) \"new\")", &env);

    eval_line("(definstance show invoice (:show show-old))", &env);
    eval_line("(definstance show invoice (:show show-new))", &env);

    assert_eq!(
        eval_line("(typeclass-op 'show 'invoice 'show)", &env),
        "SHOW-NEW"
    );
    assert_eq!(
        eval_line("(cdr (assoc 'instances (typeclass-trace 'show)))", &env),
        "((INVOICE (SHOW . SHOW-NEW)))"
    );
}

#[test]
fn defcap_and_cap_op_aliases_work() {
    let env = env_with_stdlib();
    eval_line(
        "(defcap printable (a) (:ops ((render (-> (a) string)))))",
        &env,
    );
    eval_line("(defun render-int (x) (princ-to-string x))", &env);
    eval_line("(definstance printable int64 (:render render-int))", &env);

    assert_eq!(
        eval_line("(cap-op 'printable 'int64 :render)", &env),
        "RENDER-INT"
    );
    assert_eq!(
        eval_line("(resolve-cap 'printable 'int64)", &env),
        "(INT64 (RENDER . RENDER-INT))"
    );
}

#[test]
fn missing_instance_reports_clear_error() {
    let env = env_with_stdlib();
    eval_line(
        "(deftypeclass eqv (a) (:ops ((eqv (-> (a a) bool)))))",
        &env,
    );

    let out = eval_line("(resolve-instance 'eqv 'missing)", &env);
    assert!(out.contains("Error"), "got: {out}");
    assert!(out.contains("missing typeclass instance"), "got: {out}");
}

#[test]
fn instance_validation_rejects_missing_and_unknown_ops() {
    let env = env_with_stdlib();
    eval_line(
        "(deftypeclass pairwise (a) (:ops ((same (-> (a a) bool)) (render (-> (a) string)))))",
        &env,
    );
    eval_line("(defun same-invoice (a b) (equal a b))", &env);
    eval_line("(defun render-invoice (x) \"invoice\")", &env);

    let missing = eval_line("(definstance pairwise invoice (:same same-invoice))", &env);
    assert!(missing.contains("Error"), "got: {missing}");
    assert!(missing.contains("missing operation"), "got: {missing}");

    let unknown = eval_line(
        "(definstance pairwise invoice (:same same-invoice) (:render render-invoice) (:extra render-invoice))",
        &env,
    );
    assert!(unknown.contains("Error"), "got: {unknown}");
    assert!(unknown.contains("unknown operation"), "got: {unknown}");
}
