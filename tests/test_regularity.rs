//! The 0.3 regularity pass: one convention where there were several, and
//! reserved-word landmines removed. Breaking changes recorded in
//! CHANGELOG.md — no compatibility shims.

mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

#[test]
fn hash_ops_are_collection_first_only() {
    let e = env_with_stdlib();
    assert_eq!(
        eval_line(
            "(let ((h (make-hash-table))) (sethash h 'k 1) (gethash h 'k))",
            &e
        ),
        "1"
    );
    // The old either-order guess is gone.
    let out = eval_line(
        "(let ((h (make-hash-table))) (sethash h 'k 1) (gethash 'k h))",
        &e,
    );
    assert!(out.contains("collection first"), "got: {out}");
    assert_eq!(
        eval_line(
            "(let ((h (make-hash-table))) (sethash h 'k 1) (remhash h 'k) (gethash h 'k))",
            &e
        ),
        "()"
    );
}

#[test]
fn comparisons_are_variadic_monotone_chains() {
    let e = env_with_stdlib();
    assert_eq!(eval_line("(< 1 2 3 10)", &e), "T");
    assert_eq!(eval_line("(< 1 3 2)", &e), "()");
    assert_eq!(eval_line("(> 9 5 1)", &e), "T");
    assert_eq!(eval_line("(= 4 4 4)", &e), "T");
    assert_eq!(eval_line("(<= 1 1 2)", &e), "T");
    assert_eq!(eval_line("(>= 3 3 1)", &e), "T");
    assert_eq!(eval_line("(>= 3 4)", &e), "()");
}

#[test]
fn set_is_the_value_level_setter() {
    let e = env_with_stdlib();
    assert_eq!(eval_line("(progn (set 'x 42) x)", &e), "42");
    // Computed symbol — the case the quoting CSET macro cannot express.
    assert_eq!(eval_line("(progn (set (car (list 'y)) 7) y)", &e), "7");
    let out = eval_line("(set 't 1)", &e);
    assert!(out.contains("Error"), "got: {out}");
}

#[test]
fn hex_literals_are_digit_leading() {
    let e = env_with_stdlib();
    assert_eq!(eval_line("0FFh", &e), "255");
    assert_eq!(eval_line("1Ah", &e), "26");
    // `ch` and friends are ordinary symbols now.
    assert_eq!(eval_line("(let ((ch 12)) ch)", &e), "12");
    assert_eq!(eval_line("(let ((each 'x)) each)", &e), "X");
}

#[test]
fn label_is_an_ordinary_name_in_data_and_bindings() {
    let e = env_with_stdlib();
    // A list value headed by the symbol LABEL is DATA (the old variable-read
    // auto-eval hack is gone)...
    assert_eq!(eval_line("(let ((x (list 'label 2))) x)", &e), "(LABEL 2)");
    // ...while the LABEL special form in operator position still works.
    assert_eq!(
        eval_line(
            "((label fact (lambda (n) (if (< n 1) 1 (* n (fact (- n 1)))))) 5)",
            &e
        ),
        "120"
    );
    // And records may have a LABEL field.
    eval_line("(defrecord tagged (label symbol))", &e);
    assert_eq!(eval_line("(tagged-label (make-tagged 'hi))", &e), "HI");
}

#[test]
fn defun_supports_optional_and_key_parameters() {
    let e = env_with_stdlib();
    eval_line(
        "(defun f (a &optional (b 2) c &key (d 3) label) (list a b c d label))",
        &e,
    );
    assert_eq!(eval_line("(f 1)", &e), "(1 2 () 3 ())");
    assert_eq!(eval_line("(f 1 9)", &e), "(1 9 () 3 ())");
    assert_eq!(eval_line("(f 1 9 8 :d 4 :label 'x)", &e), "(1 9 8 4 X)");
    // Defaults may reference earlier parameters (LET* semantics).
    eval_line("(defun g (a &optional (b (* a 10))) (list a b))", &e);
    assert_eq!(eval_line("(g 2)", &e), "(2 20)");
    assert_eq!(eval_line("(g 2 5)", &e), "(2 5)");
    // &rest interoperates: binds everything after the optionals.
    eval_line(
        "(defun h (a &optional b &rest r &key (c 3)) (list a b r c))",
        &e,
    );
    assert_eq!(eval_line("(h 1 2 :c 9)", &e), "(1 2 (:C 9) 9)");
}

#[test]
fn checker_demands_numeric_arithmetic() {
    let e = env_with_stdlib();
    // #322: known non-numerics rejected statically, like the evaluator would
    // at runtime.
    let out = eval_line("(check-type (+ \"a\" \"b\"))", &e);
    assert!(out.contains("expects numeric operands"), "got: {out}");
    let out = eval_line("(check-type (< \"x\" \"y\"))", &e);
    assert!(out.contains("comparable"), "got: {out}");
    // Char arithmetic and comparison are evaluator-legal: allowed.
    assert_eq!(eval_line("(check-type (+ 'a' 'b'))", &e), "\"char\"");
    assert_eq!(eval_line("(check-type (< 'a' 'b'))", &e), "\"bool\"");
    // Polymorphic code is untouched — no scheme churn.
    eval_line("(defun sq (x) (* x x))", &e);
    assert_eq!(
        eval_line("(see-type 'sq)", &e),
        "(CHECKED (FORALL (A) (-> (A) A)))"
    );
}

#[test]
fn variadic_operators_census_batch() {
    let e = env_with_stdlib();
    // append: kernel, variadic, dotted tail preserved dynamically.
    assert_eq!(eval_line("(append)", &e), "()");
    assert_eq!(
        eval_line("(append (list 1) (list 2) (list 3))", &e),
        "(1 2 3)"
    );
    assert_eq!(eval_line("(append (list 1) 'tail)", &e), "(1 . TAIL)");
    // gcd/lcm fold with CL identities.
    assert_eq!(eval_line("(gcd 12 18 24)", &e), "6");
    assert_eq!(eval_line("(gcd)", &e), "0");
    assert_eq!(eval_line("(lcm 2 3 4)", &e), "12");
    assert_eq!(eval_line("(lcm)", &e), "1");
    // logior is the canonical CL name (logor removed).
    assert_eq!(eval_line("(logior 1 2 4)", &e), "7");
    let out = eval_line("(logor 1 2)", &e);
    assert!(out.contains("Unbound"), "got: {out}");
    // mapcar zips N lists, stopping at the shortest.
    assert_eq!(
        eval_line("(mapcar #'+ (list 1 2 3) (list 10 20 30))", &e),
        "(11 22 33)"
    );
    assert_eq!(
        eval_line("(mapcar #'+ (list 1 2 3) (list 10 20))", &e),
        "(11 22)"
    );
    // Checker rules for the variadic family.
    assert_eq!(
        eval_line("(check-type (append (list 1) (list 2)))", &e),
        "\"(list int64)\""
    );
    let out = eval_line("(check-type (append (list 1) (list \"s\")))", &e);
    assert!(out.contains("type error"), "got: {out}");
    assert_eq!(
        eval_line("(check-type (concat \"a\" \"b\"))", &e),
        "\"string\""
    );
    assert_eq!(eval_line("(check-type (min 1 2 3))", &e), "\"int64\"");
    let out = eval_line("(check-type (min \"a\" \"b\"))", &e);
    assert!(out.contains("numeric"), "got: {out}");
}

#[test]
fn stdlib_staples_0_3() {
    // The census gap probe: sort-by, enumerate, frequencies, padding.
    let e = env_with_stdlib();
    assert_eq!(
        eval_line("(enumerate (list 'a 'b 'c))", &e),
        "((0 A) (1 B) (2 C))"
    );
    assert_eq!(eval_line("(enumerate (list 'a 'b) 1)", &e), "((1 A) (2 B))");
    assert_eq!(
        eval_line("(frequencies (list 'a 'b 'a 'a 'c 'b))", &e),
        "((A . 3) (B . 2) (C . 1))"
    );
    assert_eq!(
        eval_line("(check-type (frequencies (list 'a)))", &e),
        "\"(list (pair symbol int64))\""
    );
    // sort-by: collection first (like sort); default key comparison #'<,
    // optional predicate for other orders.
    assert_eq!(
        eval_line(
            "(sort-by (list (list 3 'x) (list 1 'y) (list 2 'z)) #'car)",
            &e
        ),
        "((1 Y) (2 Z) (3 X))"
    );
    assert_eq!(
        eval_line(
            "(sort-by (list \"bb\" \"a\" \"ccc\") #'string-length* #'>)",
            &e
        ),
        "(\"ccc\" \"bb\" \"a\")"
    );
    // Padding: never truncates, defaults to spaces.
    assert_eq!(eval_line("(string-pad-left \"42\" 5)", &e), "\"   42\"");
    assert_eq!(
        eval_line("(string-pad-left \"42\" 5 \"0\")", &e),
        "\"00042\""
    );
    assert_eq!(
        eval_line("(string-pad-right \"ab\" 4 \".\")", &e),
        "\"ab..\""
    );
    assert_eq!(eval_line("(string-pad-left \"hello\" 3)", &e), "\"hello\"");
    assert_eq!(eval_line("(string-repeat \"ab\" 3)", &e), "\"ababab\"");
}
