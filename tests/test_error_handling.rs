mod test_helpers;
use lamedh::eval_line;
use test_helpers::env_with_stdlib;

#[test]
fn test_errorset_success() {
    let env = env_with_stdlib();
    let output = eval_line("(errorset '(+ 1 2))", &env);
    assert_eq!(output, "(3)");
}

#[test]
fn test_errorset_catches_division_by_zero() {
    let env = env_with_stdlib();
    let output = eval_line("(errorset '(/ 1 0))", &env);
    assert_eq!(output, "()");
}

#[test]
fn test_errorset_catches_error_function() {
    let env = env_with_stdlib();
    let output = eval_line("(errorset '(error \"test error\"))", &env);
    assert_eq!(output, "()");
}

#[test]
fn test_errorset_catches_unbound_variable() {
    let env = env_with_stdlib();
    let output = eval_line("(errorset 'undefined-variable)", &env);
    assert_eq!(output, "()");
}

#[test]
fn test_errorset_nested_evaluation() {
    let env = env_with_stdlib();
    let output = eval_line("(errorset '(+ (* 2 3) (/ 10 2)))", &env);
    assert_eq!(output, "(11)");
}

#[test]
fn test_error_with_string_message() {
    let env = env_with_stdlib();
    let output = eval_line("(error \"custom error message\")", &env);
    assert!(output.contains("Error"));
}

#[test]
fn test_error_with_symbol_message() {
    let env = env_with_stdlib();
    let output = eval_line("(error 'error-symbol)", &env);
    assert!(output.contains("Error"));
}

#[test]
fn test_errorset_with_lambda() {
    let env = env_with_stdlib();
    let output = eval_line("(errorset '((lambda (x) (* x x)) 5))", &env);
    assert_eq!(output, "(25)");
}

#[test]
fn test_errorset_preserves_nil() {
    let env = env_with_stdlib();
    let output = eval_line("(errorset 'nil)", &env);
    assert_eq!(output, "(())");
}

#[test]
fn test_multiple_errorsets() {
    let env = env_with_stdlib();
    eval_line("(def result1 (errorset '(+ 1 1)))", &env);
    eval_line("(def result2 (errorset '(/ 1 0)))", &env);
    assert_eq!(eval_line("result1", &env), "(2)");
    assert_eq!(eval_line("result2", &env), "()");
}
