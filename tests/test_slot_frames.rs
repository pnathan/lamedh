//! Regression tests for slot frames with routing tables (issue #200 M3,
//! answering #126's sound-lexical-addressing question): compiled parameter
//! reads become direct slot indexing, while every name/id-based write
//! routes through the frame's table — so dynamic shadow-writes hit the
//! same storage the compiled read observes. Coverage affects speed, never
//! correctness: unrouted frames fall back to full resolution.

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line, with_large_stack};

fn env() -> Shared<Environment> {
    Environment::with_stdlib()
}

#[test]
fn macro_shadow_write_of_a_param_is_observed_by_compiled_reads() {
    let e = env();
    // The exact case that killed GlobalGet: an operative's expansion writes
    // the caller's frame. With routing, the write lands in the param's slot.
    eval_line("(defmacro sf-clobber () '(setq x 999))", &e);
    eval_line("(defun sf-victim (x) (sf-clobber) x)", &e);
    assert_eq!(eval_line("(sf-victim 1)", &e), "999");
}

#[test]
fn runtime_eval_shadow_write_is_observed_too() {
    let e = env();
    eval_line("(defun sf-victim2 (x) (eval '(setq x 777)) x)", &e);
    assert_eq!(eval_line("(sf-victim2 1)", &e), "777");
}

#[test]
fn fresh_bindings_from_expansions_land_in_the_frame_map() {
    let e = env();
    eval_line("(defmacro sf-fresh () '(def y 42))", &e);
    eval_line("(defun sf-victim3 (x) (sf-fresh) (+ x y))", &e);
    assert_eq!(eval_line("(sf-victim3 1)", &e), "43");
}

#[test]
fn nested_closures_read_outer_params_across_depth() {
    let e = env();
    eval_line("(defun sf-outer (x) (funcall (lambda (y) (+ x y)) 10))", &e);
    assert_eq!(eval_line("(sf-outer 32)", &e), "42");
    // Two levels of nesting, both directions.
    eval_line(
        "(defun sf-deep (a) (funcall (lambda (b) (funcall (lambda (c) (list a b c)) 3)) 2))",
        &e,
    );
    assert_eq!(eval_line("(sf-deep 1)", &e), "(1 2 3)");
}

#[test]
fn reads_past_let_frames_count_depth_correctly() {
    let e = env();
    eval_line("(defun sf-lv (a) (let ((b (+ a 1))) (+ a b)))", &e);
    assert_eq!(eval_line("(sf-lv 20)", &e), "41");
    // Let shadowing a param: the inner binder wins.
    eval_line("(defun sf-shadow (a) (let ((a 100)) a))", &e);
    assert_eq!(eval_line("(sf-shadow 1)", &e), "100");
}

#[test]
fn rest_lambdas_use_mixed_frames() {
    let e = env();
    eval_line("(defun sf-rst (a &rest more) (list a more))", &e);
    assert_eq!(eval_line("(sf-rst 1 2 3)", &e), "(1 (2 3))");
}

#[test]
fn param_made_dynamic_mid_flight_defers_to_the_cell() {
    let e = env();
    // Obscure but part of the soundness pledge: making a param's symbol
    // dynamic mid-body must make subsequent reads see the dynamic cell,
    // exactly as the tree-walker's resolve would.
    eval_line("(defun sf-dyn (q) (eval '(defdynamic q 555)) q)", &e);
    assert_eq!(eval_line("(sf-dyn 1)", &e), "555");
}

#[test]
fn tco_and_first_class_env_still_hold() {
    with_large_stack(|| {
        let e = env();
        eval_line(
            "(defun sf-count (n a) (if (< n 1) a (sf-count (- n 1) (+ a 1))))",
            &e,
        );
        assert_eq!(eval_line("(sf-count 1000000 0)", &e), "1000000");
        // Slot bindings are visible through first-class environments.
        eval_line("(defun sf-cap (v) (the-environment))", &e);
        assert_eq!(eval_line("(eval 'v (sf-cap 42))", &e), "42");
    });
}
