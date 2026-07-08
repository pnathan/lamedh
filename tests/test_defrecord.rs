//! DEFRECORD — the one-door gradual-typed record (lib/20-condensation.lisp).
//! One form, one positional representation, full row typing for every field
//! type (including compound), and compile-eligibility metadata for a future
//! native pass (#297 stage 3). The property that binds it together: any
//! defrecord value flows through any row-polymorphic function naming a
//! subset of its fields — at the checker AND at runtime (#305).

mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

fn env() -> lamedh::Shared<lamedh::environment::Environment> {
    let e = env_with_stdlib();
    eval_line("(defrecord coin (value int64))", &e);
    eval_line("(defrecord chest (value int64) (items (list string)))", &e);
    eval_line("(defun the-value (self) (nth 1 self))", &e);
    eval_line(
        "(declare-type! 'the-value '(forall (r) (-> ((record ((value int64)) r)) int64)))",
        &e,
    );
    eval_line("(defun worth (x) (the-value x))", &e);
    e
}

#[test]
fn one_row_function_accepts_every_record_at_runtime() {
    let e = env();
    // The money property: a scalar-only record and a list-field record both
    // flow through ONE row-polymorphic function — and actually return the
    // right value, not just type-check (contrast #305).
    assert_eq!(eval_line("(worth (make-coin 5))", &e), "5");
    assert_eq!(eval_line("(worth (make-chest 9 (list \"gold\")))", &e), "9");
}

#[test]
fn the_row_is_inferred_across_records() {
    let e = env();
    assert_eq!(
        eval_line("(see-type 'worth)", &e),
        "(CHECKED (FORALL (A) (-> ((RECORD ((VALUE INT64)) A)) INT64)))"
    );
}

#[test]
fn compound_fields_get_row_schemes_automatically() {
    let e = env();
    // The broadening: a (list string) field is row-typed without any hand-
    // written accessor. Previously a compound field disabled rows entirely.
    assert_eq!(
        eval_line("(see-type 'chest-items)", &e),
        "(DECLARED (FORALL (A) (-> ((RECORD ((ITEMS (LIST STRING))) A)) (LIST STRING))))"
    );
    // And it reads at runtime.
    assert_eq!(
        eval_line("(chest-items (make-chest 1 (list \"a\" \"b\")))", &e),
        "(\"a\" \"b\")"
    );
}

#[test]
fn generates_constructor_predicate_and_accessors() {
    let e = env();
    assert_eq!(eval_line("(coin-value (make-coin 7))", &e), "7");
    assert_eq!(eval_line("(coin-p (make-coin 7))", &e), "T");
    assert_eq!(eval_line("(coin-p (make-chest 1 (list)))", &e), "()");
}

#[test]
fn compile_eligibility_is_computed_from_field_types() {
    let e = env();
    // Scalar-only -> compile-eligible (native pass could store it, #297 s3).
    assert_eq!(eval_line("(record-compile-eligible-p 'coin)", &e), "T");
    // A list field is not natively storable today -> not eligible.
    assert_eq!(eval_line("(record-compile-eligible-p 'chest)", &e), "()");
    // An (array int64) field IS natively storable -> eligible.
    eval_line("(defrecord grid (cells (array int64)) (n int64))", &e);
    assert_eq!(eval_line("(record-compile-eligible-p 'grid)", &e), "T");
}

#[test]
fn cross_kind_misuse_is_a_static_row_error() {
    let e = env();
    // A coin {value} has no items field; asking for it is caught by the
    // checker, and (unlike #305's struct case) also correctly at runtime.
    let out = eval_line("(check-type (chest-items (make-coin 5)))", &e);
    assert!(
        out.contains("type error") && out.contains("items"),
        "expected a closed-record row error naming items, got: {out}"
    );
}

#[test]
fn records_are_distinguishable_by_brand() {
    let e = env();
    // Two records with the same field shape stay distinct by brand (the
    // predicate discriminates), while both flow through a shared row.
    eval_line("(defrecord alpha (value int64))", &e);
    eval_line("(defrecord beta (value int64))", &e);
    assert_eq!(eval_line("(alpha-p (make-beta 1))", &e), "()");
    assert_eq!(eval_line("(worth (make-alpha 3))", &e), "3");
    assert_eq!(eval_line("(worth (make-beta 4))", &e), "4");
}
