//! Stack traces on runtime errors: named frames are recorded pay-on-error
//! (success truncates them away), tail calls collapse into one frame (TCO
//! semantics), catchers (ERRORSET / HANDLER-CASE / the toplevel formatter)
//! consume the frames into LAST-BACKTRACE.

mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

#[test]
fn toplevel_errors_carry_the_non_tail_chain() {
    let e = env_with_stdlib();
    eval_line("(defun inner (x) (error \"boom\"))", &e);
    eval_line("(defun middle (x) (+ 1 (inner x)))", &e);
    eval_line("(defun outer () (* 2 (middle 41)))", &e);
    let out = eval_line("(outer)", &e);
    assert_eq!(
        out,
        "Error: boom\n  in: INNER \u{2190} MIDDLE \u{2190} OUTER"
    );
}

#[test]
fn tail_calls_collapse_into_one_frame() {
    let e = env_with_stdlib();
    // 100k tail-recursive frames are ONE trace entry, like Scheme.
    eval_line("(defun f (n) (if (< n 1) (car 5) (f (- n 1))))", &e);
    let out = eval_line("(f 100000)", &e);
    assert_eq!(out, "Error: CAR: expected a list, got 5\n  in: F");
}

#[test]
fn direct_toplevel_errors_format_unchanged() {
    let e = env_with_stdlib();
    // No named frames -> exactly the old single-line format.
    assert_eq!(
        eval_line("(car 5)", &e),
        "Error: CAR: expected a list, got 5"
    );
    assert_eq!(
        eval_line("nosuchvar", &e),
        "Error: Unbound variable: NOSUCHVAR"
    );
}

#[test]
fn handlers_read_last_backtrace() {
    let e = env_with_stdlib();
    eval_line("(defun deep () (car 5))", &e);
    eval_line("(defun wrap () (+ 1 (deep)))", &e);
    assert_eq!(
        eval_line("(handler-case (wrap) (error (er) (last-backtrace)))", &e),
        "(DEEP WRAP)"
    );
    // errorset consumes the frames the same way.
    eval_line("(errorset '(wrap))", &e);
    assert_eq!(eval_line("(last-backtrace)", &e), "(DEEP WRAP)");
}

#[test]
fn a_caught_error_leaves_no_stale_frames() {
    let e = env_with_stdlib();
    eval_line("(defun deep () (car 5))", &e);
    eval_line("(errorset '(deep))", &e);
    // A subsequent, DIRECT toplevel error must not inherit DEEP's frames.
    assert_eq!(
        eval_line("(car 5)", &e),
        "Error: CAR: expected a list, got 5"
    );
}

#[test]
fn control_flow_unwinds_do_not_corrupt_traces() {
    let e = env_with_stdlib();
    // THROW through named frames, caught: later errors trace correctly.
    eval_line("(defun thrower () (throw 'tag 42))", &e);
    assert_eq!(eval_line("(catch 'tag (+ 1 (thrower)))", &e), "42");
    eval_line("(defun deep () (car 5))", &e);
    let out = eval_line("(+ 1 (deep))", &e);
    assert_eq!(out, "Error: CAR: expected a list, got 5\n  in: DEEP");
}
