//! Issue #526: the common forms CL-trained hands reach for -- LABELS, EQL,
//! TYPE-OF, MAKE-LIST, STRING-LENGTH, PARSE-INTEGER :JUNK-ALLOWED and the
//! `#\c` character syntax. The Lisp-level behavior is covered by
//! tests/lisp/97-common-forms.lisp; this file pins the host-visible parts:
//! reading and printing, the static types the checker sees, and records.

mod test_helpers;

use lamedh::{eval_line, reader};
use test_helpers::env_with_stdlib;

#[test]
fn hash_char_literal_reads_as_the_char_value() {
    let e = env_with_stdlib();
    // Same value as 'c', so it prints in the one canonical spelling.
    assert_eq!(eval_line("#\\a", &e), "'a'");
    assert_eq!(eval_line("#\\Space", &e), "' '");
    assert_eq!(eval_line("(list #\\( #\\) #\\\")", &e), "('(' ')' '\"')");
    assert_eq!(eval_line("(eq #\\a 'a')", &e), "T");
    assert_eq!(eval_line("(charp #\\newline)", &e), "T");
    // An unknown character name is a read error, not a char plus a symbol.
    assert!(eval_line("#\\ab", &e).contains("parse error"));
}

#[test]
fn hash_char_literal_does_not_confuse_repl_continuation() {
    assert!(!reader::is_incomplete("(list #\\( #\\\")"));
    assert!(reader::is_incomplete("(list #\\)"));
}

#[test]
fn labels_recursion_in_checked_definitions() {
    let e = env_with_stdlib();
    eval_line(
        "(defun* $sum-to (n) (labels ((lp (i acc) (if (> i n) acc (lp (+ i 1) (+ acc i))))) (lp 1 0)))",
        &e,
    );
    assert_eq!(eval_line("($sum-to 100)", &e), "5050");
    eval_line(
        "(defun $parity (n) (labels ((ev (k) (if (= k 0) 'even (od (- k 1)))) \
                                     (od (k) (if (= k 0) 'odd (ev (- k 1))))) \
                              (ev n)))",
        &e,
    );
    assert_eq!(
        eval_line("(list ($parity 10) ($parity 7))", &e),
        "(EVEN ODD)"
    );
}

#[test]
fn type_of_records_and_static_types() {
    let e = env_with_stdlib();
    eval_line("(defrecord Pt (x int) (y int))", &e);
    assert_eq!(eval_line("(type-of (make-Pt 1 2))", &e), "PT");
    // TYPE-OF accepts any value statically: its body's NULL/CONSP arms
    // would otherwise derive a (list a) argument and reject (type-of 1).
    assert_eq!(
        eval_line("(see-type 'type-of)", &e),
        "(DECLARED (FORALL (A) (-> (A) SYMBOL)))"
    );
    eval_line("(defun $kind () (type-of 1))", &e);
    assert_eq!(
        eval_line("(see-type '$kind)", &e),
        "(CHECKED (-> () SYMBOL))"
    );
    assert_eq!(eval_line("($kind)", &e), "INTEGER");
    assert_eq!(
        eval_line("(see-type 'eql)", &e),
        "(DECLARED (FORALL (A B) (-> (A B) BOOL)))"
    );
    assert_eq!(
        eval_line("(see-type 'string-length)", &e),
        "(DECLARED (-> (STRING) INT64))"
    );
}

#[test]
fn make_list_and_parse_integer_keywords() {
    let e = env_with_stdlib();
    assert_eq!(
        eval_line("(make-list 3 :initial-element 'x)", &e),
        "(X X X)"
    );
    assert_eq!(eval_line("(parse-integer \"12x\")", &e), "()");
    assert_eq!(
        eval_line("(parse-integer \"12x\" :junk-allowed t)", &e),
        "12"
    );
    // The idiom from the issue: read a leading count off a line.
    eval_line(
        "(defun $count-or-zero (s) (let ((n (parse-integer s :junk-allowed t))) (if n n 0)))",
        &e,
    );
    assert_eq!(eval_line("($count-or-zero \"3 apples\")", &e), "3");
    assert_eq!(eval_line("($count-or-zero \"apples\")", &e), "0");
}
