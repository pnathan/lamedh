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
        "(definstance volume ((s string)) int64 (string-length s))",
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
    // Kind-preserving map, collection first (protocols dispatch on arg 1).
    assert_eq!(eval_line("(map (list 1 2 3) #'1+)", &e), "(2 3 4)");
    assert_eq!(
        eval_line("(array->list (map (list->array (list 1 2)) #'1+))", &e),
        "(2 3)"
    );
    let out = eval_line("(check-type (map 5 #'1+))", &e);
    assert!(out.contains("no `MAP` instance for int64"), "got: {out}");
    // for-each over lists and hash tables (key value pairs), for effect.
    assert_eq!(
        eval_line(
            "(let ((n (array 1)))
               (store n 0 0)
               (for-each (list 1 2 3) (lambda (x) (store n 0 (+ (fetch n 0) x))))
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
               (for-each h (lambda (k v) (store acc 0 (+ (fetch acc 0) v))))
               (fetch acc 0))",
            &e
        ),
        "5"
    );
}
