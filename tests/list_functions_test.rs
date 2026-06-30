use lamedh::{Shared, environment::Environment, eval_line, evaluator, reader};

fn env_with_prologue() -> Shared<Environment> {
    let env = Environment::new_with_builtins();
    lamedh::load_directory("lib", &env).unwrap();
    let prologue = std::fs::read_to_string("prologue.lisp").unwrap();
    let expressions = reader::read_all(&prologue, &env).unwrap();
    for expr in expressions {
        evaluator::eval(&expr, &env).unwrap();
    }
    env
}

#[test]
fn test_list_functions_from_file() {
    let env = env_with_prologue();

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
