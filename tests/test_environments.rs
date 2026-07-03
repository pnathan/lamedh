mod test_helpers;
use lamedh::eval_line;
use test_helpers::env_with_stdlib;

#[test]
fn test_the_environment_returns_env() {
    let env = env_with_stdlib();
    let output = eval_line("(the-environment)", &env);
    assert_eq!(output, "<environment>");
}

#[test]
fn test_eval_with_env() {
    let env = env_with_stdlib();
    let output = eval_line("(eval '(+ 1 2) (the-environment))", &env);
    assert_eq!(output, "3");
}

#[test]
fn test_eval_bindings_visible() {
    let env = env_with_stdlib();
    // Define a variable, then eval a reference to it using the-environment
    eval_line("(setq my-secret-val 42)", &env);
    let output = eval_line("(eval 'my-secret-val (the-environment))", &env);
    assert_eq!(output, "42");
}

#[test]
fn test_make_environment_default() {
    let env = env_with_stdlib();
    let output = eval_line("(make-environment)", &env);
    assert_eq!(output, "<environment>");
}

#[test]
fn test_make_environment_with_parent() {
    let env = env_with_stdlib();
    let output = eval_line("(make-environment (the-environment))", &env);
    assert_eq!(output, "<environment>");
}

#[test]
fn test_environment_is_atom() {
    // An environment is not a cons cell, so (atom ...) should return T
    let env = env_with_stdlib();
    let output = eval_line("(atom (the-environment))", &env);
    assert_eq!(output, "T");
}

#[test]
fn test_eval_with_fresh_env_has_builtins() {
    // A fresh environment from make-environment should still have builtins
    let env = env_with_stdlib();
    let output = eval_line("(eval '(+ 10 5) (make-environment))", &env);
    assert_eq!(output, "15");
}

#[test]
fn test_eval_with_fresh_env_does_not_see_caller_symbol_value() {
    let env = env_with_stdlib();
    let output = eval_line(
        "(progn (def isolated-secret 1234) (eval 'isolated-secret (make-environment)))",
        &env,
    );
    assert!(
        output.starts_with("Error:"),
        "fresh eval environment leaked caller binding: {output}"
    );
}

#[test]
fn test_eval_with_child_env_inherits_parent() {
    let env = env_with_stdlib();
    // Define a variable in current env
    eval_line("(setq inherited-val 99)", &env);
    // A child of the-environment should see that binding
    let output = eval_line(
        "(eval 'inherited-val (make-environment (the-environment)))",
        &env,
    );
    assert_eq!(output, "99");
}

#[test]
fn test_environment_eq_same_object() {
    // Two captures of the-environment in the same scope should be equal (same Rc)
    let env = env_with_stdlib();
    let output = eval_line("(eq (the-environment) (the-environment))", &env);
    // Each call creates a fresh capture of the current env Rc — they are ptr-equal
    // since the-environment returns Rc::clone of the same underlying env
    assert_eq!(output, "T");
}

#[test]
fn test_eval_one_arg_still_works() {
    // Ensure the original single-argument eval still works
    let env = env_with_stdlib();
    let output = eval_line("(eval '(* 3 4))", &env);
    assert_eq!(output, "12");
}

#[test]
fn test_cross_namespace_lambda_params_issue_223() {
    let env = env_with_stdlib();
    // Lambda created inside a foreign (make-environment) should bind params
    // under that table's ids and resolve them correctly at call time.
    let output = eval_line(
        "(let ((fresh (make-environment)) (f nil)) \
           (eval '(setq y 999) fresh) \
           (setq f (eval '(lambda (y) y) fresh)) \
           (funcall f 42))",
        &env,
    );
    assert_eq!(output, "42", "param should shadow global, got: {output}");

    // Without a pre-set global, should also work (not raise unbound).
    let output2 = eval_line(
        "(let ((fresh (make-environment)) (f nil)) \
           (setq f (eval '(lambda (y) y) fresh)) \
           (funcall f 7))",
        &env,
    );
    assert_eq!(output2, "7", "param should bind, got: {output2}");

    // Multi-parameter cross-namespace lambda.
    let output3 = eval_line(
        "(let ((fresh (make-environment)) (f nil)) \
           (setq f (eval '(lambda (a b) (+ a b)) fresh)) \
           (funcall f 10 20))",
        &env,
    );
    assert_eq!(output3, "30", "multi-param cross-ns, got: {output3}");
}

#[test]
fn test_make_environment_bad_arg_errors() {
    let env = env_with_stdlib();
    let output = eval_line("(make-environment 42)", &env);
    assert!(
        output.starts_with("Error:"),
        "Expected an error, got: {output}"
    );
}
