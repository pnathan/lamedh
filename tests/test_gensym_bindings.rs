//! Regression tests for issue #285: a GENSYM-created symbol must work as a
//! binder everywhere an interned symbol does. The historical bug: binding
//! sites (lambda/fexpr/macro parameters, SETQ) computed their frame key by
//! re-interning the symbol's *name*, which for a gensym — deliberately
//! absent from the name map — minted a fresh twin with a different id, so
//! the body's occurrence of the real gensym resolved to nothing.

use lamedh::environment::Environment;
use lamedh::eval_line;

fn e() -> lamedh::Shared<Environment> {
    Environment::new_with_builtins()
}

#[test]
fn gensym_as_lambda_parameter() {
    let env = e();
    // The original #285 repro.
    let out = eval_line(
        "(progn (setq g (gensym))
                (eval (list (list 'lambda (list g) (list '+ g 1)) 41)))",
        &env,
    );
    assert_eq!(out, "42");
}

#[test]
fn gensym_parameter_captured_by_nested_closure() {
    let env = e();
    let out = eval_line(
        "(progn (setq g (gensym))
                (setq f (eval (list 'lambda (list g) (list 'lambda '(n) (list '+ 'n g)))))
                (funcall (funcall f 40) 2))",
        &env,
    );
    assert_eq!(out, "42");
}

#[test]
fn gensym_as_rest_parameter() {
    let env = e();
    let out = eval_line(
        "(progn (setq g (gensym))
                (funcall (eval (list 'lambda (list 'a '&rest g) g)) 1 2 3))",
        &env,
    );
    assert_eq!(out, "(2 3)");
}

#[test]
fn setq_on_gensym_at_top_level() {
    let env = e();
    let out = eval_line(
        "(progn (setq g (gensym)) (eval (list 'progn (list 'setq g 9) g)))",
        &env,
    );
    assert_eq!(out, "9");
}

#[test]
fn setq_on_gensym_inside_prog() {
    let env = e();
    let out = eval_line(
        "(progn (setq g (gensym))
                (eval (list 'prog (list g) (list 'setq g 7) (list 'return g))))",
        &env,
    );
    assert_eq!(out, "7");
}

#[test]
fn setq_on_gensym_inside_compiled_lambda_body() {
    let env = e();
    // Lambda bodies are pre-compiled at definition time (issue #233), so
    // this exercises the Code::SetVar path, not the tree-walker's SETQ.
    let out = eval_line(
        "(progn (setq g (gensym))
                (setq f (eval (list 'lambda (list g)
                                    (list 'setq g (list '* g 2))
                                    g)))
                (funcall f 21))",
        &env,
    );
    assert_eq!(out, "42");
}

#[test]
fn gensym_as_macro_and_fexpr_parameter() {
    let env = e();
    let out = eval_line(
        "(progn (setq g (gensym))
                (setq fx (eval (list 'fexpr (list g) (list 'car g))))
                (fx (+ 1 2)))",
        &env,
    );
    // A fexpr receives its operands unevaluated: car of ((+ 1 2)).
    assert_eq!(out, "(+ 1 2)");
}

#[test]
fn two_gensyms_with_colliding_names_stay_distinct() {
    let env = e();
    // Interning "G0000" by name after a gensym G0000 exists must not
    // capture or be captured by the gensym's bindings.
    let out = eval_line(
        "(progn (setq g (gensym))
                (setq g0000 1)
                (funcall (eval (list 'lambda (list g) (list 'list g 'g0000))) 2))",
        &env,
    );
    assert_eq!(out, "(2 1)");
}

#[test]
fn interned_symbols_unaffected() {
    let env = e();
    assert_eq!(eval_line("((lambda (x) (+ x 1)) 41)", &env), "42");
    assert_eq!(
        eval_line("(progn (setq y 1) (setq y (+ y 1)) y)", &env),
        "2"
    );
}
