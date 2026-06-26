//! End-to-end tests for the introspection builtins: `describe`, `see-source`,
//! and `disassemble`, exercised through the public `eval_line` REPL entry point.

use lamedh::environment::Environment;
use lamedh::eval_line;

#[test]
fn see_source_returns_reconstructed_lambda() {
    let env = Environment::with_stdlib();
    eval_line("(defun cube (x) (* x (* x x)))", &env);
    // Without the tree flag, see-source returns the form itself.
    assert_eq!(
        eval_line("(see-source 'cube)", &env),
        "(LAMBDA (X) (* X (* X X)))"
    );
}

#[test]
fn see_source_handles_macros_and_fexprs() {
    let env = Environment::with_stdlib();
    eval_line("(defmacro twice (x) (list 'progn x x))", &env);
    assert_eq!(
        eval_line("(see-source 'twice)", &env),
        "(MACRO (X) (LIST (QUOTE PROGN) X X))"
    );
}

#[test]
fn see_source_tree_mode_returns_t() {
    let env = Environment::with_stdlib();
    eval_line("(defun f (n) (if (< n 2) n (f (- n 1))))", &env);
    // Tree mode prints to stdout and returns T.
    assert_eq!(eval_line("(see-source 'f t)", &env), "T");
}

#[test]
fn see_source_on_builtin_errors() {
    let env = Environment::with_stdlib();
    let out = eval_line("(see-source 'car)", &env);
    assert!(
        out.contains("rror") || out.contains("no inspectable source"),
        "got: {out}"
    );
}

#[test]
fn describe_returns_t() {
    let env = Environment::with_stdlib();
    eval_line("(defun g (a b) \"adds\" (+ a b))", &env);
    assert_eq!(eval_line("(describe 'g)", &env), "T");
    assert_eq!(eval_line("(describe '+)", &env), "T");
    assert_eq!(eval_line("(describe 'totally-unbound)", &env), "T");
}

#[test]
fn disassemble_typed_and_untyped() {
    let env = Environment::with_stdlib();
    eval_line(
        "(deffun-typed (fact int64) ((n int64)) (if (<= n 1) 1 (* n (fact (- n 1)))))",
        &env,
    );
    // Both the typed case and the "no typed edition" case return T.
    assert_eq!(eval_line("(disassemble 'fact)", &env), "T");
    assert_eq!(eval_line("(disassemble 'car)", &env), "T");
}
