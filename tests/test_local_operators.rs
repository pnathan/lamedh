//! Local operator bindings: flet / macrolet / fexprlet / vaulet (Lisp-layer
//! sugar over the anonymous LAMBDA / MACRO / FEXPR / VAU constructors).
//!
//! These rely on the fact that operator dispatch resolves the head symbol
//! through the ordinary lexical environment chain, so a locally-bound
//! function/macro/fexpr/vau value is used as an operator inside its scope.

mod test_helpers;
use lamedh::eval_line;
use test_helpers::env_with_stdlib;

#[test]
fn flet_binds_a_local_function() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(flet ((sq (x) (* x x))) (sq 7))", &env), "49");
}

#[test]
fn flet_local_shadows_only_within_scope() {
    let env = env_with_stdlib();
    eval_line("(defun f (x) (+ x 100))", &env);
    // Inside the flet, f is the local; outside, the global is restored.
    assert_eq!(eval_line("(flet ((f (x) (* x 2))) (f 10))", &env), "20");
    assert_eq!(eval_line("(f 10)", &env), "110");
}

#[test]
fn flet_supports_multiple_bindings() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line(
            "(flet ((dbl (x) (* x 2)) (inc (x) (+ x 1))) (+ (dbl 5) (inc 5)))",
            &env
        ),
        "16"
    );
}

#[test]
fn macrolet_expands_a_local_macro() {
    let env = env_with_stdlib();
    // (twice e) -> (progn e e). Use a counter to observe two evaluations.
    eval_line("(def counter 0)", &env);
    let out = eval_line(
        "(macrolet ((twice (e) (list 'progn e e))) (twice (setq counter (+ counter 1))))",
        &env,
    );
    assert_eq!(out, "2");
    assert_eq!(eval_line("counter", &env), "2");
}

#[test]
fn fexprlet_receives_unevaluated_operands() {
    let env = env_with_stdlib();
    // The fexpr gets the unevaluated operand list; (car a) is the raw form.
    assert_eq!(
        eval_line("(fexprlet ((q (a) (car a))) (q (+ 1 2)))", &env),
        "(+ 1 2)"
    );
}

#[test]
fn vaulet_is_a_local_operative() {
    let env = env_with_stdlib();
    // A local `my-if` that evaluates operands in the caller env on demand.
    let prog = "(vaulet ((my-if (ops e) \
                   (if (eval (car ops) e) \
                       (eval (car (cdr ops)) e) \
                       (eval (car (cdr (cdr ops))) e)))) \
                 (my-if t 'yes 'no))";
    assert_eq!(eval_line(prog, &env), "YES");
}

#[test]
fn anonymous_macro_constructor_yields_a_macro_value() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("(let ((m (macro (x) (list '* x x)))) (m 6))", &env),
        "36"
    );
}

#[test]
fn anonymous_fexpr_constructor_yields_a_fexpr_value() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("(let ((q (fexpr (a) (car a)))) (q (+ 1 2)))", &env),
        "(+ 1 2)"
    );
}
