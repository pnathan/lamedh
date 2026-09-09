//! Integration tests for the portable Hindley-Milner checker
//! (lib/45-hm-check.lisp, issue #451): HM-CHECK-LAMBDA / HM-CHECK-EXPR over
//! the module's small core language, pinning the verdict shape (CHECKED /
//! TYPE-ERROR / DYNAMIC), the row-polymorphism it exists to confirm, and
//! the nominal (named) type application that stands in for protocol
//! dispatch's representation needs. See the module's own header comment
//! for what is deliberately NOT covered (full Lamedh surface syntax,
//! actual dispatch resolution, and wiring into `defun`).

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line};

fn env() -> Shared<Environment> {
    Environment::with_stdlib()
}

#[test]
fn identity_generalizes_to_a_polymorphic_scheme() {
    let e = env();
    assert_eq!(
        eval_line("(hm-check-lambda '(x) '(x))", &e),
        "(CHECKED (FORALL (A) (-> (A) A)))"
    );
}

#[test]
fn application_infers_the_classic_apply_type() {
    let e = env();
    assert_eq!(
        eval_line("(hm-check-lambda '(f x) '((f x)))", &e),
        "(CHECKED (FORALL (A B) (-> ((-> (A) B) A) B)))"
    );
}

#[test]
fn if_unifies_both_branches() {
    let e = env();
    assert_eq!(
        eval_line("(hm-check-lambda '(x) '((if x 1 2)))", &e),
        "(CHECKED (-> (BOOL) INT64))"
    );
    let mismatch = eval_line("(hm-check-lambda '(x) '((if x 1 \"s\")))", &e);
    assert_eq!(mismatch, "(TYPE-ERROR \"cannot unify INT64 with STRING\")");
}

#[test]
fn let_polymorphism_lets_one_binding_serve_two_types() {
    let e = env();
    // The classic HM let-polymorphism example: ID is used at INT64 and at
    // STRING from one monomorphic LAMBDA, licensed by generalizing ID's
    // scheme at the LET before either use instantiates it separately.
    assert_eq!(
        eval_line(
            "(hm-check-expr '(let ((id (lambda (y) y))) (mk-record (a (id 1)) (b (id \"s\")))))",
            &e
        ),
        "(CHECKED (RECORD ((A . INT64) (B . STRING)) ()))"
    );
}

#[test]
fn record_field_access_infers_an_open_row_polymorphic_type() {
    let e = env();
    // The #451 row-polymorphism confirmation: FIELD-REF accepts any record
    // naming at least an X field, expressed as an open row type variable B.
    assert_eq!(
        eval_line("(hm-check-lambda '(r) '((field-ref r x)))", &e),
        "(CHECKED (FORALL (A B) (-> ((RECORD ((X . A)) B)) A)))"
    );
}

#[test]
fn record_field_access_round_trips_through_construction() {
    let e = env();
    assert_eq!(
        eval_line("(hm-check-expr '(field-ref (mk-record (x 1) (y 2)) x))", &e),
        "(CHECKED INT64)"
    );
}

#[test]
fn closed_record_construction_rejects_a_missing_field() {
    let e = env();
    let verdict = eval_line(
        "(hm-check-lambda '(x) '((the (record ((x int64))) (mk-record (y 1)))))",
        &e,
    );
    assert_eq!(
        verdict,
        "(TYPE-ERROR \"record fields disagree: {X} vs {Y}\")"
    );
}

#[test]
fn named_nominal_types_unify_by_name_and_arguments() {
    let e = env();
    // The #451 protocol-dispatch representation confirmation: a NAMED
    // application unifies with another NAMED application of the same name
    // by unifying arguments pairwise (mirroring one case of Ty::App); two
    // uses of X ascribed to the same named type in different branches must
    // agree.
    assert_eq!(
        eval_line(
            "(hm-check-lambda '(x) '((if t (the (point int64 int64) x) (the (point int64 int64) x))))",
            &e
        ),
        "(CHECKED (-> ((NAMED POINT INT64 INT64)) (NAMED POINT INT64 INT64)))"
    );
    let mismatch = eval_line(
        "(hm-check-lambda '(x) '((if t (the (point int64 int64) x) (the (shape int64) x))))",
        &e,
    );
    assert_eq!(
        mismatch,
        "(TYPE-ERROR \"cannot unify (NAMED SHAPE INT64) with (NAMED POINT INT64 INT64)\")"
    );
}

#[test]
fn occurs_check_rejects_a_self_application() {
    let e = env();
    // (lambda (x) (x x)) would need x : (-> (x) b), an infinite type.
    let verdict = eval_line("(hm-check-lambda '(x) '((x x)))", &e);
    assert!(
        verdict.contains("TYPE-ERROR") && verdict.contains("occurs-check"),
        "expected an occurs-check type error, got: {verdict}"
    );
}

#[test]
fn unbound_variable_is_a_type_error_not_a_panic() {
    let e = env();
    assert_eq!(
        eval_line("(hm-check-expr 'nowhere-defined)", &e),
        "(TYPE-ERROR \"unbound variable NOWHERE-DEFINED\")"
    );
}

#[test]
fn variadic_parameter_lists_are_reported_dynamic_not_checked() {
    let e = env();
    // Matches the native checker's own scope: `checker_lambda_source`
    // excludes any lambda with a rest parameter from checking.
    for form in [
        "(hm-check-lambda '(x &rest r) '(x))",
        "(hm-check-lambda '(x &optional y) '(x))",
        "(hm-check-lambda '(&key k) '(k))",
    ] {
        let verdict = eval_line(form, &e);
        assert!(
            verdict.starts_with("(DYNAMIC"),
            "expected DYNAMIC for {form}, got: {verdict}"
        );
    }
}

#[test]
fn checked_scheme_renders_compatibly_with_condense_vacuous_p() {
    // A rendered CHECKED scheme from this checker uses the same
    // `(forall (vars) ty)` shape as `src/jit/infer.rs`'s scheme_name, so it
    // is a drop-in argument to lib/20-condensation.lisp's
    // CONDENSE-VACUOUS-P (issue #451's row-polymorphism/protocol
    // groundwork is meant to interoperate with the existing condensation
    // layer, not just resemble it).
    let e = env();
    // (-> (a) a): the result mentions the same variable an argument does,
    // so it is NOT vacuous.
    assert_eq!(
        eval_line(
            "(condense-vacuous-p (cadr (hm-check-lambda '(x) '(x))))",
            &e
        ),
        "()"
    );
    // (-> (a) b): a free result variable no argument constrains IS vacuous.
    assert_eq!(
        eval_line("(condense-vacuous-p '(forall (a b) (-> (a) b)))", &e),
        "T"
    );
}
