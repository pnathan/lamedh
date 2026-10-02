//! Array printing and the `#(...)` array literal (issue #527).
//!
//! Arrays print their contents as `#(e1 e2 ...)`, which the reader accepts
//! back; long arrays are abridged with an unreadable `#<...N more>` marker;
//! a self-containing array prints `#<circular-array>` at the back-reference;
//! typed arrays print their element type and contents as a non-readable
//! `#<typed-array:...>` tag; channel serialization never abridges.

use lamedh::environment::Environment;
use lamedh::printer::{ARRAY_PRINT_LIMIT, print, print_unabridged};
use lamedh::{LispVal, Shared, SharedCell, eval_line};

fn env() -> Shared<Environment> {
    Environment::with_stdlib()
}

#[test]
fn array_prints_its_elements() {
    let env = env();
    assert_eq!(eval_line("(list->array (list 1 2 3))", &env), "#(1 2 3)");
    assert_eq!(eval_line("(array 3)", &env), "#(() () ())");
    assert_eq!(eval_line("(array 0)", &env), "#()");
    assert_eq!(
        eval_line(
            "(list->array (list \"a\" 'b 'c' 1.5 (list 1 2) (list->array (list 4))))",
            &env
        ),
        "#(\"a\" B 'c' 1.5 (1 2) #(4))"
    );
}

#[test]
fn array_literal_reads_and_self_evaluates() {
    let env = env();
    assert_eq!(eval_line("#(1 2 3)", &env), "#(1 2 3)");
    assert_eq!(eval_line("'#(1 (a b) \"s\")", &env), "#(1 (A B) \"s\")");
    assert_eq!(eval_line("(arrayp #())", &env), "T");
    assert_eq!(eval_line("(array-length* #(a b c d))", &env), "4");
    // Elements are read, not evaluated.
    assert_eq!(eval_line("(fetch #((+ 1 2)) 0)", &env), "(+ 1 2)");
    // Nested literal is a nested array, not a list.
    assert_eq!(eval_line("(arrayp (fetch #(#(1)) 0))", &env), "T");
}

#[test]
fn array_print_read_round_trips() {
    let env = env();
    assert_eq!(
        eval_line(
            "(let* ((a (list->array (list 1 \"x\" (list 2 3) (list->array (list 4)))))
                    (b (read-from-string (prin1-to-string a))))
               (list (arrayp b) (array->list (fetch b 3)) (prin1-to-string b)))",
            &env
        ),
        "(T (4) \"#(1 \\\"x\\\" (2 3) #(4))\")"
    );
}

#[test]
fn array_literal_rejects_malformed_input() {
    let env = env();
    for bad in ["'#(1 . 2)", "'# (1 2)", "'#(1 2"] {
        let out = eval_line(bad, &env);
        assert!(out.contains("parse error"), "{bad} should not read: {out}");
    }
}

#[test]
fn long_array_is_abridged_with_unreadable_marker() {
    let env = env();
    let out = eval_line("(list->array (iota 150))", &env);
    let expected_head: Vec<String> = (0..ARRAY_PRINT_LIMIT).map(|i| i.to_string()).collect();
    assert_eq!(
        out,
        format!(
            "#({} #<...{} more>)",
            expected_head.join(" "),
            150 - ARRAY_PRINT_LIMIT
        )
    );
    // Exactly at the limit: nothing abridged.
    let at_limit = eval_line(&format!("(list->array (iota {ARRAY_PRINT_LIMIT}))"), &env);
    assert!(!at_limit.contains("more>"), "{at_limit}");
    // The marker never reads back as a shorter array.
    let reread = eval_line(
        "(read-from-string (prin1-to-string (list->array (iota 150))))",
        &env,
    );
    assert!(reread.contains("parse error"), "{reread}");
}

#[test]
fn unabridged_print_shows_every_element() {
    let elems: Vec<LispVal> = (0..(ARRAY_PRINT_LIMIT as i64 + 50))
        .map(LispVal::Number)
        .collect();
    let arr = LispVal::Array(Shared::new(SharedCell::new(elems)));
    let full = print_unabridged(&arr);
    assert!(!full.contains("more>"), "{full}");
    assert!(full.ends_with(&format!(" {})", ARRAY_PRINT_LIMIT + 49)));
    // The unabridged scope is restored afterwards.
    assert!(print(&arr).contains("#<...50 more>"));
}

#[test]
fn circular_array_prints_back_reference() {
    let env = env();
    assert_eq!(
        eval_line("(let ((a (array 2))) (store a 0 a) a)", &env),
        "#(#<circular-array> ())"
    );
    assert_eq!(
        eval_line("(let ((a (array 2))) (store a 1 (list 1 a)) a)", &env),
        "#(() (1 #<circular-array>))"
    );
    // The same array twice, but not nested in itself, is not circular.
    assert_eq!(
        eval_line("(let ((a #(1))) (list->array (list a a)))", &env),
        "#(#(1) #(1))"
    );
}

#[test]
fn typed_array_prints_element_type_and_contents() {
    let env = env();
    assert_eq!(
        eval_line("(let ((a (typed-array 3 'int64))) (store a 1 7) a)", &env),
        "#<typed-array:int64 0 7 0>"
    );
    assert_eq!(
        eval_line("(typed-array 2 'float64)", &env),
        "#<typed-array:float64 0.0 0.0>"
    );
    assert_eq!(
        eval_line("(typed-array 0 'int64)", &env),
        "#<typed-array:int64>"
    );
    let long = eval_line("(typed-array 101 'int64)", &env);
    assert!(long.ends_with(" 0 #<...1 more>>"), "{long}");
}

#[cfg(feature = "concurrency")]
#[test]
fn channel_send_does_not_abridge_arrays() {
    let env = env();
    assert_eq!(
        eval_line(
            "(let ((c (make-channel)))
               (channel-send c (list->array (iota 150)))
               (let ((b (channel-recv c)))
                 (list (arrayp b) (array-length* b) (fetch b 149))))",
            &env
        ),
        "(T 150 149)"
    );
}

#[test]
fn array_literal_counts_one_nesting_level() {
    // `#(...)` nests exactly like `(...)` under the reader's depth limit,
    // and hitting the limit is a parse error, not a panic.
    let env = env();
    for limit in 1..=4 {
        for depth in 1..=4 {
            let list = format!("{}1{}", "(".repeat(depth), ")".repeat(depth));
            let arr = format!("{}1{}", "#(".repeat(depth), ")".repeat(depth));
            let l = lamedh::reader::read_with_depth_limit(&list, &env, limit).is_ok();
            let a = lamedh::reader::read_with_depth_limit(&arr, &env, limit).is_ok();
            assert_eq!(l, a, "depth {depth}, limit {limit}");
        }
    }
}
