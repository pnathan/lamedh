mod test_helpers;
use test_helpers::env_with_prologue;

#[test]
fn test_apply_with_builtin_functions() {
    let env = env_with_prologue();

    // (APPLY + '(1 2 3)) => 6
    let result = lamedh::eval_line("(APPLY '+ '(1 2 3))", &env);
    assert_eq!(result, "6");

    // (APPLY - '(10 5)) => 5
    let result = lamedh::eval_line("(APPLY '- '(10 5))", &env);
    assert_eq!(result, "5");

    // (APPLY CAR '((A B C))) => A
    let result = lamedh::eval_line("(APPLY 'CAR '((A B C)))", &env);
    assert_eq!(result, "A");

    // (APPLY CONS '(X (Y Z))) => (X Y Z)
    let result = lamedh::eval_line("(APPLY 'CONS '(X (Y Z)))", &env);
    assert_eq!(result, "(X Y Z)");
}

#[test]
fn test_apply_with_lambda() {
    let env = env_with_prologue();
    lamedh::eval_line("(def my-add (lambda (x y) (+ x y)))", &env);
    let result = lamedh::eval_line("(APPLY my-add '(3 4))", &env);
    assert_eq!(result, "7");

    let result = lamedh::eval_line("(APPLY (lambda (x y) (* x y)) '(5 10))", &env);
    assert_eq!(result, "50");
}

#[test]
fn test_apply_with_symbol_holding_list() {
    let env = env_with_prologue();
    lamedh::eval_line("(def my-list '(1 2 3 4))", &env);
    let result = lamedh::eval_line("(APPLY '+ my-list)", &env);
    assert_eq!(result, "10");
}

#[test]
fn test_apply_error_wrong_number_of_args() {
    let env = env_with_prologue();
    let result = lamedh::eval_line("(APPLY '+)", &env);
    assert!(result.contains("APPLY requires exactly two arguments"));
}

#[test]
fn test_apply_error_not_a_list() {
    let env = env_with_prologue();
    let result = lamedh::eval_line("(APPLY '+ 1)", &env);
    assert!(result.contains("APPLY second argument must be a proper list"));
}

#[test]
fn test_apply_error_inner_function_error() {
    let env = env_with_prologue();
    // '+' requires numbers, so applying it to a list of symbols should fail.
    let result = lamedh::eval_line("(APPLY '+ '(a b c))", &env);
    assert!(result.contains("Math functions only accept numbers"));
}
