//! DEFRECORD — the one-door gradual-typed record (lib/20-condensation.lisp,
//! issue #308). One form defines a BRANDED type (nominal in the checker,
//! denotable in signatures, row-subsumable per #299) over ONE runtime
//! representation (StructObj), with the tier — compiled or dynamic — chosen
//! from the field types. The property that binds it together: any defrecord
//! value flows through any row-polymorphic function naming a subset of its
//! fields — at the checker AND at runtime.

mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

fn env() -> lamedh::Shared<lamedh::environment::Environment> {
    let e = env_with_stdlib();
    eval_line("(defrecord coin (value int64))", &e);
    eval_line("(defrecord chest (value int64) (items (list string)))", &e);
    // No axioms anywhere: record-ref's checker rule derives the row, and the
    // derived-scheme path (#308) carries it through the helper into worth.
    eval_line("(defun the-value (self) (record-ref self 'value))", &e);
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
    // Derived end-to-end through the helper — and MORE general than the old
    // hand-declared axiom (the field type is polymorphic too).
    assert_eq!(
        eval_line("(see-type 'worth)", &e),
        "(CHECKED (FORALL (A B) (-> ((RECORD ((VALUE A)) B)) A)))"
    );
}

#[test]
fn compound_fields_get_row_schemes_automatically() {
    let e = env();
    // A (list string) field keeps chest off the compiled tier but its
    // accessor still carries a BRANDED declared scheme (generated in
    // lockstep with the definition). Row-generic access is record-ref.
    assert_eq!(
        eval_line("(see-type 'chest-items)", &e),
        "(DECLARED (-> (CHEST) (LIST STRING)))"
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
    // The named accessor is NOMINAL (#308): a coin is not a chest, whatever
    // its shape. (Shape-generic access via record-ref stays available.)
    let out = eval_line("(check-type (chest-items (make-coin 5)))", &e);
    assert!(
        out.contains("type error") && out.contains("COIN") && out.contains("CHEST"),
        "expected a nominal brand error, got: {out}"
    );
    // And the row error still exists where a row is demanded: a coin has no
    // items field for record-ref.
    let out = eval_line("(check-type (record-ref (make-coin 5) 'items))", &e);
    assert!(
        out.contains("type error") && out.contains("items"),
        "expected a row error naming items, got: {out}"
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

#[test]
fn records_print_readably_and_round_trip() {
    let e = env();
    // #308 stage D: one readable form for every record, both tiers.
    assert_eq!(
        eval_line("(make-chest 9 (list \"gold\" \"gem\"))", &e),
        "#S(CHEST 9 (\"gold\" \"gem\"))"
    );
    assert_eq!(eval_line("(make-coin 5)", &e), "#S(COIN 5)");
    // The printed form reads back to an EQUAL value (print/read round-trip —
    // the spawn/channel serialization contract).
    assert_eq!(
        eval_line(
            "(let ((c (make-chest 9 (list \"gold\")))) (equal c (read-from-string (prin1-to-string c))))",
            &e
        ),
        "T"
    );
    // And a #S literal is directly usable source syntax.
    assert_eq!(eval_line("(chest-value #S(CHEST 7 (\"x\")))", &e), "7");
}

#[test]
#[cfg(feature = "concurrency")]
fn records_cross_the_spawn_boundary() {
    let e = env();
    // A live record value spliced into a spawned body serializes as #S(...),
    // reads back in the share-nothing child, and is accessed through the
    // child's own declaration (brands are declarations: ship the schema).
    assert_eq!(
        eval_line(
            "(let ((c (make-chest 42 (list \"gold\"))))
               (await (spawn* ()
                 (list 'progn
                   '(defrecord chest (value int64) (items (list string)))
                   (list 'chest-value c)))))",
            &e
        ),
        "42"
    );
}
