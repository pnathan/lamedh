/// Tests for FUNCTION special form, DEFINE special form, apply_apply with Macro/Fexpr,
/// defexpr arg-list behavior, and LABEL happy path.
mod test_helpers;
use lamedh::eval_line;
use test_helpers::env_with_stdlib;

// ============================================================================
// FUNCTION special form
// ============================================================================

/// FUNCTION with a single-body lambda is the basic case — should return a lambda.
#[test]
fn test_function_single_body_lambda() {
    let env = env_with_stdlib();
    let result = eval_line("(function (lambda (x) (+ x 1)))", &env);
    assert!(
        !result.contains("Error"),
        "expected non-error for (function (lambda (x) (+ x 1))), got: {result}"
    );
    // The printer renders lambdas as <lambda>
    assert_eq!(result, "<lambda>", "expected a lambda value, got: {result}");
}

/// FUNCTION with a multi-body lambda wraps the bodies in PROGN and returns a lambda.
/// The last body expression is what gets evaluated, so calling it returns (* x 2).
#[test]
fn test_function_multi_body_lambda_returns_last() {
    let env = env_with_stdlib();
    // The function has two body forms: (+ x 1) and (* x 2).
    // When called with x=5, it should return (* 5 2) = 10.
    let result = eval_line(
        "((function (lambda (x) (+ x 1) (* x 2))) 5)",
        &env,
    );
    assert!(
        !result.contains("Error"),
        "expected non-error calling multi-body lambda, got: {result}"
    );
    assert_eq!(result, "10", "multi-body lambda should return last body result");
}

/// FUNCTION with a multi-body lambda yields a callable lambda value.
#[test]
fn test_function_multi_body_lambda_is_lambda() {
    let env = env_with_stdlib();
    let result = eval_line("(function (lambda (x) (+ x 1) (* x 2)))", &env);
    assert!(
        !result.contains("Error"),
        "expected non-error for multi-body FUNCTION, got: {result}"
    );
    assert_eq!(result, "<lambda>", "expected <lambda>, got: {result}");
}

/// FUNCTION with an undefined symbol name produces an "Undefined function" error.
#[test]
fn test_function_undefined_symbol_error() {
    let env = env_with_stdlib();
    let result = eval_line("(function foo)", &env);
    assert!(
        result.contains("Error") || result.contains("error"),
        "expected error for (function foo), got: {result}"
    );
    assert!(
        result.contains("Undefined function") || result.contains("FOO"),
        "error should mention undefined function or FOO, got: {result}"
    );
}

/// FUNCTION with a symbol bound to a non-function value (a number) produces an error.
#[test]
fn test_function_symbol_bound_to_non_function_error() {
    let env = env_with_stdlib();
    let result = eval_line("(progn (def n 42) (function n))", &env);
    assert!(
        result.contains("Error") || result.contains("error"),
        "expected error when symbol is bound to a number, got: {result}"
    );
    assert!(
        result.contains("not bound to a function") || result.contains("N"),
        "error message should mention not a function or N, got: {result}"
    );
}

/// FUNCTION with a literal number (not a symbol, not a lambda) produces an error.
#[test]
fn test_function_literal_number_error() {
    let env = env_with_stdlib();
    let result = eval_line("(function 42)", &env);
    assert!(
        result.contains("Error") || result.contains("error"),
        "expected error for (function 42), got: {result}"
    );
}

/// FUNCTION with a symbol bound to a lambda returns that lambda.
#[test]
fn test_function_symbol_bound_to_lambda() {
    let env = env_with_stdlib();
    // def stores unevaluated, but lambda creates an actual Lambda value.
    // We use defun (via stdlib) which properly stores a Lambda.
    let result = eval_line(
        "(progn (defun sq (x) (* x x)) (function sq))",
        &env,
    );
    assert!(
        !result.contains("Error"),
        "expected non-error when function symbol is bound to a lambda, got: {result}"
    );
    assert_eq!(result, "<lambda>", "expected <lambda>, got: {result}");
}

/// FUNCTION with a symbol bound to a builtin returns that builtin.
#[test]
fn test_function_symbol_bound_to_builtin() {
    let env = env_with_stdlib();
    // + is a builtin; (function +) should return the builtin
    let result = eval_line("(function +)", &env);
    assert!(
        !result.contains("Error"),
        "expected non-error for (function +), got: {result}"
    );
}

// ============================================================================
// DEFINE special form (Lisp 1.5 style)
// ============================================================================

/// DEFINE with a single (name lambda) pair stores the lambda unevaluated and returns (NAME).
/// Note: DEFINE stores the value raw without evaluating, so calling the defined name
/// directly won't work (it's stored as a cons cell). The return value is the list of names.
#[test]
fn test_define_single_pair_returns_name_list() {
    let env = env_with_stdlib();
    let result = eval_line(
        "(define ((foo (lambda (x) (+ x 1)))))",
        &env,
    );
    assert!(
        !result.contains("Error"),
        "expected non-error for DEFINE, got: {result}"
    );
    assert_eq!(result, "(FOO)", "expected (FOO) as the list of defined names");
}

/// DEFINE with multiple (name value) pairs returns all names.
#[test]
fn test_define_multiple_pairs_returns_all_names() {
    let env = env_with_stdlib();
    let result = eval_line(
        "(define ((bar (lambda (x) x)) (baz (lambda (y) y))))",
        &env,
    );
    assert!(
        !result.contains("Error"),
        "expected non-error for multi-pair DEFINE, got: {result}"
    );
    assert_eq!(result, "(BAR BAZ)", "expected (BAR BAZ), got: {result}");
}

/// DEFINE with an empty definition list returns NIL.
#[test]
fn test_define_empty_list_returns_nil() {
    let env = env_with_stdlib();
    let result = eval_line("(define ())", &env);
    assert!(
        !result.contains("Error"),
        "expected non-error for (define ()), got: {result}"
    );
    assert_eq!(result, "()", "expected NIL for empty define list");
}

/// DEFINE with a non-symbol as the definition name produces an error.
#[test]
fn test_define_non_symbol_name_error() {
    let env = env_with_stdlib();
    let result = eval_line("(define ((1 42)))", &env);
    assert!(
        result.contains("Error") || result.contains("error"),
        "expected error when definition name is not a symbol, got: {result}"
    );
}

/// DEFINE with an inner pair that has wrong arity (empty pair) produces an error.
#[test]
fn test_define_empty_inner_pair_error() {
    let env = env_with_stdlib();
    // (define (())) — the inner list is empty, so def_pair.len() == 0, not 2
    let result = eval_line("(define (()))", &env);
    assert!(
        result.contains("Error") || result.contains("error"),
        "expected error for (define (())), got: {result}"
    );
}

/// DEFINE with wrong outer arity (two outer args instead of one list) produces an error.
#[test]
fn test_define_wrong_outer_arity_error() {
    let env = env_with_stdlib();
    // (define foo 42) — passes two args instead of one definition list
    let result = eval_line("(define foo 42)", &env);
    assert!(
        result.contains("Error") || result.contains("error"),
        "expected error for (define foo 42), got: {result}"
    );
}

// ============================================================================
// apply_apply with Macro
// ============================================================================

/// APPLY on a macro: the macro is expanded with the provided args, then the expansion
/// is evaluated. (defmacro double-it (x) (list '+ x x)) applied to (21) should give 42.
#[test]
fn test_apply_macro_expands_and_evals() {
    let env = env_with_stdlib();
    let result = eval_line(
        "(progn (defmacro double-it (x) (list '+ x x)) (apply 'double-it '(21)))",
        &env,
    );
    assert!(
        !result.contains("Error"),
        "expected non-error for apply with macro, got: {result}"
    );
    assert_eq!(result, "42", "apply on double-it macro with 21 should give 42");
}

// ============================================================================
// apply_apply with Fexpr
// ============================================================================

/// APPLY on a fexpr: the fexpr's single param receives the list of args unevaluated.
/// (defexpr myexpr (args) (car args)) applied to '(hello world) should return HELLO.
#[test]
fn test_apply_fexpr_receives_arg_list() {
    let env = env_with_stdlib();
    let result = eval_line(
        "(progn (defexpr myexpr (args) (car args)) (apply 'myexpr '(hello world)))",
        &env,
    );
    assert!(
        !result.contains("Error"),
        "expected non-error for apply with fexpr, got: {result}"
    );
    assert_eq!(result, "HELLO", "fexpr's (car args) should return first arg HELLO");
}

// ============================================================================
// defexpr: args list is a proper list of all passed arguments
// ============================================================================

/// When a fexpr is called, its param receives a proper list of all arguments (unevaluated).
/// (defexpr f (args) (length args)) called with 4 args should return 4.
#[test]
fn test_defexpr_args_is_proper_list_with_all_args() {
    let env = env_with_stdlib();
    let result = eval_line(
        "(progn (defexpr f (args) (length args)) (f a b c d))",
        &env,
    );
    assert!(
        !result.contains("Error"),
        "expected non-error for defexpr with 4 args, got: {result}"
    );
    assert_eq!(result, "4", "fexpr should see 4 arguments in args list");
}

/// Fexpr with zero args — args is empty, length should be 0.
#[test]
fn test_defexpr_args_empty_when_no_args() {
    let env = env_with_stdlib();
    let result = eval_line(
        "(progn (defexpr f (args) (length args)) (f))",
        &env,
    );
    assert!(
        !result.contains("Error"),
        "expected non-error for defexpr with 0 args, got: {result}"
    );
    assert_eq!(result, "0", "fexpr should see 0 arguments in args list");
}

// ============================================================================
// LABEL happy path
// ============================================================================

/// LABEL creates a locally-named lambda and calling it with an argument works.
/// ((label my-fn (lambda (x) (* x x))) 5) should return 25.
#[test]
fn test_label_creates_callable_lambda() {
    let env = env_with_stdlib();
    let result = eval_line("((label my-fn (lambda (x) (* x x))) 5)", &env);
    assert!(
        !result.contains("Error"),
        "expected non-error for LABEL lambda call, got: {result}"
    );
    assert_eq!(result, "25", "label my-fn should compute 5*5 = 25");
}

/// LABEL is often used for self-referential (recursive) functions.
/// A recursive factorial via LABEL should work correctly.
#[test]
fn test_label_recursive_factorial() {
    let env = env_with_stdlib();
    let result = eval_line(
        "((label fact (lambda (n) (if (zerop n) 1 (* n (fact (- n 1)))))) 5)",
        &env,
    );
    assert!(
        !result.contains("Error"),
        "expected non-error for LABEL recursive factorial, got: {result}"
    );
    assert_eq!(result, "120", "fact(5) should be 120");
}
