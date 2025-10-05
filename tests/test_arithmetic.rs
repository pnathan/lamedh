mod test_helpers;
use lamedh::eval_line;
use test_helpers::env_with_stdlib;

#[test]
fn test_add_two_numbers() {
    let env = env_with_stdlib();
    let output = eval_line("(PLUS 1 2)", &env);
    assert_eq!(output, "3");
}

#[test]
fn test_numeric_compare() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(EQUAL-NUMBER 1 1)", &env), "T");
    assert_eq!(eval_line("(EQUAL-NUMBER 1 2)", &env), "()");
}