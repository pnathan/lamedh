use lamedh::{environment::Environment, eval_line};

#[test]
fn test_function_creates_closure() {
    let env = Environment::new_with_builtins();
    let program = r#"
    (DEF MAKE-COUNTER
        (LAMBDA (N)
            (FUNCTION (LAMBDA () (SETQ N (+ N 1))))))
    "#;
    eval_line(program, &env);

    let counter_def = "(DEF COUNTER (MAKE-COUNTER 0))";
    eval_line(counter_def, &env);

    let result1 = eval_line("(COUNTER)", &env);
    assert_eq!(result1, "1");

    let result2 = eval_line("(COUNTER)", &env);
    assert_eq!(result2, "2");

    let result3 = eval_line("(COUNTER)", &env);
    assert_eq!(result3, "3");
}

#[test]
fn test_lambda_also_creates_closure() {
    let env = Environment::new_with_builtins();
    let program = r#"
    (DEF MAKE-COUNTER
        (LAMBDA (N)
            (LAMBDA () (SETQ N (+ N 1)))))
    "#;
    eval_line(program, &env);

    let counter_def = "(DEF COUNTER (MAKE-COUNTER 0))";
    eval_line(counter_def, &env);

    let result1 = eval_line("(COUNTER)", &env);
    assert_eq!(result1, "1");

    let result2 = eval_line("(COUNTER)", &env);
    assert_eq!(result2, "2");
}