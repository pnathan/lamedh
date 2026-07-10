//! The type table (lib/28-types.lisp): verified declared schemes for
//! builtins and stdlib functions — "typing with vigor". Plus census batch 2
//! (one hash-removal name, LENGTH over every sized collection).

mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

#[test]
fn schemes_flow_through_user_code() {
    let e = env_with_stdlib();
    // String pipeline: fully derived from the table, no annotations.
    eval_line("(defun shout (s) (string-upcase (concat s \"!\")))", &e);
    assert_eq!(
        eval_line("(see-type 'shout)", &e),
        "(CHECKED (-> (STRING) STRING))"
    );
    let out = eval_line("(check-type (shout 5))", &e);
    assert!(out.contains("cannot unify int64 with string"), "got: {out}");
    // Predicates clear the vacuous frontier.
    eval_line("(defun all-even (xs) (every #'evenp xs))", &e);
    assert_eq!(
        eval_line("(see-type 'all-even)", &e),
        "(CHECKED (FORALL (A) (-> ((LIST A)) BOOL)))"
    );
    // Math results are known even where arguments stay gradual.
    assert_eq!(eval_line("(check-type (+ 1 (floor 3.7)))", &e), "\"int64\"");
    assert_eq!(eval_line("(check-type (sqrt 4))", &e), "\"float64\"");
    // Conversions.
    assert_eq!(
        eval_line("(check-type (string-length (princ-to-string 42)))", &e),
        "\"int64\""
    );
}

#[test]
fn axioms_match_the_evaluator() {
    let e = env_with_stdlib();
    // member's miss value IS a list — total, so it carries a full scheme.
    assert_eq!(eval_line("(member 9 (list 1 2))", &e), "()");
    assert_eq!(
        eval_line("(check-type (member 1 (list 1 2)))", &e),
        "\"(list int64)\""
    );
    // Integer-only predicates reject floats in BOTH worlds.
    let ev = eval_line("(zerop 0.0)", &e);
    assert!(ev.contains("Error"), "got: {ev}");
    let ck = eval_line("(check-type (zerop 0.0))", &e);
    assert!(ck.contains("type error"), "got: {ck}");
    // nil-on-miss functions deliberately have NO declared result type:
    // nth out of range returns () and stays gradual.
    assert_eq!(eval_line("(nth 9 (list 1 2))", &e), "()");
    assert_eq!(eval_line("(check-type (nth 0 (list 1 2)))", &e), "\"any\"");
}

#[test]
fn census_batch2_hash_surface() {
    let e = env_with_stdlib();
    // LENGTH covers every sized collection now.
    assert_eq!(
        eval_line(
            "(let ((h (make-hash-table))) (sethash h 'a 1) (sethash h 'b 2) (length h))",
            &e
        ),
        "2"
    );
    // remhash is the one removal name; delete-key is gone.
    assert_eq!(
        eval_line(
            "(let ((h (make-hash-table))) (sethash h 'a 1) (remhash h 'a) (length h))",
            &e
        ),
        "0"
    );
    let out = eval_line("(delete-key (make-hash-table) 'x)", &e);
    assert!(out.contains("Unbound"), "got: {out}");
}
