mod test_helpers;
use lamedh::{LispVal, eval_line, evaluator, printer, reader};
use test_helpers::env_with_stdlib;

#[test]
fn test_define_and_call_function() {
    let env = env_with_stdlib();
    eval_line("(defun square (x) (TIMES x x))", &env);
    let result = eval_line("(square 5)", &env);
    assert_eq!(result, "25");
}

#[test]
fn test_let_binding() {
    let env = env_with_stdlib();
    let output = eval_line("(let ((x 10)) (TIMES x 2))", &env);
    assert_eq!(output, "20");
}

#[test]
fn test_docstrings() {
    let env = env_with_stdlib();
    let test_code = std::fs::read_to_string("tests/docstring_test.lisp").unwrap();
    let expressions = reader::read_all(&test_code, &env).unwrap();
    let mut result = LispVal::Nil;
    for expr in expressions {
        result = evaluator::eval(&expr, &env).unwrap();
    }
    assert_eq!(printer::print(&result), "T");
}
