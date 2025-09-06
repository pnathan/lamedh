use lithhp::{environment::Environment, eval_line};

fn env_with_prologue() -> Environment {
    let mut env = Environment::new_with_builtins();
    let prologue = "(defmacro defun (name params body) `(def ,name (lambda ,params ,body)))";
    eval_line(prologue, &mut env);
    env
}

#[test]
fn test_add_two_numbers() {
    let mut env = env_with_prologue();
    let output = eval_line("(+ 1 2)", &mut env);
    assert_eq!(output, "3");
}

#[test]
fn test_define_and_call_function() {
    let mut env = env_with_prologue();
    eval_line("(defun square (x) (* x x))", &mut env);
    let result = eval_line("(square 5)", &mut env);
    assert_eq!(result, "25");
}

#[test]
fn test_let_binding() {
    let mut env = env_with_prologue();
    let output = eval_line("(let ((x 10)) (* x 2))", &mut env);
    assert_eq!(output, "20");
}

#[test]
fn test_eq() {
    let mut env = env_with_prologue();
    assert_eq!(eval_line("(eq 1 1)", &mut env), "t");
    assert_eq!(eval_line("(eq 1 2)", &mut env), "()");
    assert_eq!(eval_line("(eq \"a\" \"a\")", &mut env), "t");
    assert_eq!(eval_line("(eq \"a\" \"b\")", &mut env), "()");
    assert_eq!(eval_line("(eq t t)", &mut env), "t");
    assert_eq!(eval_line("(eq nil nil)", &mut env), "t");
    assert_eq!(eval_line("(eq t nil)", &mut env), "()");
}

#[test]
fn test_logical_ops() {
    let mut env = env_with_prologue();
    assert_eq!(eval_line("(not t)", &mut env), "()");
    assert_eq!(eval_line("(not nil)", &mut env), "t");
    assert_eq!(eval_line("(and t t)", &mut env), "t");
    assert_eq!(eval_line("(and t nil)", &mut env), "()");
    assert_eq!(eval_line("(or t nil)", &mut env), "t");
    assert_eq!(eval_line("(or nil nil)", &mut env), "()");
}

#[test]
fn test_if_with_t_nil() {
    let mut env = env_with_prologue();
    assert_eq!(eval_line("(if t 1 2)", &mut env), "1");
    assert_eq!(eval_line("(if nil 1 2)", &mut env), "2");
}
