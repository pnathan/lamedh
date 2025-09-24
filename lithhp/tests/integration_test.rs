use lithhp::{LispVal, environment::Environment, eval_line, evaluator, printer, reader};

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
    let output = eval_line("(PLUS 1 2)", &mut env);
    assert_eq!(output, "3");
}

#[test]
fn test_define_and_call_function() {
    let mut env = env_with_prologue();
    eval_line("(defun square (x) (TIMES x x))", &mut env);
    let result = eval_line("(square 5)", &mut env);
    assert_eq!(result, "25");
}

#[test]
fn test_let_binding() {
    let mut env = env_with_prologue();
    let output = eval_line("(let ((x 10)) (TIMES x 2))", &mut env);
    assert_eq!(output, "20");
}

#[test]
fn test_eq() {
    let mut env = env_with_prologue();
    assert_eq!(eval_line("(EQ 1 1)", &mut env), "T");
    assert_eq!(eval_line("(EQ 1 2)", &mut env), "()");
    assert_eq!(eval_line("(EQ \"a\" \"a\")", &mut env), "T");
    assert_eq!(eval_line("(EQ \"a\" \"b\")", &mut env), "()");
    assert_eq!(eval_line("(EQ T T)", &mut env), "T");
    assert_eq!(eval_line("(EQ nil nil)", &mut env), "T");
    assert_eq!(eval_line("(EQ T nil)", &mut env), "()");
}

#[test]
fn test_logical_ops() {
    let mut env = env_with_prologue();
    assert_eq!(eval_line("(not T)", &mut env), "()");
    assert_eq!(eval_line("(not nil)", &mut env), "T");
    assert_eq!(eval_line("(and T T)", &mut env), "T");
    assert_eq!(eval_line("(and T nil)", &mut env), "()");
    assert_eq!(eval_line("(or T nil)", &mut env), "T");
    assert_eq!(eval_line("(or nil nil)", &mut env), "()");
}

#[test]
fn test_if_with_t_nil() {
    let mut env = env_with_prologue();
    assert_eq!(eval_line("(if T 1 2)", &mut env), "1");
    assert_eq!(eval_line("(if nil 1 2)", &mut env), "2");
}

#[test]
fn test_numeric_compare() {
    let mut env = env_with_prologue();
    assert_eq!(eval_line("(EQUAL-NUMBER 1 1)", &mut env), "T");
    assert_eq!(eval_line("(EQUAL-NUMBER 1 2)", &mut env), "()");
}

#[test]
fn test_cons_dotted_pair() {
    let mut env = env_with_prologue();
    let output = eval_line("(cons 'a 'b)", &mut env);
    assert_eq!(output, "(A . B)");
}

#[test]
fn test_atom() {
    let mut env = env_with_prologue();
    assert_eq!(eval_line("(atom 'a)", &mut env), "T");
    assert_eq!(eval_line("(atom 1)", &mut env), "T");
    assert_eq!(eval_line("(atom \"s\")", &mut env), "T");
    assert_eq!(eval_line("(atom '(1 2))", &mut env), "()");
    assert_eq!(eval_line("(atom (cons 1 2))", &mut env), "()");
    assert_eq!(eval_line("(atom nil)", &mut env), "T");
}

#[test]
fn test_cond() {
    let mut env = env_with_prologue();
    assert_eq!(eval_line("(cond (T 1))", &mut env), "1");
    assert_eq!(eval_line("(cond (() 1) (T 2))", &mut env), "2");
    assert_eq!(eval_line("(cond (nil 1) (T 2))", &mut env), "2");
    assert_eq!(eval_line("(cond (() 1) (() 2))", &mut env), "()");
    assert_eq!(eval_line("(cond (T))", &mut env), "T");
    assert_eq!(eval_line("(cond (1))", &mut env), "1");
    assert_eq!(eval_line("(cond (T 1 2 3))", &mut env), "3");
}

#[test]
fn test_null() {
    let mut env = env_with_prologue();
    assert_eq!(eval_line("(null nil)", &mut env), "T");
    assert_eq!(eval_line("(null '())", &mut env), "T");
    assert_eq!(eval_line("(null 1)", &mut env), "()");
    assert_eq!(eval_line("(null T)", &mut env), "()");
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
    assert_eq!(output1, "((A . 1) (B . 2) (C . 3))");

    // Test with keys list shorter
    let output2 = eval_line("(pairlis '(a b) '(1 2 3))", &mut env);
    assert_eq!(output2, "((A . 1) (B . 2))");

    // Test with values list shorter
    let output3 = eval_line("(pairlis '(a b c) '(1 2))", &mut env);
    assert_eq!(output3, "((A . 1) (B . 2))");

    // Test with one list empty
    let output4 = eval_line("(pairlis '() '(1 2 3))", &mut env);
    assert_eq!(output4, "()");

    // Test with both lists empty
    let output5 = eval_line("(pairlis '() '())", &mut env);
    assert_eq!(output5, "()");
}

#[test]
fn test_docstrings() {
    let mut env = env_with_prologue();
    let test_code = std::fs::read_to_string("tests/docstring_test.lisp").unwrap();
    let expressions = reader::read_all(&test_code, &mut env).unwrap();
    let mut result = LispVal::Nil;
    for expr in expressions {
        result = evaluator::eval(&expr, &mut env).unwrap();
    }
    assert_eq!(printer::print(&result), "T");
}

#[test]
fn test_prog_feature() {
    let mut env = env_with_prologue();
    let test_code = std::fs::read_to_string("tests/prog_test.lisp").unwrap();
    let expressions = reader::read_all(&test_code, &mut env).unwrap();
    for expr in expressions {
        evaluator::eval(&expr, &mut env).unwrap();
    }

    // (DEFUN test-prog-basic () (PROG (X Y) (SETQ X 10) (SETQ Y 20) (PLUS X Y))) -> last evaluated expr is (PLUS X Y) = 30
    // but PROG returns NIL if it falls through. The last statement is PLUS, but its result is not returned.
    // The value of a PROG that falls through is NIL.
    // Let's modify the test to return the value.
    // (DEFUN test-prog-basic () (PROG (X Y) (SETQ X 10) (SETQ Y 20) (RETURN (PLUS X Y))))
    // I will modify the test file instead.

    // After modifying test-prog-basic to use RETURN:
    // I will modify it now.
    // (DEFUN test-prog-basic () (PROG (X Y) (SETQ X 10) (SETQ Y 20) (RETURN (PLUS X Y))))
    // Let's assume this change is made. The result of test-prog-basic should be 30.
    // But since I cannot modify the file and then run the test in the same step,
    // I will write the test to expect NIL for now, and then modify the lisp file.
    // The current test `test-prog-basic` will return NIL. I will check that first.
    // And I will add another test `test-prog-basic-return` to the lisp file.

    // test-prog-basic: returns 10 + 20 = 30
    assert_eq!(eval_line("(test-prog-basic)", &mut env), "30");

    // test-prog-return: (RETURN X) where X is 100
    assert_eq!(eval_line("(test-prog-return)", &mut env), "100");

    // test-prog-go-forward: (RETURN X) where X is 101
    assert_eq!(eval_line("(test-prog-go-forward)", &mut env), "111");

    // test-prog-go-backward-loop: (RETURN SUM) where SUM is 1+2+3+4+5=15
    assert_eq!(eval_line("(test-prog-go-backward-loop)", &mut env), "15");

    // test-prog-fall-through: falls through, returns NIL
    assert_eq!(eval_line("(test-prog-fall-through)", &mut env), "()");

    // test-nested-prog: The inner prog returns 10, which is then returned by the outer prog.
    assert_eq!(eval_line("(test-nested-prog)", &mut env), "10");
}

#[test]
fn test_cxr_compositions() {
    let mut env = env_with_prologue();
    let test_code = std::fs::read_to_string("tests/cxr_test.lisp").unwrap();
    let expressions = reader::read_all(&test_code, &mut env).unwrap();
    for expr in expressions {
        evaluator::eval(&expr, &mut env).unwrap();
    }
}
