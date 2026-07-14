//! Regression coverage for issue #361: several Prelude list/string builders
//! recursed non-tail-recursively, so `(cons x (recurse ...))` (or the
//! string-building equivalent via CONCAT) grew one native/eval frame per
//! input element. On inputs of only a few thousand elements/characters they
//! hit the ~10000 eval-frame recursion limit — an ordinary, non-adversarial
//! input, not a runaway/infinite recursion.
//!
//! The fix rewrites each offender as a tail-recursive accumulator (+ REVERSE
//! at the end, or direct accumulation for the string builders), so the
//! evaluator's TCO keeps native stack usage O(1) regardless of input size.
//!
//! Every test below runs on the DEFAULT test-thread stack (no
//! `with_large_stack`) — that is the point: these functions must not need an
//! inflated stack to handle ordinary-sized input. Sizes are chosen to sit
//! safely past the old ~10000-frame failure point while keeping wall-clock
//! time reasonable (a few of these functions are inherently O(n^2) in their
//! *time* complexity — unchanged from before this fix — so their test inputs
//! are shaped to avoid also timing out).

mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

// ---- past the old failure size: must not error, must produce the right length ----

#[test]
fn string_to_list_past_frame_limit() {
    let e = env_with_stdlib();
    eval_line("(defvar big-str (make-string 12000 'x'))", &e);
    assert_eq!(eval_line("(length (string->list big-str))", &e), "12000");
}

#[test]
fn filter_past_frame_limit() {
    let e = env_with_stdlib();
    assert_eq!(
        eval_line("(length (filter #'numberp (iota 12000)))", &e),
        "12000"
    );
}

#[test]
fn take_past_frame_limit() {
    let e = env_with_stdlib();
    assert_eq!(eval_line("(length (take (iota 20000) 12000))", &e), "12000");
}

#[test]
fn take_while_past_frame_limit() {
    let e = env_with_stdlib();
    assert_eq!(
        eval_line("(length (take-while #'numberp (iota 12000)))", &e),
        "12000"
    );
}

#[test]
fn butlast_past_frame_limit() {
    let e = env_with_stdlib();
    assert_eq!(eval_line("(length (butlast (iota 12000)))", &e), "11999");
}

#[test]
fn zip_past_frame_limit() {
    let e = env_with_stdlib();
    assert_eq!(
        eval_line("(length (zip (iota 12000) (iota 12000)))", &e),
        "12000"
    );
}

#[test]
fn pairlis_past_frame_limit() {
    let e = env_with_stdlib();
    assert_eq!(
        eval_line("(length (pairlis (iota 12000) (iota 12000)))", &e),
        "12000"
    );
}

#[test]
fn copy_past_frame_limit() {
    let e = env_with_stdlib();
    assert_eq!(eval_line("(length (copy (iota 12000)))", &e), "12000");
}

#[test]
fn copy_list_star_past_frame_limit() {
    let e = env_with_stdlib();
    // COPY dispatches to COPY-LIST* for ordinary lists via the COPY protocol
    // (lib/29-protocols.lisp); COPY-LIST* is the fix that actually matters
    // for `(copy <big list>)` in practice.
    assert_eq!(eval_line("(length (copy-list* (iota 12000)))", &e), "12000");
}

#[test]
fn string_repeat_past_frame_limit() {
    let e = env_with_stdlib();
    assert_eq!(
        eval_line("(string-length* (string-repeat \"ab\" 12000))", &e),
        "24000"
    );
}

#[test]
fn make_string_past_frame_limit() {
    let e = env_with_stdlib();
    // MAKE-STRING inherits its fix from STRING-REPEAT.
    assert_eq!(
        eval_line("(string-length* (make-string 12000))", &e),
        "12000"
    );
}

#[test]
fn string_join_past_frame_limit() {
    let e = env_with_stdlib();
    eval_line("(defvar parts (mapcar #'princ-to-string (iota 8000)))", &e);
    assert_eq!(
        eval_line(
            "(length (string-split (string-join parts \",\") \",\"))",
            &e
        ),
        "8000"
    );
}

#[test]
fn string_capitalize_past_frame_limit() {
    let e = env_with_stdlib();
    // 12000 characters via 2000 repeats of a 6-char word/space pattern.
    assert_eq!(
        eval_line(
            "(string-length* (string-capitalize (string-repeat \"ab cd \" 2000)))",
            &e
        ),
        "12000"
    );
}

#[test]
fn string_split_past_frame_limit() {
    let e = env_with_stdlib();
    // STRING-SPLIT's per-call cost is proportional to the remaining string,
    // so single-character items keep this well under a few seconds while
    // still exercising >10000 levels of what used to be non-tail recursion
    // (the unfixed version reliably raises "recursion limit exceeded" by
    // 12000 single-character items; see PR body for the measurement).
    eval_line(
        "(defvar split-src (string-join (mapcar (lambda (i) \"a\") (iota 11000)) \",\"))",
        &e,
    );
    assert_eq!(
        eval_line("(length (string-split split-src \",\"))", &e),
        "11000"
    );
}

#[test]
fn remove_duplicates_past_frame_limit() {
    let e = env_with_stdlib();
    // REMOVE-DUPLICATES is O(n^2) in the number of *distinct* elements
    // (MEMBER + REMOVE-ALL per survivor) — unchanged by this fix, and that
    // was already true before it. A 12000-element list cycling through 5
    // distinct values exercises 12000 levels of what used to be non-tail
    // recursion while keeping the O(n^2) part bounded by the 5 distinct
    // values, not the 12000 total elements.
    eval_line(
        "(defvar cyc (mapcar (lambda (i) (mod i 5)) (iota 12000)))",
        &e,
    );
    assert_eq!(eval_line("(remove-duplicates cyc)", &e), "(0 1 2 3 4)");
}

// ---- exact-equality spot checks: unchanged semantics on small inputs ----

#[test]
fn pairlis_semantics_unchanged() {
    let e = env_with_stdlib();
    assert_eq!(
        eval_line("(pairlis '(a b c) '(1 2 3))", &e),
        "((A . 1) (B . 2) (C . 3))"
    );
    assert_eq!(eval_line("(pairlis nil nil)", &e), "()");
    assert_eq!(eval_line("(pairlis '(a) nil)", &e), "()");
}

#[test]
fn copy_semantics_unchanged() {
    let e = env_with_stdlib();
    assert_eq!(eval_line("(copy '(1 2 . 3))", &e), "(1 2 . 3)");
    assert_eq!(eval_line("(copy 5)", &e), "5");
    assert_eq!(eval_line("(copy nil)", &e), "()");
    assert_eq!(eval_line("(equal (copy '(1 2 3)) '(1 2 3))", &e), "T");
}

#[test]
fn copy_list_star_semantics_unchanged() {
    let e = env_with_stdlib();
    assert_eq!(eval_line("(copy-list* '(1 2 3 . 4))", &e), "(1 2 3 . 4)");
    assert_eq!(eval_line("(copy-list* nil)", &e), "()");
    assert_eq!(eval_line("(equal (copy-list* '(1 2 3)) '(1 2 3))", &e), "T");
}

#[test]
fn filter_semantics_unchanged() {
    let e = env_with_stdlib();
    assert_eq!(eval_line("(filter #'evenp '(1 2 3 4 5 6))", &e), "(2 4 6)");
    assert_eq!(eval_line("(filter #'evenp nil)", &e), "()");
    // The LIST protocol instance still routes through the fixed fallback.
    assert_eq!(
        eval_line("(filter (lambda (x) (> x 2)) '(1 2 3 4))", &e),
        "(3 4)"
    );
}

#[test]
fn take_semantics_unchanged() {
    let e = env_with_stdlib();
    assert_eq!(eval_line("(take '(1 2 3 4 5) 3)", &e), "(1 2 3)");
    assert_eq!(eval_line("(take '(1 2 3) 0)", &e), "()");
    assert_eq!(eval_line("(take nil 5)", &e), "()");
    assert_eq!(eval_line("(take '(1 2) 10)", &e), "(1 2)");
}

#[test]
fn take_while_semantics_unchanged() {
    let e = env_with_stdlib();
    assert_eq!(
        eval_line("(take-while #'evenp '(2 4 6 1 8))", &e),
        "(2 4 6)"
    );
    assert_eq!(eval_line("(take-while #'evenp '(1 2 3))", &e), "()");
    assert_eq!(eval_line("(take-while #'evenp nil)", &e), "()");
}

#[test]
fn butlast_semantics_unchanged() {
    let e = env_with_stdlib();
    assert_eq!(eval_line("(butlast '(1 2 3 4))", &e), "(1 2 3)");
    assert_eq!(eval_line("(butlast nil)", &e), "()");
    assert_eq!(eval_line("(butlast (list 1))", &e), "()");
}

#[test]
fn zip_semantics_unchanged() {
    let e = env_with_stdlib();
    assert_eq!(eval_line("(zip '(1 2 3) '(a b))", &e), "((1 A) (2 B))");
    assert_eq!(eval_line("(zip nil '(1 2))", &e), "()");
}

#[test]
fn remove_duplicates_semantics_unchanged() {
    let e = env_with_stdlib();
    assert_eq!(
        eval_line("(remove-duplicates '(1 2 1 3 2 4))", &e),
        "(1 2 3 4)"
    );
    assert_eq!(eval_line("(remove-duplicates nil)", &e), "()");
    // First occurrence kept, order preserved.
    assert_eq!(eval_line("(remove-duplicates '(3 1 3 2 1))", &e), "(3 1 2)");
}

#[test]
fn string_to_list_semantics_unchanged() {
    let e = env_with_stdlib();
    assert_eq!(
        eval_line("(string->list \"abc\")", &e),
        "(\"a\" \"b\" \"c\")"
    );
    assert_eq!(eval_line("(string->list \"\")", &e), "()");
}

#[test]
fn string_join_semantics_unchanged() {
    let e = env_with_stdlib();
    assert_eq!(
        eval_line("(string-join (list \"a\" \"b\" \"c\") \",\")", &e),
        "\"a,b,c\""
    );
    assert_eq!(
        eval_line("(string-join (list \"only\") \",\")", &e),
        "\"only\""
    );
    assert_eq!(eval_line("(string-join nil \",\")", &e), "\"\"");
}

#[test]
fn string_repeat_semantics_unchanged() {
    let e = env_with_stdlib();
    assert_eq!(eval_line("(string-repeat \"ab\" 3)", &e), "\"ababab\"");
    assert_eq!(eval_line("(string-repeat \"ab\" 0)", &e), "\"\"");
    assert_eq!(eval_line("(string-repeat \"ab\" -1)", &e), "\"\"");
}

#[test]
fn make_string_semantics_unchanged() {
    let e = env_with_stdlib();
    assert_eq!(eval_line("(make-string 5 'x')", &e), "\"xxxxx\"");
    assert_eq!(eval_line("(make-string 0 'x')", &e), "\"\"");
    let out = eval_line("(make-string -1)", &e);
    assert!(out.contains("non-negative"), "got: {out}");
}

#[test]
fn string_capitalize_semantics_unchanged() {
    let e = env_with_stdlib();
    assert_eq!(
        eval_line("(string-capitalize \"hello world-foo\")", &e),
        "\"Hello World-Foo\""
    );
    assert_eq!(eval_line("(string-capitalize \"\")", &e), "\"\"");
}

#[test]
fn string_split_semantics_unchanged() {
    let e = env_with_stdlib();
    assert_eq!(
        eval_line("(string-split \",a,,b,\" \",\")", &e),
        "(\"\" \"a\" \"\" \"b\" \"\")"
    );
    assert_eq!(eval_line("(string-split \"abc\" \"z\")", &e), "(\"abc\")");
}
