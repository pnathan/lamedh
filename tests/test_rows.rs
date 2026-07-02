mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

fn env_with_invoice() -> lamedh::Shared<lamedh::environment::Environment> {
    let env = env_with_stdlib();
    eval_line(
        "(defconcept invoice (:fields ((id int64) (amount int64))) (:invariant (>= amount 0)))",
        &env,
    );
    eval_line(
        "(defconcept receipt (:fields ((id int64) (total int64))))",
        &env,
    );
    env
}

#[test]
fn concept_accessors_carry_declared_row_schemes() {
    let env = env_with_invoice();
    assert_eq!(
        eval_line("(see-type 'invoice-amount)", &env),
        "(DECLARED (FORALL (A) (-> ((RECORD ((AMOUNT INT64)) A)) INT64)))"
    );
    let ctor = eval_line("(see-type 'make-invoice)", &env);
    assert!(ctor.contains("DECLARED"), "got: {ctor}");
    assert!(
        ctor.contains("(RECORD ((AMOUNT INT64) (ID INT64)))"),
        "constructor should return a closed record, got: {ctor}"
    );
}

#[test]
fn functions_over_accessors_infer_row_polymorphic_schemes() {
    let env = env_with_invoice();
    // "any record with an int64 amount, and the rest is r" — inferred, not
    // written. The two arguments get independent rows.
    assert_eq!(
        eval_line(
            "(check-type (defun sum-amounts (x y) (+ (invoice-amount x) (invoice-amount y))))",
            &env
        ),
        "\"SUM-AMOUNTS : (forall (a b) (-> ((record ((amount int64)) a) (record ((amount int64)) b)) int64))\""
    );
    // Two accessors on one argument merge into one row.
    let both = eval_line(
        "(check-type (defun spend (x) (+ (invoice-amount x) (invoice-id x))))",
        &env,
    );
    assert!(
        both.contains("(record ((amount int64) (id int64)) a)"),
        "got: {both}"
    );
}

#[test]
fn constructor_closed_record_grounds_row_calls() {
    let env = env_with_invoice();
    assert_eq!(
        eval_line(
            "(check-type (defun ok () (invoice-amount (make-invoice 1 5))))",
            &env
        ),
        "\"OK : (-> () int64)\""
    );
}

#[test]
fn cross_concept_misuse_is_a_static_type_error() {
    let env = env_with_invoice();
    eval_line("(defun bad () (receipt-total (make-invoice 1 5)))", &env);
    let verdict = eval_line("(see-type 'bad)", &env);
    assert!(verdict.starts_with("(TYPE-ERROR"), "got: {verdict}");
    assert!(verdict.contains("lacks field"), "got: {verdict}");
    // Applying an accessor to a non-record is equally an error.
    eval_line("(defun bad2 (x) (invoice-amount (invoice-id x)))", &env);
    let verdict2 = eval_line("(see-type 'bad2)", &env);
    assert!(verdict2.starts_with("(TYPE-ERROR"), "got: {verdict2}");
}

#[test]
fn derived_equality_gets_an_informative_row_scheme() {
    let env = env_with_invoice();
    eval_line("(derive invoice equality)", &env);
    let same = eval_line("(check-type (defun same (x y) (invoice-equal x y)))", &env);
    assert!(
        same.contains("(record ((amount int64) (id int64)) a)"),
        "got: {same}"
    );
    assert!(same.contains("bool"), "got: {same}");
}

#[test]
fn lens_round_trip_checks_end_to_end() {
    let env = env_with_invoice();
    eval_line("(derive invoice lens)", &env);
    assert_eq!(
        eval_line(
            "(check-type (defun rt () (invoice-amount (plist->invoice (invoice->plist (make-invoice 1 5))))))",
            &env
        ),
        "\"RT : (-> () int64)\""
    );
}

#[test]
fn edit_barrier_rejects_row_misuse_and_rolls_back() {
    let env = env_with_invoice();
    eval_line("(defun spend (x) (+ (invoice-amount x) 1))", &env);
    assert_eq!(
        eval_line(
            "(errorset '(edit! 'spend '(((+ (invoice-amount x) 1) (+ (invoice-amount (invoice-id x)) 1)))))",
            &env
        ),
        "()"
    );
    // Rollback: the original row-typed definition survives.
    assert_eq!(eval_line("(spend (make-invoice 1 5))", &env), "6");
}

#[test]
fn unmappable_field_types_fall_back_to_undeclared() {
    let env = env_with_stdlib();
    eval_line("(defconcept bag (:fields ((items list))))", &env);
    // No declaration installed: the accessor reports its (vacuous) inferred
    // scheme, honestly.
    let verdict = eval_line("(see-type 'bag-items)", &env);
    assert!(verdict.starts_with("(CHECKED"), "got: {verdict}");
    assert_eq!(
        eval_line("(cadr (condense-check-type-one 'bag-items))", &env),
        "VACUOUS"
    );
}

#[test]
fn row_accessor_reads_the_named_field_regardless_of_layout() {
    // Seam C: a DECLARED row scheme is by-name, so the accessor must read by
    // name. ARMORED puts hp at slot 2; SLIME puts it at slot 1. Applying
    // ARMORED-HP to a SLIME type-checks (the closed slime record has an int64
    // hp), and must now *return that hp*, not the garbage at a fixed offset.
    let env = env_with_stdlib();
    eval_line(
        "(defconcept armored (:fields ((armor int64) (hp int64))))",
        &env,
    );
    eval_line("(defconcept slime (:fields ((hp int64))))", &env);
    eval_line("(defun probe () (armored-hp (make-slime 9)))", &env);
    // The cross-concept call type-checks as int64...
    assert_eq!(
        eval_line("(see-type 'probe)", &env),
        "(CHECKED (-> () INT64))"
    );
    // ...and the runtime now agrees with the type: 9, not NIL.
    assert_eq!(eval_line("(probe)", &env), "9");
    // Same-concept access is unchanged.
    assert_eq!(eval_line("(armored-hp (make-armored 5 42))", &env), "42");
    assert_eq!(eval_line("(armored-armor (make-armored 5 42))", &env), "5");
}

#[test]
fn declare_type_validates_its_input() {
    let env = env_with_stdlib();
    // Duplicate record labels are rejected.
    assert_eq!(
        eval_line(
            "(errorset '(declare-type! 'x '(record ((a int64) (a bool)))))",
            &env
        ),
        "()"
    );
    // A row tail must be a type variable.
    assert_eq!(
        eval_line(
            "(errorset '(declare-type! 'x '(forall (r) (record ((a int64)) int64))))",
            &env
        ),
        "()"
    );
    // A well-formed declaration echoes its rendered scheme.
    let ok = eval_line(
        "(declare-type! 'my-get '(forall (r) (-> ((record ((amount int64)) r)) int64)))",
        &env,
    );
    assert!(ok.contains("RECORD"), "got: {ok}");
}

#[test]
fn declared_schemes_do_not_reach_the_native_tier() {
    let env = env_with_invoice();
    // A function whose type mentions records is checked but must never be
    // reported as a native/typed-island function.
    eval_line("(defun spend (x) (+ (invoice-amount x) 1))", &env);
    let verdict = eval_line("(see-type 'spend)", &env);
    assert!(verdict.starts_with("(CHECKED"), "got: {verdict}");
    assert!(!verdict.contains("COMPILED"), "got: {verdict}");
}
