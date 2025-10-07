use lamedh::{environment::Environment, eval_line, evaluator, load_directory, reader};
use std::rc::Rc;

fn env_with_stdlib() -> Rc<Environment> {
    let env = Environment::new_with_builtins();
    load_directory("lib", &env).unwrap();
    env
}

#[test]
fn test_list_functions_from_file() {
    let env = env_with_stdlib();

    // Read and evaluate the Lisp test file
    let test_lisp_code = std::fs::read_to_string("tests/list_functions_test.lisp").unwrap();
    let expressions = reader::read_all(&test_lisp_code, &env).unwrap();
    for expr in expressions {
        evaluator::eval(&expr, &env).unwrap();
    }

    // Run the test function and assert the result is T
    let result = eval_line("(test-list-functions)", &env);
    assert_eq!(result, "T");
}