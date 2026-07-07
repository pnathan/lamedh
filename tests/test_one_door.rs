//! One-door gradual defun: every plain `defun` quietly attempts typed
//! compilation (via jit-optimize's auto-typed membrane, which keeps the
//! original closure as fallback); functions that don't infer stay
//! interpreted silently. Opt-outs: `(declare (no-compile))` in the body or
//! `(declaim (no-compile name...))` — both also gate explicit jit-optimize
//! (issue #168). "Types are weather, not architecture."

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line};

fn env() -> Shared<Environment> {
    Environment::with_stdlib()
}

#[test]
fn inferable_defun_compiles_quietly() {
    let e = env();
    // A literal pins the numeric kind; fully-ambiguous bodies like
    // (* x x) stay CHECKED by design (inference cannot pick a kind).
    eval_line("(defun od-sq (x) (* x (* x 1)))", &e);
    assert_eq!(
        eval_line("(see-type 'od-sq)", &e),
        "(TYPED (-> (INT64) INT64) COMPILED)"
    );
    assert_eq!(eval_line("(od-sq 9)", &e), "81");
}

#[test]
fn uninferable_defun_stays_interpreted_silently() {
    let e = env();
    eval_line("(defun od-first (l) (car l))", &e);
    let out = eval_line("(see-type 'od-first)", &e);
    assert!(
        out.starts_with("(CHECKED"),
        "list fn stays uncompiled: {out}"
    );
    assert_eq!(eval_line("(od-first '(7 8))", &e), "7");
}

#[test]
fn introspection_survives_compilation() {
    let e = env();
    eval_line("(defun od-dbl (x) (* x 2))", &e);
    assert_eq!(
        eval_line("(see-source 'od-dbl)", &e),
        "(LAMBDA (X) (* X 2))"
    );
}

#[test]
fn declare_no_compile_pins_to_the_tree_walker() {
    let e = env();
    eval_line("(defun od-pin (x) (declare (no-compile)) (* x 2))", &e);
    // The declare form is stripped from the body and the function works.
    assert_eq!(eval_line("(od-pin 21)", &e), "42");
    let out = eval_line("(see-type 'od-pin)", &e);
    assert!(
        out.starts_with("(CHECKED"),
        "pinned fn must not compile: {out}"
    );
    // Explicit jit-optimize is refused with the #168 status.
    assert_eq!(
        eval_line("(jit-optimize od-pin)", &e),
        "\"OD-PIN: compile disabled by declaration\""
    );
}

#[test]
fn declaim_no_compile_covers_later_definitions() {
    let e = env();
    eval_line("(declaim (no-compile od-later od-later2))", &e);
    eval_line("(defun od-later (x) (+ x 1))", &e);
    let out = eval_line("(see-type 'od-later)", &e);
    assert!(
        out.starts_with("(CHECKED"),
        "declaimed fn must not compile: {out}"
    );
    assert_eq!(
        eval_line("(jit-optimize od-later)", &e),
        "\"OD-LATER: compile disabled by declaration\""
    );
}

#[test]
fn compiled_defuns_keep_flag_parity() {
    let e = env();
    eval_line("(defun od-mul2 (x) (* x 2))", &e);
    assert_eq!(
        eval_line("(see-type 'od-mul2)", &e),
        "(TYPED (-> (INT64) INT64) COMPILED)"
    );
    eval_line("(clear-all-flags)", &e);
    eval_line("(od-mul2 9223372036854775807)", &e);
    assert_eq!(
        eval_line("(flag-set-p 'OVERFLOW)", &e),
        "T",
        "the compiled edition must set OVERFLOW exactly like the evaluator"
    );
}

#[test]
fn fallback_handles_nonmatching_arguments() {
    let e = env();
    eval_line("(defun od-add1 (x) (+ x 1))", &e);
    // Typed fast path:
    assert_eq!(eval_line("(od-add1 41)", &e), "42");
    // Non-int argument falls back to the original closure:
    assert_eq!(eval_line("(od-add1 1.5)", &e), "2.5");
}
