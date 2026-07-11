//! ONE dispatch system (0.3, Paul's ruling: "only one can live").
//! `definterface`/`implements?`/`method` are gone; `defprotocol` owns
//! dispatch, and conformance is `implements!`/`implements-p` over
//! protocol instances — graded by the checker's verdict on each
//! instance's implementation.

mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

#[test]
fn the_old_interface_system_is_gone() {
    let env = env_with_stdlib();
    for gone in ["definterface", "implements?", "method", "method-symbol"] {
        let out = eval_line(&format!("(boundp '{gone})"), &env);
        assert_eq!(out, "()", "{gone} should be unbound, got: {out}");
    }
}

#[test]
fn implements_asserts_protocol_conformance() {
    let env = env_with_stdlib();
    eval_line("(defrecord goblin2 (name string) (hp int64))", &env);
    eval_line("(defprotocol greet2 \"voice\")", &env);
    eval_line(
        "(definstance greet2 ((self goblin2)) string (goblin2-name self))",
        &env,
    );
    // Conformance to a contract = clean instances for every named protocol.
    assert_eq!(eval_line("(implements-p 'goblin2 'greet2)", &env), "T");
    assert_eq!(
        eval_line("(implements! 'goblin2 'greet2)", &env),
        "((GREET2 . INSTANCE))"
    );
    // A protocol with no instance for the brand is MISSING and fails.
    eval_line("(defprotocol render2 \"draw\")", &env);
    assert_eq!(eval_line("(implements-p 'goblin2 'render2)", &env), "()");
    let out = eval_line("(implements! 'goblin2 'greet2 'render2)", &env);
    assert!(out.contains("RENDER2 . MISSING"), "got: {out}");
}

#[test]
fn type_erroring_instances_are_graded_mismatch() {
    let env = env_with_stdlib();
    eval_line("(defrecord thing2 (n int64))", &env);
    eval_line("(defprotocol bump2 \"b\")", &env);
    // The implementation misuses its field: the hidden defun's checker
    // verdict is a type error, so the claim is refused as MISMATCH.
    eval_line(
        "(definstance bump2 ((self thing2)) int64 (concat (thing2-n self) \"x\"))",
        &env,
    );
    let out = eval_line("(implements! 'thing2 'bump2)", &env);
    assert!(out.contains("BUMP2 . MISMATCH"), "got: {out}");
    assert_eq!(eval_line("(implements-p 'thing2 'bump2)", &env), "()");
}

#[test]
fn protocol_dispatch_replaces_method_dispatch() {
    let env = env_with_stdlib();
    eval_line(
        "(defrecord invoice2 (id int64) (amount int64) (:derive equality))",
        &env,
    );
    eval_line("(defprotocol total2 \"sum\")", &env);
    eval_line(
        "(definstance total2 ((self invoice2)) int64 (invoice2-amount self))",
        &env,
    );
    assert_eq!(eval_line("(total2 (make-invoice2 1 40))", &env), "40");
    // Instances are checker-selected at typed call sites too.
    assert_eq!(
        eval_line("(check-type (total2 (make-invoice2 1 2)))", &env),
        "\"int64\""
    );
}
