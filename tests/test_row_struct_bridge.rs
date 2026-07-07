//! Integration tests for #297 stage 2: nominal structs subsume into record
//! rows in the checker — a `defstruct-typed` value is its closed row plus
//! identity, so one row-typed function accepts every structurally-
//! conforming struct while nominal identity stays intact. Checker-only:
//! rows never reach codegen.

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line};

fn env() -> Shared<Environment> {
    let e = Environment::with_stdlib();
    eval_line("(defstruct-typed Goblin (hp int64) (gold int64))", &e);
    eval_line("(defstruct-typed Dragon (hp int64) (fire float64))", &e);
    eval_line("(defstruct-typed Point (x int64) (y int64))", &e);
    eval_line(
        "(declare-type! 'hp-of '(forall (r) (-> ((record ((hp int64)) r)) int64)))",
        &e,
    );
    e
}

#[test]
fn conforming_structs_flow_through_a_row_typed_function() {
    let e = env();
    assert_eq!(
        eval_line("(check-type (hp-of (make-goblin 7 100)))", &e),
        "\"int64\""
    );
    assert_eq!(
        eval_line("(check-type (hp-of (make-dragon 9 2.5)))", &e),
        "\"int64\"",
        "a second, differently-shaped struct conforms through the same row"
    );
}

#[test]
fn nonconforming_struct_is_rejected_with_the_missing_field() {
    let e = env();
    let out = eval_line("(check-type (hp-of (make-point 1 2)))", &e);
    assert!(
        out.contains("POINT") && out.contains("hp"),
        "expected a missing-field rejection naming the struct and field, got: {out}"
    );
}

#[test]
fn field_type_mismatch_is_rejected() {
    let e = env();
    eval_line(
        "(declare-type! 'fhp '(forall (r) (-> ((record ((hp float64)) r)) float64)))",
        &e,
    );
    let out = eval_line("(check-type (fhp (make-goblin 7 100)))", &e);
    assert!(
        out.contains("type error"),
        "int64 hp must not satisfy a float64 hp row, got: {out}"
    );
}

#[test]
fn row_requirements_propagate_through_inference() {
    let e = env();
    assert_eq!(
        eval_line("(check-type (defun raid (g) (+ 1 (hp-of g))))", &e),
        "\"RAID : (forall (a) (-> ((record ((hp int64)) a)) int64))\""
    );
}

#[test]
fn nominal_identity_is_preserved() {
    let e = env();
    // Same field sets, distinct brands: still not unifiable nominally.
    eval_line("(defstruct-typed Foo (v int64))", &e);
    eval_line("(defstruct-typed Bar (v int64))", &e);
    eval_line("(defun-typed (foo-only int64) ((f Foo)) (foo-v f))", &e);
    let out = eval_line("(check-type (foo-only (make-bar 3)))", &e);
    assert!(
        out.contains("type error"),
        "Bar must not pass where Foo is demanded, got: {out}"
    );
    // But both flow through a row that names their shared field.
    eval_line(
        "(declare-type! 'v-of '(forall (r) (-> ((record ((v int64)) r)) int64)))",
        &e,
    );
    assert_eq!(
        eval_line("(check-type (v-of (make-foo 1)))", &e),
        "\"int64\""
    );
    assert_eq!(
        eval_line("(check-type (v-of (make-bar 2)))", &e),
        "\"int64\""
    );
}

#[test]
fn closed_records_demand_exact_field_sets() {
    let e = env();
    eval_line(
        "(declare-type! 'exact-hp '(-> ((record ((hp int64)))) int64))",
        &e,
    );
    let out = eval_line("(check-type (exact-hp (make-goblin 7 100)))", &e);
    assert!(
        out.contains("type error") && out.to_lowercase().contains("gold"),
        "a closed record must reject the struct's extra field, got: {out}"
    );
}
