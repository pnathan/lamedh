//! HASH-CODE: a kernel identity/value hash primitive (issue #474).
//!
//! `KERNEL.md` Part XI previously had no primitive letting Lamedh code
//! derive a stable, distinct hash value for host-opaque or
//! identity-compared values (hash tables, arrays, environments, closures,
//! ...). `(hash-code v)` fills that gap by exposing the reference's own
//! `Hash for LispVal` (`src/lib.rs`) — already required to agree with the
//! `PartialEq for LispVal` relation `EQUAL` and hash-table keys use — as a
//! fixnum. This suite pins down: `EQUAL a b` implies `(hash-code a) =
//! (hash-code b)`, and that distinct allocations of an identity-compared
//! type get (with overwhelming probability) distinct codes.

use lamedh::{self, Shared, environment::Environment, eval_line};

fn env_with_stdlib() -> Shared<Environment> {
    Environment::with_stdlib()
}

#[test]
fn hash_code_returns_a_fixnum() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(fixp (hash-code 5))", &env), "T");
    assert_eq!(eval_line("(fixp (hash-code \"abc\"))", &env), "T");
    assert_eq!(eval_line("(fixp (hash-code (make-hash-table)))", &env), "T");
}

#[test]
fn hash_code_consistent_with_equal_for_atoms() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("(= (hash-code 100000) (hash-code 100000))", &env),
        "T"
    );
    assert_eq!(
        eval_line(
            "(= (hash-code \"ab\") (hash-code (concat \"a\" \"b\")))",
            &env
        ),
        "T"
    );
    assert_eq!(
        eval_line("(= (hash-code 'foo) (hash-code 'foo))", &env),
        "T"
    );
    // 0.0 and -0.0 are EQUAL (and EQ), so they must hash the same.
    assert_eq!(eval_line("(= (hash-code 0.0) (hash-code -0.0))", &env), "T");
    // NaN is EQ/EQUAL to NaN in this Lisp (Part IV carve-out).
    assert_eq!(
        eval_line("(= (hash-code (/ 0.0 0.0)) (hash-code (/ 0.0 0.0)))", &env),
        "T"
    );
}

#[test]
fn hash_code_consistent_with_equal_for_conses() {
    let env = env_with_stdlib();
    // Two separately allocated but structurally identical (EQUAL, not EQ)
    // lists must still hash the same.
    assert_eq!(
        eval_line(
            "(= (hash-code (list 1 2 3)) (hash-code (list 1 2 3)))",
            &env
        ),
        "T"
    );
    assert_eq!(
        eval_line(
            "(= (hash-code (cons 1 (cons 2 nil))) (hash-code (list 1 2)))",
            &env
        ),
        "T"
    );
}

#[test]
fn hash_code_distinguishes_distinct_identity_typed_values() {
    let env = env_with_stdlib();
    // Distinct arrays/hash tables/environments are never EQUAL, and (with
    // overwhelming probability for a 64-bit hasher over distinct pointers)
    // must not collide either -- this is the whole point of the primitive:
    // a portable Lisp-level hash function can now spread these instead of
    // bucketing every one of them together.
    assert_eq!(
        eval_line(
            "(let ((h1 (make-hash-table)) (h2 (make-hash-table))) (= (hash-code h1) (hash-code h2)))",
            &env
        ),
        "()"
    );
    // Both arrays are bound and kept alive for the duration of the
    // comparison, so a freed-and-reused allocation address cannot make two
    // genuinely distinct, live arrays collide.
    assert_eq!(
        eval_line(
            "(let ((a (array 3)) (b (array 3))) (= (hash-code a) (hash-code b)))",
            &env
        ),
        "()"
    );
    assert_eq!(
        eval_line(
            "(let ((e (make-environment))) (= (hash-code (the-environment)) (hash-code e)))",
            &env
        ),
        "()"
    );
    // But the SAME allocation must hash to the same code every time it is
    // asked (identity is stable for the object's lifetime).
    assert_eq!(
        eval_line(
            "(let ((h (make-hash-table))) (= (hash-code h) (hash-code h)))",
            &env
        ),
        "T"
    );
}

#[test]
fn hash_code_distinguishes_closures_by_captured_environment() {
    let env = env_with_stdlib();
    // Two lambdas built from the same text in the same environment are EQ
    // (Part IV) and must hash the same.
    assert_eq!(
        eval_line(
            "(let ((f (lambda (x) x)) (g (lambda (x) x))) (= (hash-code f) (hash-code g)))",
            &env
        ),
        "T"
    );
    // Two lambdas built in different call frames (different captured
    // environments) are not EQ and, since closures were previously
    // unhashable-by-value (issue #474), must now actually get distinct
    // codes rather than colliding. Both closures are bound and kept alive
    // for the comparison so a freed-and-reused environment allocation
    // cannot make two genuinely distinct, live closures collide.
    eval_line("(defun make-adder (n) (lambda (x) (+ x n)))", &env);
    assert_eq!(
        eval_line(
            "(let ((f (make-adder 1)) (g (make-adder 2))) (= (hash-code f) (hash-code g)))",
            &env
        ),
        "()"
    );
}

#[test]
fn hash_code_requires_exactly_one_argument() {
    let env = env_with_stdlib();
    let zero_args = eval_line("(hash-code)", &env);
    assert!(
        zero_args.contains("Error"),
        "expected error for (hash-code), got: {zero_args}"
    );
    let two_args = eval_line("(hash-code 1 2)", &env);
    assert!(
        two_args.contains("Error"),
        "expected error for (hash-code 1 2), got: {two_args}"
    );
}
