//! Tests for the symbol value-cell global namespace.
//!
//! Root-level (global) bindings live in a per-symbol value cell rather than a
//! HashMap on the root environment frame. These tests pin the observable
//! semantics: globals resolve, redefine, and update; locals still shadow;
//! functions see globals; and independent `make-environment` namespaces stay
//! isolated (each interpreter chain has its own symbol table, hence its own
//! cells).

mod test_helpers;
use lamedh::eval_line;
use test_helpers::env_with_stdlib;

#[test]
fn global_def_and_lookup() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(progn (def g 10) g)", &env), "10");
}

#[test]
fn global_redefinition_takes_effect() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(progn (def g 1) (def g 2) g)", &env), "2");
}

#[test]
fn global_setq_updates_cell() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(progn (def g 1) (setq g 5) g)", &env), "5");
}

#[test]
fn top_level_setq_creates_global() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(progn (setq fresh 7) fresh)", &env), "7");
}

#[test]
fn function_body_sees_global() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line(
            "(progn (def base 100) (defun addbase (x) (+ x base)) (addbase 5))",
            &env
        ),
        "105"
    );
}

#[test]
fn local_param_shadows_global_and_global_survives() {
    let env = env_with_stdlib();
    // Inside f, x is the param (99); the global x (1) is untouched afterward.
    assert_eq!(
        eval_line("(progn (def x 1) (defun f (x) x) (list (f 99) x))", &env),
        "(99 1)"
    );
}

#[test]
fn let_binding_shadows_global() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("(progn (def y 1) (list (let ((y 2)) y) y))", &env),
        "(2 1)"
    );
}

#[test]
fn make_environment_namespace_is_isolated() {
    let env = env_with_stdlib();
    // Defining a global inside a fresh environment must not clobber this one's
    // binding of the same name — separate symbol table, separate cell.
    assert_eq!(
        eval_line(
            "(progn (def leaky 1) (eval (quote (def leaky 999)) (make-environment)) leaky)",
            &env
        ),
        "1"
    );
}

#[test]
fn boundp_reflects_global_cell() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line(
            "(progn (def bb 1) (list (boundp (quote bb)) (boundp (quote never-defined))))",
            &env
        ),
        "(T ())"
    );
}

#[test]
fn current_environment_includes_globals() {
    let env = env_with_stdlib();
    // current-environment reifies all visible bindings, including global cells.
    assert_eq!(
        eval_line(
            "(progn (def ce 7) (gethash (current-environment) (quote ce)))",
            &env
        ),
        "7"
    );
}

#[test]
fn unbound_global_errors() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("definitely-not-bound", &env),
        "Error: Unbound variable: DEFINITELY-NOT-BOUND"
    );
}

#[test]
fn recursion_through_global_cell() {
    let env = env_with_stdlib();
    // The recursive self-reference resolves through the global cell each call.
    assert_eq!(
        eval_line(
            "(progn (defun fact (n) (if (zerop n) 1 (* n (fact (sub1 n))))) (fact 6))",
            &env
        ),
        "720"
    );
}
