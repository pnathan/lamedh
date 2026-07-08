//! record-ref / record-with (issue #308 stage A): representation-generic
//! record field access with checker-native row rules. The runtime half
//! closes #305 (native structs were opaque to generic accessors); the
//! checker half makes row types DERIVED end-to-end — no declare-type!
//! axioms needed for field access.

mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

fn env() -> lamedh::Shared<lamedh::environment::Environment> {
    let e = env_with_stdlib();
    eval_line("(defstruct-typed Coin (value int64))", &e);
    eval_line("(defstruct-typed Point (x int64) (y int64))", &e);
    eval_line("(defrecord chest (value int64) (items (list string)))", &e);
    e
}

#[test]
fn closes_305_native_structs_are_generically_accessible() {
    let e = env();
    // The exact #305 scenario: a compiled struct read through a generic
    // accessor, returning the right value (was: opaque, () via nth).
    assert_eq!(eval_line("(record-ref (make-coin 5) 'value)", &e), "5");
}

#[test]
fn one_primitive_both_representations() {
    let e = env();
    eval_line("(defun worth (x) (record-ref x 'value))", &e);
    assert_eq!(eval_line("(worth (make-coin 5))", &e), "5");
    assert_eq!(eval_line("(worth (make-chest 9 (list \"gold\")))", &e), "9");
    // Compound field through the same primitive.
    assert_eq!(
        eval_line("(record-ref (make-chest 1 (list \"a\" \"b\")) 'items)", &e),
        "(\"a\" \"b\")"
    );
}

#[test]
fn row_types_are_derived_with_no_axioms() {
    let e = env();
    // The crown: the row (and even the field type) come from the checker's
    // native record-ref rule — zero declare-type! in sight, and MORE general
    // than the hand-declared version (field type polymorphic).
    eval_line("(defun worth (x) (record-ref x 'value))", &e);
    assert_eq!(
        eval_line("(see-type 'worth)", &e),
        "(CHECKED (FORALL (A B) (-> ((RECORD ((VALUE A)) B)) A)))"
    );
}

#[test]
fn static_guards_fire_at_direct_sites() {
    let e = env();
    // Struct subsumption (#299) + the native rule: a Point has no value.
    let out = eval_line("(check-type (record-ref (make-point 1 2) 'value))", &e);
    assert!(
        out.contains("type error") && out.contains("POINT") && out.contains("value"),
        "got: {out}"
    );
    // Type-safe update: wrong replacement type is rejected statically.
    let out = eval_line(
        "(check-type (record-with (make-coin 5) 'value \"str\"))",
        &e,
    );
    assert!(
        out.contains("type error") && out.contains("string"),
        "got: {out}"
    );
}

#[test]
fn functional_update_works_on_both_representations() {
    let e = env();
    eval_line(
        "(defun bump (x) (record-with x 'value (+ 1 (record-ref x 'value))))",
        &e,
    );
    // Typed update preserves the record's row in the derived signature.
    assert_eq!(
        eval_line("(see-type 'bump)", &e),
        "(CHECKED (FORALL (A) (-> ((RECORD ((VALUE INT64)) A)) (RECORD ((VALUE INT64)) A))))"
    );
    // And the original is untouched (records are values).
    assert_eq!(
        eval_line(
            "(let ((c (make-coin 5))) (list (record-ref (bump c) 'value) (record-ref c 'value)))",
            &e
        ),
        "(6 5)"
    );
    assert_eq!(
        eval_line("(record-ref (bump (make-chest 9 (list))) 'value)", &e),
        "10"
    );
}

#[test]
fn errors_are_clean() {
    let e = env();
    let out = eval_line(
        "(handler-case (record-ref (make-coin 5) 'items) (error (er) (error-message er)))",
        &e,
    );
    assert!(out.contains("no field items"), "got: {out}");
    let out = eval_line(
        "(handler-case (record-ref 42 'value) (error (er) 'not-a-record))",
        &e,
    );
    assert_eq!(out, "NOT-A-RECORD");
    // A computed (non-quoted) field name is dynamic: checker degrades to
    // any rather than rejecting (the gradual frontier), runtime still works.
    assert_eq!(
        eval_line("(let ((f 'value)) (record-ref (make-coin 5) f))", &e),
        "5"
    );
}
