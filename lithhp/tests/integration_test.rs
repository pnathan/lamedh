use lithhp::{environment::Environment, eval_line};

use lithhp::{evaluator, reader};

fn env_with_prologue() -> Environment {
    let mut env = Environment::new_with_builtins();
    let prologue = std::fs::read_to_string("prologue.lisp").unwrap();
    let expressions = reader::read_all(&prologue, &mut env).unwrap();
    for expr in expressions {
        evaluator::eval(&expr, &mut env).unwrap();
    }
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

#[test]
fn test_numeric_compare() {
    let mut env = env_with_prologue();
    assert_eq!(eval_line("(= 1 1)", &mut env), "t");
    assert_eq!(eval_line("(= 1 2)", &mut env), "()");
}

#[test]
fn test_cons_dotted_pair() {
    let mut env = env_with_prologue();
    let output = eval_line("(cons 'a 'b)", &mut env);
    assert_eq!(output, "(a . b)");
}

#[test]
fn test_atom() {
    let mut env = env_with_prologue();
    assert_eq!(eval_line("(atom 'a)", &mut env), "t");
    assert_eq!(eval_line("(atom 1)", &mut env), "t");
    assert_eq!(eval_line("(atom \"s\")", &mut env), "t");
    assert_eq!(eval_line("(atom '(1 2))", &mut env), "()");
    assert_eq!(eval_line("(atom (cons 1 2))", &mut env), "()");
    assert_eq!(eval_line("(atom nil)", &mut env), "t");
}

#[test]
fn test_cond() {
    let mut env = env_with_prologue();
    assert_eq!(eval_line("(cond (t 1))", &mut env), "1");
    assert_eq!(eval_line("(cond (() 1) (t 2))", &mut env), "2");
    assert_eq!(eval_line("(cond (nil 1) (t 2))", &mut env), "2");
    assert_eq!(eval_line("(cond (() 1) (() 2))", &mut env), "()");
    assert_eq!(eval_line("(cond (t))", &mut env), "t");
    assert_eq!(eval_line("(cond (1))", &mut env), "1");
    assert_eq!(eval_line("(cond (t 1 2 3))", &mut env), "3");
}

#[test]
fn test_null() {
    let mut env = env_with_prologue();
    assert_eq!(eval_line("(null nil)", &mut env), "t");
    assert_eq!(eval_line("(null '())", &mut env), "t");
    assert_eq!(eval_line("(null 1)", &mut env), "()");
    assert_eq!(eval_line("(null t)", &mut env), "()");
}

#[test]
fn test_cdr_of_dotted_list() {
    let mut env = env_with_prologue();
    let output = eval_line("(cdr (cons 1 (cons 2 3)))", &mut env);
    assert_eq!(output, "(2 . 3)");
}

#[test]
fn test_pairlis() {
    let mut env = env_with_prologue();
    // Test with equal length lists
    let output1 = eval_line("(pairlis '(a b c) '(1 2 3))", &mut env);
    assert_eq!(output1, "((a . 1) (b . 2) (c . 3))");

    // Test with keys list shorter
    let output2 = eval_line("(pairlis '(a b) '(1 2 3))", &mut env);
    assert_eq!(output2, "((a . 1) (b . 2))");

    // Test with values list shorter
    let output3 = eval_line("(pairlis '(a b c) '(1 2))", &mut env);
    assert_eq!(output3, "((a . 1) (b . 2))");

    // Test with one list empty
    let output4 = eval_line("(pairlis '() '(1 2 3))", &mut env);
    assert_eq!(output4, "()");

    // Test with both lists empty
    let output5 = eval_line("(pairlis '() '())", &mut env);
    assert_eq!(output5, "()");
}
