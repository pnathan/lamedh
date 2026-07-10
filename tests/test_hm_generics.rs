//! HM generics (0.3, #321): parametric records and variants as proper type
//! application — nominal by name, arguments unified pairwise, constructor
//! applications absorbing into their variant's application, instantiation
//! at every use, erased at runtime (one StructObj representation).

mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

fn env() -> lamedh::Shared<lamedh::environment::Environment> {
    let e = env_with_stdlib();
    eval_line("(defrecord (duo a b) (first a) (second b))", &e);
    e
}

#[test]
fn parametric_records_declare_polymorphic_schemes() {
    let e = env();
    assert_eq!(
        eval_line("(see-type 'make-duo)", &e),
        "(DECLARED (FORALL (A B) (-> (A B) (DUO A B))))"
    );
    assert_eq!(
        eval_line("(see-type 'duo-first)", &e),
        "(DECLARED (FORALL (A B) (-> ((DUO A B)) A)))"
    );
    // Runtime is the ordinary erased record.
    assert_eq!(eval_line("(make-duo 1 \"s\")", &e), "#S(DUO 1 \"s\")");
    assert_eq!(eval_line("(duo-first (make-duo 1 \"s\"))", &e), "1");
}

#[test]
fn instantiation_is_precise_per_use() {
    let e = env();
    assert_eq!(
        eval_line("(check-type (+ 1 (duo-first (make-duo 1 \"x\"))))", &e),
        "\"int64\""
    );
    let out = eval_line("(check-type (+ 1 (duo-first (make-duo \"x\" 2))))", &e);
    assert!(out.contains("type error"), "got: {out}");
}

#[test]
fn option_and_result_are_parametric() {
    let e = env();
    assert_eq!(
        eval_line("(see-type 'some)", &e),
        "(DECLARED (FORALL (A) (-> (A) (SOME A))))"
    );
    assert_eq!(
        eval_line("(check-type (+ 1 (unwrap-or (some 5) 0)))", &e),
        "\"int64\""
    );
    // Payload and default must agree.
    let out = eval_line("(check-type (unwrap-or (some \"s\") 0))", &e);
    assert!(out.contains("type error"), "got: {out}");
    // (none) instantiates freshly, like nil.
    assert_eq!(
        eval_line("(check-type (+ 1 (unwrap-or (none) 7)))", &e),
        "\"int64\""
    );
    assert_eq!(
        eval_line("(check-type (+ 1 (result-or (ok 2) 0)))", &e),
        "\"int64\""
    );
}

#[test]
fn cross_nominal_applications_are_rejected() {
    let e = env();
    let out = eval_line("(check-type (unwrap-or (make-duo 1 2) 0))", &e);
    assert!(out.contains("cannot unify (duo"), "got: {out}");
}

#[test]
fn recursive_generic_records_through_parametric_variants() {
    let e = env();
    eval_line("(defrecord (node a) (val a) (next (option (node a))))", &e);
    assert_eq!(
        eval_line("(see-type 'node-next)", &e),
        "(DECLARED (FORALL (A) (-> ((NODE A)) (OPTION (NODE A)))))"
    );
    eval_line(
        "(defun sum-nodes (n)
           (+ (node-val n)
              (variant-case (node-next n)
                (some (r) (sum-nodes r))
                (none () 0))))",
        &e,
    );
    assert_eq!(
        eval_line("(sum-nodes (make-node 1 (some (make-node 2 (none)))))", &e),
        "3"
    );
    assert_eq!(
        eval_line("(check-type (make-node 1 (some (make-node 2 (none)))))", &e),
        "\"(node int64)\""
    );
    let out = eval_line("(check-type (make-node 1 (some (make-duo 1 2))))", &e);
    assert!(out.contains("cannot unify (duo"), "got: {out}");
}

#[test]
fn bare_generic_names_mean_all_any_application() {
    let e = env();
    // Pre-parametric spelling still works, gradually: `option` = (option any).
    eval_line("(defrecord cell (v int64) (link option))", &e);
    assert_eq!(
        eval_line("(see-type 'cell-link)", &e),
        "(DECLARED (-> (CELL) (OPTION ANY)))"
    );
    assert_eq!(
        eval_line("(cell-v (make-cell 1 (some (make-cell 2 (none)))))", &e),
        "1"
    );
}

#[test]
fn generic_applications_row_subsume() {
    let e = env();
    // A (pair int64 string) flows through a row-typed reader with the
    // instantiated field type.
    eval_line("(defun the-first (x) (record-ref x 'first))", &e);
    assert_eq!(
        eval_line("(check-type (+ 1 (the-first (make-duo 4 \"s\"))))", &e),
        "\"int64\""
    );
}

#[test]
fn sibling_constructors_join_at_their_variant() {
    let e = env_with_stdlib();
    // option-of's IF builds (none) in one branch and (some x) in the other:
    // sibling constructor applications unify (meeting at the variant).
    assert_eq!(
        eval_line("(check-type (if t (some 5) (none)))", &e),
        "\"(some int64)\""
    );
    assert_eq!(
        eval_line(
            "(check-type (+ 1 (unwrap-or (if nil (some 5) (none)) 0)))",
            &e
        ),
        "\"int64\""
    );
}

#[test]
fn reserved_type_names_are_rejected() {
    let e = env_with_stdlib();
    let out = eval_line("(defrecord (pair a b) (first a) (second b))", &e);
    assert!(out.contains("built-in type name"), "got: {out}");
    let out = eval_line("(defvariant (list a) (lnil) (lcons (h a)))", &e);
    assert!(out.contains("built-in type name"), "got: {out}");
}
