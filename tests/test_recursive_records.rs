//! Recursive record types: self- and mutually-referential defrecord field
//! types stay NOMINAL (no silent degrade to any). Struct unification is by
//! brand name, struct-into-row expansion re-resolves through the registry
//! snapshot, and forward references get provisional defs, so recursion works
//! at every depth. Terminators use Option — the blessed idiom.

mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

#[test]
fn self_reference_is_nominal_not_any() {
    let e = env_with_stdlib();
    eval_line("(defrecord node (val int64) (next node))", &e);
    assert_eq!(
        eval_line("(see-type 'node-next)", &e),
        "(DECLARED (-> (NODE) NODE))"
    );
    assert_eq!(
        eval_line("(see-type 'make-node)", &e),
        "(DECLARED (-> (INT64 NODE) NODE))"
    );
}

#[test]
fn deep_access_works_and_stays_checked() {
    let e = env_with_stdlib();
    eval_line("(defrecord node (val int64) (next node))", &e);
    eval_line(
        "(def chain (make-node 1 (make-node 2 (make-node 3 'end))))",
        &e,
    );
    assert_eq!(
        eval_line("(node-val (node-next (node-next chain)))", &e),
        "3"
    );
    // Nominal at depth: the recursive read's type unifies with the accessor.
    assert_eq!(
        eval_line("(check-type (node-val (node-next chain)))", &e),
        "\"int64\""
    );
    // Row access through a recursive read (registry re-resolution).
    assert_eq!(
        eval_line("(check-type (record-ref (node-next chain) 'val))", &e),
        "\"int64\""
    );
}

#[test]
fn mutual_recursion_via_forward_reference() {
    let e = env_with_stdlib();
    // tree references branch before branch exists: a provisional def is
    // registered, and the later declaration completes it.
    eval_line("(defrecord tree (left branch) (v int64))", &e);
    eval_line("(defrecord branch (t1 tree))", &e);
    assert_eq!(
        eval_line("(see-type 'tree-left)", &e),
        "(DECLARED (-> (TREE) BRANCH))"
    );
    assert_eq!(
        eval_line("(see-type 'branch-t1)", &e),
        "(DECLARED (-> (BRANCH) TREE))"
    );
}

#[test]
fn option_terminated_recursion_is_fully_checked() {
    let e = env_with_stdlib();
    eval_line("(defrecord node (val int64) (next option))", &e);
    eval_line(
        "(defun sum-nodes (n)
           (+ (node-val n)
              (variant-case (node-next n)
                (some (rest) (sum-nodes rest))
                (none () 0))))",
        &e,
    );
    assert_eq!(
        eval_line(
            "(sum-nodes (make-node 1 (some (make-node 2 (some (make-node 3 (none)))))))",
            &e
        ),
        "6"
    );
    // Checked construction: (none) is an OPTION; a foreign brand is not.
    assert_eq!(
        eval_line("(check-type (make-node 1 (none)))", &e),
        "\"NODE\""
    );
    eval_line("(defvariant color (red) (blue))", &e);
    let out = eval_line("(check-type (make-node 1 (red)))", &e);
    assert!(
        out.contains("RED is not a constructor of variant OPTION"),
        "got: {out}"
    );
}

#[test]
fn typo_field_types_surface_as_phantom_brands_not_silent_any() {
    let e = env_with_stdlib();
    // A misspelled type is a forward reference: honest nominal errors at the
    // first unification instead of a silent any.
    eval_line("(defrecord pt (x intt64) (y int64))", &e);
    assert_eq!(
        eval_line("(see-type 'pt-x)", &e),
        "(DECLARED (-> (PT) INTT64))"
    );
    let out = eval_line("(check-type (+ 1 (pt-x (make-pt 1 2))))", &e);
    assert!(out.contains("INTT64"), "got: {out}");
}
