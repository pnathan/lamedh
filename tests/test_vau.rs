use lamedh::environment::Environment;
use lamedh::{Shared, eval_all, eval_line};

fn env() -> Shared<Environment> {
    Environment::with_stdlib()
}

fn eval(src: &str, env: &Shared<Environment>) -> String {
    eval_line(src, env)
}

fn evals(src: &str, env: &Shared<Environment>) -> String {
    match eval_all(src, env) {
        Ok(results) => results
            .last()
            .map(lamedh::printer::print)
            .unwrap_or_default(),
        Err(e) => format!("{e}"),
    }
}

#[test]
fn test_vau_returns_operand_unevaluated() {
    let e = env();
    evals("(def q (vau (x en) (car x)))", &e);
    assert_eq!(eval("(q foo)", &e), "FOO");
}

#[test]
fn test_vau_receives_unevaluated_expression() {
    let e = env();
    evals("(def q (vau (x en) (car x)))", &e);
    // The operand list contains the unevaluated form (+ 1 2)
    assert_eq!(eval("(q (+ 1 2))", &e), "(+ 1 2)");
}

#[test]
fn test_vau_can_eval_in_caller_env() {
    let e = env();
    evals("(def my-eval (vau (x en) (eval (car x) en)))", &e);
    evals("(def z 42)", &e);
    assert_eq!(eval("(my-eval z)", &e), "42");
}

#[test]
fn test_vau_env_param_is_environment() {
    let e = env();
    evals("(def f (vau (x en) (eval (car x) en)))", &e);
    evals("(def answer 99)", &e);
    assert_eq!(eval("(f answer)", &e), "99");
}

#[test]
fn test_vau_multi_form_body() {
    let e = env();
    evals("(def f (vau (x en) (+ 1 1) (+ 2 2) (+ 3 3)))", &e);
    // Multiple body forms: should evaluate all, return last
    assert_eq!(eval("(f)", &e), "6");
}

// --- issue #461: VAU must guard dynamic-variable parameter names ---
//
// If a VAU parameter name (operand-list param or env param) happens to be a
// symbol previously declared dynamic (DEFDYNAMIC/DEFVAR), a plain lexical
// `set_id` write is invisible to the body: `Environment::resolve` redirects
// every reference of a dynamic symbol straight to its global dynamic cell,
// never consulting the lexical frame slot that was just written. VAU must
// install a proper `DynamicBinding` guard instead, matching macros and
// fexprs.

#[test]
fn test_vau_operand_param_shadowing_dynamic_issue_461() {
    let e = env();
    // Declare OPS dynamic globally, matching the issue's own repro.
    evals("(defdynamic ops 'GLOBAL)", &e);
    evals("(def v (vau (ops env) ops))", &e);
    // The body should see the operand list just handed to this call, not the
    // global dynamic value of OPS.
    assert_eq!(eval("(v 1 2 3)", &e), "(1 2 3)");
    // And the dynamic binding must be restored after the call returns.
    assert_eq!(eval("ops", &e), "GLOBAL");
}

#[test]
fn test_vau_env_param_shadowing_dynamic_issue_461() {
    let e = env();
    // Declare ENV dynamic globally; use it as VAU's environment-parameter name.
    evals("(defdynamic env 'GLOBAL)", &e);
    evals("(def z 123)", &e);
    evals("(def f (vau (x env) (eval (car x) env)))", &e);
    // The body must see the caller's environment through the dynamically
    // shadowed ENV param, not the global dynamic value of ENV.
    assert_eq!(eval("(f z)", &e), "123");
    assert_eq!(eval("env", &e), "GLOBAL");
}

#[test]
fn test_vau_both_params_shadowing_dynamic_issue_461() {
    let e = env();
    evals("(defdynamic ops 'GLOBAL-OPS)", &e);
    evals("(defdynamic env 'GLOBAL-ENV)", &e);
    evals("(def z 7)", &e);
    evals(
        "(def f (vau (ops env) (cons (car ops) (list (eval z env)))))",
        &e,
    );
    assert_eq!(eval("(f z)", &e), "(Z 7)");
    assert_eq!(eval("ops", &e), "GLOBAL-OPS");
    assert_eq!(eval("env", &e), "GLOBAL-ENV");
}

#[test]
fn test_vau_non_dynamic_params_unaffected_issue_461() {
    let e = env();
    // Sanity: ordinary (non-dynamic) VAU parameter names behave exactly as
    // before the fix.
    evals("(def q (vau (x en) (car x)))", &e);
    assert_eq!(eval("(q foo)", &e), "FOO");
    evals("(def my-eval (vau (x en) (eval (car x) en)))", &e);
    evals("(def z 42)", &e);
    assert_eq!(eval("(my-eval z)", &e), "42");
}

#[test]
fn test_vau_apply_operand_param_shadowing_dynamic_issue_461() {
    let e = env();
    evals("(defdynamic ops 'GLOBAL)", &e);
    evals("(def v (vau (ops env) ops))", &e);
    // APPLY on a VAU: args are already evaluated and treated as the operand
    // list (see builtins_core.rs apply_apply). Same dynamic-guard bug applies.
    assert_eq!(eval("(apply v '(1 2 3))", &e), "(1 2 3)");
    assert_eq!(eval("ops", &e), "GLOBAL");
}

#[test]
fn test_vau_apply_env_param_shadowing_dynamic_issue_461() {
    let e = env();
    evals("(defdynamic env 'GLOBAL)", &e);
    evals("(def z 55)", &e);
    evals("(def f (vau (x env) (eval (car x) env)))", &e);
    assert_eq!(eval("(apply f '(z))", &e), "55");
    assert_eq!(eval("env", &e), "GLOBAL");
}

#[test]
fn test_vau_stored_in_variable() {
    let e = env();
    evals("(def op (vau (x en) (car x)))", &e);
    assert_eq!(eval("(op hello)", &e), "HELLO");
}

#[test]
fn test_vau_printer() {
    let e = env();
    evals("(def op (vau (x en) x))", &e);
    assert_eq!(eval("op", &e), "<vau>");
}

#[test]
fn test_vau_functionp() {
    let e = env();
    evals("(def op (vau (x en) x))", &e);
    assert_eq!(eval("(functionp op)", &e), "T");
}

#[test]
fn test_dollar_vau_alias() {
    let e = env();
    evals("(def op ($vau (x en) (car x)))", &e);
    assert_eq!(eval("(op hello)", &e), "HELLO");
}

#[test]
fn test_stdlib_dollar_if_true_branch() {
    let e = env();
    assert_eq!(eval("($if t 1 2)", &e), "1");
}

#[test]
fn test_stdlib_dollar_if_false_branch() {
    let e = env();
    assert_eq!(eval("($if nil 1 2)", &e), "2");
}

#[test]
fn test_stdlib_dollar_if_lazy_eval() {
    let e = env();
    // The true branch evaluates variable x
    evals("(def x 10)", &e);
    assert_eq!(eval("($if t x 0)", &e), "10");
}

#[test]
fn test_stdlib_dollar_sequence() {
    let e = env();
    // $sequence evaluates all forms in order and returns the last
    assert_eq!(eval("($sequence (+ 1 1) (* 3 4) (- 10 1))", &e), "9");
}

#[test]
fn test_vau_wrong_param_list_count() {
    let e = env();
    let result = eval("(vau (x) x)", &e);
    assert!(result.contains("exactly two symbols"), "got: {result}");
}

#[test]
fn test_vau_non_symbol_param() {
    let e = env();
    let result = eval("(vau (42 e) e)", &e);
    assert!(result.contains("symbol"), "got: {result}");
}

#[test]
fn test_vau_derived_my_if() {
    let e = env();
    evals(
        "(def my-if (vau (x en) (if (eval (car x) en) (eval (cadr x) en) (eval (caddr x) en))))",
        &e,
    );
    assert_eq!(eval("(my-if t (+ 1 2) (* 5 5))", &e), "3");
    assert_eq!(eval("(my-if nil (+ 1 2) (* 5 5))", &e), "25");
}

#[test]
fn test_vau_captures_closure_env() {
    let e = env();
    // Vau closes over the environment at definition time
    evals("(def x 1)", &e);
    evals("(def get-x (vau (unused en) x))", &e);
    assert_eq!(eval("(get-x)", &e), "1");
    // Rebind x
    evals("(def x 2)", &e);
    // Should see updated global x (global env is shared)
    assert_eq!(eval("(get-x)", &e), "2");
}
