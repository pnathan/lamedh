//! Typed protocols (0.3): one name, many typed instances, resolved by
//! inference at check time, by value kind at runtime, and compiled per
//! instance through the one-door pipeline. LENGTH is the shipped pilot.

mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

#[test]
fn length_pilot_covers_all_kinds() {
    let e = env_with_stdlib();
    assert_eq!(eval_line("(length (list 1 2 3))", &e), "3");
    assert_eq!(eval_line("(length \"abcd\")", &e), "4");
    assert_eq!(eval_line("(length (array 5))", &e), "5");
    assert_eq!(
        eval_line(
            "(let ((h (make-hash-table))) (sethash h 'a 1) (length h))",
            &e
        ),
        "1"
    );
}

#[test]
fn checker_selects_instances_by_shape() {
    let e = env_with_stdlib();
    assert_eq!(
        eval_line("(check-type (length (list 1 2)))", &e),
        "\"int64\""
    );
    assert_eq!(eval_line("(check-type (length \"abc\"))", &e), "\"int64\"");
    // A known type with NO instance is the promised misuse error.
    let out = eval_line("(check-type (length 3.5))", &e);
    assert!(
        out.contains("no `LENGTH` instance for float64"),
        "got: {out}"
    );
    // An unknown argument still types the result: every instance agrees
    // on int64, so callers derive it gradually.
    eval_line("(defun n-items (x) (length x))", &e);
    assert_eq!(
        eval_line("(see-type 'n-items)", &e),
        "(CHECKED (FORALL (A) (-> (A) INT64)))"
    );
}

#[test]
fn user_types_join_the_protocol() {
    let e = env_with_stdlib();
    eval_line("(defrecord playlist (songs (list string)))", &e);
    eval_line(
        "(definstance length ((p playlist)) int64 (length (playlist-songs p)))",
        &e,
    );
    assert_eq!(
        eval_line("(length (make-playlist (list \"a\" \"b\")))", &e),
        "2"
    );
    assert_eq!(
        eval_line("(check-type (length (make-playlist (list \"a\"))))", &e),
        "\"int64\""
    );
    // The hidden implementation name contains `@`, which the reader does
    // not accept — instances cannot be shadowed or called around the
    // dispatcher from source. (They still compile like any defun; the
    // dispatcher looked one up to produce the results above.)
    let out = eval_line("(see-type '$length@playlist)", &e);
    assert!(out.contains("parse error"), "got: {out}");
}

#[test]
fn new_protocols_are_first_class() {
    let e = env_with_stdlib();
    eval_line("(defprotocol volume \"loudness of a thing\")", &e);
    eval_line("(definstance volume ((n int64)) int64 (* n 2))", &e);
    eval_line(
        "(definstance volume ((s string)) int64 (string-length* s))",
        &e,
    );
    assert_eq!(eval_line("(volume 4)", &e), "8");
    assert_eq!(eval_line("(volume \"abc\")", &e), "3");
    assert_eq!(eval_line("(check-type (volume 4))", &e), "\"int64\"");
    let out = eval_line("(check-type (volume (list 1)))", &e);
    assert!(out.contains("no `VOLUME` instance"), "got: {out}");
    // Runtime misuse errors too (no fallback registered).
    let out = eval_line("(volume 3.5)", &e);
    assert!(out.contains("no VOLUME instance"), "got: {out}");
}

#[test]
fn variant_values_dispatch_through_their_variant() {
    let e = env_with_stdlib();
    eval_line("(defvariant (box2 a) (full (item a)) (empty))", &e);
    eval_line(
        "(definstance length ((b (box2 a))) int64
           (variant-case b (full (x) 1) (empty () 0)))",
        &e,
    );
    assert_eq!(eval_line("(length (full 'x))", &e), "1");
    assert_eq!(eval_line("(length (empty))", &e), "0");
    assert_eq!(eval_line("(check-type (length (full 1)))", &e), "\"int64\"");
}

#[test]
fn map_and_for_each_are_sequence_protocols() {
    let e = env_with_stdlib();
    // Kind-preserving map, FUNCTION FIRST (the CL HOF convention); the
    // protocol dispatches on argument position 1.
    assert_eq!(eval_line("(map #'1+ (list 1 2 3))", &e), "(2 3 4)");
    assert_eq!(
        eval_line("(array->list (map #'1+ (list->array (list 1 2))))", &e),
        "(2 3)"
    );
    let out = eval_line("(check-type (map #'1+ 5))", &e);
    assert!(out.contains("no `MAP` instance for int64"), "got: {out}");
    // Strings map to strings (kind preservation across all three kinds).
    assert_eq!(eval_line("(map #'string-upcase \"abc\")", &e), "\"ABC\"");
    assert_eq!(
        eval_line("(check-type (map #'string-upcase \"ab\"))", &e),
        "\"string\""
    );
    // for-each over lists and hash tables (key value pairs), for effect.
    assert_eq!(
        eval_line(
            "(let ((n (array 1)))
               (store n 0 0)
               (for-each (lambda (x) (store n 0 (+ (fetch n 0) x))) (list 1 2 3))
               (fetch n 0))",
            &e
        ),
        "6"
    );
    assert_eq!(
        eval_line(
            "(let ((h (make-hash-table)) (acc (array 1)))
               (store acc 0 0)
               (sethash h 'a 2) (sethash h 'b 3)
               (for-each (lambda (k v) (store acc 0 (+ (fetch acc 0) v))) h)
               (fetch acc 0))",
            &e
        ),
        "5"
    );
    // Strings visit per character (one-char strings, like string->list).
    assert_eq!(
        eval_line(
            "(let ((acc (array 1)))
               (store acc 0 \"\")
               (for-each (lambda (c) (store acc 0 (concat (fetch acc 0) c))) \"xyz\")
               (fetch acc 0))",
            &e
        ),
        "\"xyz\""
    );
    // Custom fn-first protocols via (:dispatch 1).
    eval_line("(defprotocol pick \"grab\" (:dispatch 1))", &e);
    eval_line(
        "(definstance pick ((f any) (xs (list a))) any (funcall f (car xs)))",
        &e,
    );
    eval_line(
        "(definstance pick ((f any) (s string)) any (funcall f (ref s 0)))",
        &e,
    );
    assert_eq!(eval_line("(pick #'1+ (list 41 5))", &e), "42");
    assert_eq!(eval_line("(pick #'string-upcase \"x y\")", &e), "\"X\"");
}

#[test]
fn ref_is_the_strict_read_protocol() {
    let e = env_with_stdlib();
    // One name over fetch/gethash/nth/elt/record-ref, collection first.
    assert_eq!(eval_line("(ref (list 'a 'b 'c) 1)", &e), "B");
    assert_eq!(eval_line("(ref (list->array (list 1 2)) 0)", &e), "1");
    assert_eq!(eval_line("(ref \"hello\" 1)", &e), "\"e\"");
    assert_eq!(
        eval_line(
            "(let ((h (make-hash-table))) (sethash h 'k 9) (ref h 'k))",
            &e
        ),
        "9"
    );
    // Records read by field name (the pre-protocol fallback, by brand).
    eval_line("(defrecord pt2 (x int64) (y int64))", &e);
    assert_eq!(eval_line("(ref (make-pt2 3 4) 'y)", &e), "4");
    // STRICT: absence errors (the lenient reads keep their old names).
    let out = eval_line("(ref (list 'a) 5)", &e);
    assert!(out.contains("out of bounds"), "got: {out}");
    let out = eval_line("(ref (make-hash-table) 'missing)", &e);
    assert!(out.contains("missing key"), "got: {out}");
    // Strictness is what buys honest result types.
    assert_eq!(
        eval_line("(check-type (ref (list 1 2) 0))", &e),
        "\"int64\""
    );
    assert_eq!(eval_line("(check-type (ref \"ab\" 0))", &e), "\"string\"");
    let out = eval_line("(check-type (ref 3.5 0))", &e);
    assert!(out.contains("no `REF` instance for float64"), "got: {out}");
}

#[test]
fn put_bang_writes_mutable_containers() {
    let e = env_with_stdlib();
    assert_eq!(
        eval_line("(let ((a (array 2))) (put! a 0 'v) (fetch a 0))", &e),
        "V"
    );
    assert_eq!(
        eval_line(
            "(let ((h (make-hash-table))) (put! h 'k 7) (gethash h 'k))",
            &e
        ),
        "7"
    );
    // Returns the written value, typed.
    assert_eq!(
        eval_line("(check-type (put! (list->array (list 1)) 0 5))", &e),
        "\"int64\""
    );
    // Immutable kinds have no instance.
    let out = eval_line("(check-type (put! (list 1) 0 5))", &e);
    assert!(out.contains("no `PUT!` instance"), "got: {out}");
}

#[test]
fn copy_protocol_covers_every_kind() {
    let e = env_with_stdlib();
    // Hash copy is fresh (the previously-missing case).
    assert_eq!(
        eval_line(
            "(let* ((h (make-hash-table))
                    (c (progn (sethash h 'a 1) (copy h))))
               (sethash c 'a 99)
               (list (gethash h 'a) (gethash c 'a)))",
            &e
        ),
        "(1 99)"
    );
    assert_eq!(
        eval_line(
            "(let* ((a (list->array (list 1 2))) (c (copy a)))
               (store c 0 9)
               (list (fetch a 0) (fetch c 0)))",
            &e
        ),
        "(1 9)"
    );
    assert_eq!(eval_line("(copy (list 1 2))", &e), "(1 2)");
    assert_eq!(
        eval_line("(check-type (copy (list 1 2)))", &e),
        "\"(list int64)\""
    );
    // Immutable kinds: identity; atoms via the Lisp 1.5 fallback.
    assert_eq!(eval_line("(copy \"s\")", &e), "\"s\"");
    assert_eq!(eval_line("(copy 'atom)", &e), "ATOM");
}

#[test]
fn filter_is_a_generic_fn_first_protocol() {
    let e = env_with_stdlib();
    assert_eq!(eval_line("(filter #'evenp (list 1 2 3 4))", &e), "(2 4)");
    assert_eq!(
        eval_line(
            "(array->list (filter #'evenp (list->array (list 1 2 3 4))))",
            &e
        ),
        "(2 4)"
    );
    assert_eq!(eval_line("(filter #'alpha-p \"a1b2c\")", &e), "\"abc\"");
    assert_eq!(
        eval_line("(check-type (filter #'evenp (list 1 2)))", &e),
        "\"(list int64)\""
    );
    let out = eval_line("(check-type (filter #'evenp 5))", &e);
    assert!(out.contains("no `FILTER` instance for int64"), "got: {out}");
}
