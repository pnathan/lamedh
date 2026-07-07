//! Integration tests for lib/23-match.lisp: the structural pattern language
//! (PAT-MATCH core; MATCH / DESTRUCTURING-BIND / SGREP / REWRITE surfaces),
//! plus the reader amendment that admits `?`-leading symbols and `_` as a
//! symbol constituent.

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line, with_large_stack};

fn env() -> Shared<Environment> {
    Environment::with_stdlib()
}

// ------------------------------------------------------------- reader ----

#[test]
fn reader_admits_pattern_symbols() {
    let e = env();
    assert_eq!(eval_line("'?x", &e), "?X");
    assert_eq!(eval_line("'??xs", &e), "??XS");
    assert_eq!(eval_line("'?_", &e), "?_");
    assert_eq!(eval_line("'some_name", &e), "SOME_NAME");
    // The `?`-suffix predicate naming convention is untouched.
    assert_eq!(eval_line("'null?", &e), "NULL?");
}

// ---------------------------------------------------------- pat-match ----

#[test]
fn pat_match_binds_and_fails() {
    let e = env();
    assert_eq!(
        eval_line("(pat-match '(?x ?y) '(1 2))", &e),
        "((?Y . 2) (?X . 1))"
    );
    // Repeated variable must be EQUAL (unification-lite).
    assert_eq!(
        eval_line("(match-fail-p (pat-match '(?x ?x) '(1 2)))", &e),
        "T"
    );
    assert_eq!(
        eval_line("(match-fail-p (pat-match '(?x ?x) '(1 1)))", &e),
        "()"
    );
    // A match that binds nothing is NIL, not failure.
    assert_eq!(eval_line("(pat-match '(a b) '(a b))", &e), "()");
    assert_eq!(
        eval_line("(match-fail-p (pat-match '(a b) '(a c)))", &e),
        "T"
    );
}

#[test]
fn pat_match_segments_backtrack() {
    let e = env();
    assert_eq!(
        eval_line("(pat-match '(a ??mid z) '(a b c d z))", &e),
        "((??MID B C D))"
    );
    // Two segments around a literal anchor require backtracking.
    assert_eq!(
        eval_line("(pat-match '(??x c ??y) '(a b c d e))", &e),
        "((??Y D E) (??X A B))"
    );
    // Empty segments are allowed.
    assert_eq!(eval_line("(pat-match '(??x a) '(a))", &e), "((??X))");
}

#[test]
fn pat_match_operators() {
    let e = env();
    assert_eq!(
        eval_line("(pat-match '(?is ?n numberp) 42)", &e),
        "((?N . 42))"
    );
    assert_eq!(
        eval_line("(match-fail-p (pat-match '(?is ?n numberp) 'sym))", &e),
        "T"
    );
    assert_eq!(
        eval_line("(pat-match '(?and ?x (?is ?_ numberp)) 7)", &e),
        "((?X . 7))"
    );
    assert_eq!(eval_line("(pat-match '(?or 1 2 ?x) 3)", &e), "((?X . 3))");
    assert_eq!(
        eval_line("(pat-match '(?not (?is ?_ numberp)) 'sym)", &e),
        "()"
    );
    // Quote escape: match the literal symbol ?X.
    assert_eq!(eval_line("(pat-match ''?x '?x)", &e), "()");
    assert_eq!(eval_line("(match-fail-p (pat-match ''?x 'other))", &e), "T");
}

#[test]
fn pat_match_dotted_patterns() {
    let e = env();
    assert_eq!(
        eval_line("(pat-match '(?h . ?t) '(1 2 3))", &e),
        "((?T 2 3) (?H . 1))"
    );
    assert_eq!(
        eval_line("(pat-match '(?a . ?b) '(1 . 2))", &e),
        "((?B . 2) (?A . 1))"
    );
}

// -------------------------------------------------------------- match ----

#[test]
fn match_clauses_guards_and_default() {
    let e = env();
    assert_eq!(
        eval_line(
            "(match '(add 1 2)
               ((add ?a ?b) :when (> ?b ?a) 'ascending)
               ((add ?a ?b) 'other))",
            &e
        ),
        "ASCENDING"
    );
    assert_eq!(
        eval_line(
            "(match '(add 2 1)
               ((add ?a ?b) :when (> ?b ?a) 'ascending)
               ((add ?a ?b) 'other))",
            &e
        ),
        "OTHER"
    );
    assert_eq!(
        eval_line("(match 99 ((?is ?n numberp) (* ?n 2)) (?_ 'default))", &e),
        "198"
    );
    assert_eq!(
        eval_line("(match 'zzz ((?is ?n numberp) ?n) (?_ 'default))", &e),
        "DEFAULT"
    );
    // No matching clause: NIL.
    assert_eq!(eval_line("(match 1 (2 'two))", &e), "()");
}

#[test]
fn match_binds_segments_in_body() {
    let e = env();
    assert_eq!(
        eval_line("(match '(1 2 3 4) ((?x ??rest) (list ?x ??rest)))", &e),
        "(1 (2 3 4))"
    );
}

#[test]
fn destructuring_bind_success_and_error() {
    let e = env();
    assert_eq!(
        eval_line(
            "(destructuring-bind (?a (?b . ?c)) '(1 (2 3 4)) (list ?a ?b ?c))",
            &e
        ),
        "(1 2 (3 4))"
    );
    let out = eval_line(
        "(handler-case (destructuring-bind (?a ?b) '(1) ?a) (error (er) 'no-match))",
        &e,
    );
    assert_eq!(out, "NO-MATCH");
}

// -------------------------------------------------------------- sgrep ----

#[test]
fn sgrep_finds_all_matching_subforms() {
    let e = env();
    let out = eval_line(
        "(length (sgrep '(setq ?v ?e) '(progn (setq a 1) (if x (setq b (setq c 2))))))",
        &e,
    );
    assert_eq!(out, "3", "must find nested setqs too");
    // Bindings ride along with each hit.
    let out = eval_line(
        "(cdr (assoc '?v (cdr (car (sgrep '(setq ?v ?e) '(setq a 1))))))",
        &e,
    );
    assert_eq!(out, "A");
}

#[test]
fn sgrep_fn_searches_function_source() {
    let e = env();
    let out = eval_line("(length (sgrep-fn '(car ?x) 'cadr))", &e);
    assert_eq!(out, "1", "cadr is (car (cdr x)) — one car call");
}

// ------------------------------------------------------------ rewrite ----

#[test]
fn rewrite_is_bottom_up_and_loop_safe() {
    let e = env();
    // Innermost-first: nested matches inside a binding are transformed.
    assert_eq!(
        eval_line(
            "(rewrite '(plus ?a ?b) '(+ ?a ?b) '(f (plus 1 (plus 2 3))))",
            &e
        ),
        "(F (+ 1 (+ 2 3)))"
    );
    // Identity template must not loop.
    assert_eq!(
        eval_line("(rewrite '(?f ??args) '(?f ??args) '(a (b c)))", &e),
        "(A (B C))"
    );
    // Atoms and dotted tails are rewritten too.
    assert_eq!(eval_line("(rewrite 'x 'y '(x (x . x)))", &e), "(Y (Y . Y))");
}

#[test]
fn instantiate_splices_segments() {
    let e = env();
    assert_eq!(
        eval_line(
            "(instantiate '(?f ??args done) '((?f . call) (??args 1 2 3)))",
            &e
        ),
        "(CALL 1 2 3 DONE)"
    );
}

// -------------------------------------------------------- composition ----

#[test]
fn match_composes_with_guard_fences() {
    with_large_stack(|| {
        let e = env();
        // Pattern matching inside a fuel fence: the matcher itself is
        // stdlib Lisp defined outside the fence, charged per call.
        let out = eval_line(
            "(with-fuel 100000
               (match '(inc 41) ((inc ?n) (+ ?n 1)) (?_ 'nope)))",
            &e,
        );
        assert_eq!(out, "42");
    });
}

#[test]
fn rewrite_as_code_transform_end_to_end() {
    let e = env();
    // The homoiconic loop: rewrite code, then run it.
    let out = eval_line(
        "(eval (rewrite '(double ?x) '(* 2 ?x) '(+ (double 10) (double 11))))",
        &e,
    );
    assert_eq!(out, "42");
}
