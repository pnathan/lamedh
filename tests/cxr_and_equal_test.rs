use lamedh::{self, environment::Environment, eval_line};
use std::rc::Rc;

fn env_with_stdlib() -> Rc<Environment> {
    let env = Environment::new_with_builtins();
    lamedh::load_file("prologue.lisp", &env).unwrap();
    lamedh::load_directory("lib", &env).unwrap();
    env
}

#[test]
fn test_equal_atoms() {
    let env = env_with_stdlib();

    // Test with atoms
    assert_eq!(eval_line("(equal 4 4)", &env), "T");
    assert_eq!(eval_line("(equal 4 5)", &env), "()");
    assert_eq!(eval_line("(equal 'a 'a)", &env), "T");
    assert_eq!(eval_line("(equal 'a 'b)", &env), "()");
}

#[test]
fn test_equal_lists() {
    let env = env_with_stdlib();

    // Test with lists of increasing complexity
    assert_eq!(eval_line("(equal '(4) '(4))", &env), "T");
    assert_eq!(eval_line("(equal '(4) '(5))", &env), "()");
    assert_eq!(eval_line("(equal '(4 5) '(4 5))", &env), "T");
    assert_eq!(eval_line("(equal '(4 5) '(4 6))", &env), "()");
    assert_eq!(eval_line("(equal '(4 5 6) '(4 5 6))", &env), "T");
    assert_eq!(eval_line("(equal '(4 5 6) '(4 5 7))", &env), "()");
    assert_eq!(eval_line("(equal '(1 2 3 4 5) '(1 2 3 4 5))", &env), "T");
}

#[test]
fn test_equal_nested_lists() {
    let env = env_with_stdlib();

    // Test with nested lists
    assert_eq!(eval_line("(equal '((1 2) (3 4)) '((1 2) (3 4)))", &env), "T");
    assert_eq!(eval_line("(equal '((1 2) (3 4)) '((1 2) (3 5)))", &env), "()");
    assert_eq!(eval_line("(equal '(a (b c) d) '(a (b c) d))", &env), "T");
}

#[test]
fn test_cxr_2level() {
    let env = env_with_stdlib();

    // Test 2-level cxr functions
    assert_eq!(eval_line("(caar '((1 2) (3 4)))", &env), "1");
    assert_eq!(eval_line("(cadr '(1 2 3 4))", &env), "2");
    assert_eq!(eval_line("(cdar '((1 2 3) (4 5)))", &env), "(2 3)");
    assert_eq!(eval_line("(cddr '(1 2 3 4))", &env), "(3 4)");
}

#[test]
fn test_cxr_3level() {
    let env = env_with_stdlib();

    // Test 3-level cxr functions
    assert_eq!(eval_line("(caaar '(((1 2) (3 4)) ((5 6) (7 8))))", &env), "1");

    assert_eq!(eval_line("(caadr '((1 2) (3 4) (5 6)))", &env), "3");
    assert_eq!(eval_line("(cadar '(((1 2) (3 4)) ((5 6) (7 8))))", &env), "(3 4)");
    assert_eq!(eval_line("(caddr '(1 2 3 4 5))", &env), "3");
    assert_eq!(eval_line("(cdaar '(((1 2 3) (4 5)) ((6 7) (8 9))))", &env), "(2 3)");
    assert_eq!(eval_line("(cdadr '((1 2) (3 4 5) (6 7)))", &env), "(4 5)");
    assert_eq!(eval_line("(cddar '(((1 2) (3 4 5)) ((6 7) (8 9))))", &env), "()");
    assert_eq!(eval_line("(cdddr '(1 2 3 4 5 6))", &env), "(4 5 6)");
}

#[test]
fn test_cxr_4level() {
    let env = env_with_stdlib();

    // Test a few 4-level cxr functions
    assert_eq!(eval_line("(caaaar '((((1 2) (3 4)) ((5 6) (7 8))) (((9 10) (11 12)) ((13 14) (15 16)))))", &env), "1");
    assert_eq!(eval_line("(cadddr '(1 2 3 4 5))", &env), "4");
    assert_eq!(eval_line("(cddddr '(1 2 3 4 5 6 7))", &env), "(5 6 7)");
}

#[test]
fn test_recursive_factorial() {
    let env = env_with_stdlib();

    let factorial_def = r#"
    (defun factorial (n)
      (if (= n 0)
          1
          (* n (factorial (- n 1)))))
    "#;
    eval_line(factorial_def, &env);

    assert_eq!(eval_line("(factorial 0)", &env), "1");
    assert_eq!(eval_line("(factorial 1)", &env), "1");
    assert_eq!(eval_line("(factorial 5)", &env), "120");
    assert_eq!(eval_line("(factorial 10)", &env), "3628800");
}

#[test]
fn test_recursive_fibonacci() {
    let env = env_with_stdlib();

    let fib_def = r#"
    (defun fib (n)
      (if (= n 0)
          0
          (if (= n 1)
              1
              (+ (fib (- n 1)) (fib (- n 2))))))
    "#;
    eval_line(fib_def, &env);

    assert_eq!(eval_line("(fib 0)", &env), "0");
    assert_eq!(eval_line("(fib 1)", &env), "1");
    assert_eq!(eval_line("(fib 2)", &env), "1");
    assert_eq!(eval_line("(fib 3)", &env), "2");
    assert_eq!(eval_line("(fib 4)", &env), "3");
    assert_eq!(eval_line("(fib 5)", &env), "5");
    assert_eq!(eval_line("(fib 10)", &env), "55");
}

#[test]
fn test_recursive_list_length() {
    let env = env_with_stdlib();

    let length_def = r#"
    (defun length (lst)
      (if (null lst)
          0
          (+ 1 (length (cdr lst)))))
    "#;
    eval_line(length_def, &env);

    assert_eq!(eval_line("(length '())", &env), "0");
    assert_eq!(eval_line("(length '(1))", &env), "1");
    assert_eq!(eval_line("(length '(1 2 3))", &env), "3");
    assert_eq!(eval_line("(length '(a b c d e f g))", &env), "7");
}

#[test]
fn test_recursive_reverse() {
    let env = env_with_stdlib();

    let reverse_def = r#"
    (defun reverse (lst)
      (defun reverse-helper (l acc)
        (if (null l)
            acc
            (reverse-helper (cdr l) (cons (car l) acc))))
      (reverse-helper lst nil))
    "#;
    eval_line(reverse_def, &env);

    assert_eq!(eval_line("(reverse '())", &env), "()");
    assert_eq!(eval_line("(reverse '(1))", &env), "(1)");
    assert_eq!(eval_line("(reverse '(1 2 3))", &env), "(3 2 1)");
    assert_eq!(eval_line("(reverse '(a b c d))", &env), "(D C B A)");
}

#[test]
fn test_deeply_nested_recursion() {
    let env = env_with_stdlib();

    // Test deeply nested list recursion
    let sum_nested_def = r#"
    (defun sum-all (lst)
      (if (null lst)
          0
          (if (atom (car lst))
              (+ (car lst) (sum-all (cdr lst)))
              (+ (sum-all (car lst)) (sum-all (cdr lst))))))
    "#;
    eval_line(sum_nested_def, &env);

    assert_eq!(eval_line("(sum-all '())", &env), "0");
    assert_eq!(eval_line("(sum-all '(1 2 3))", &env), "6");
    assert_eq!(eval_line("(sum-all '(1 (2 3) 4))", &env), "10");
    assert_eq!(eval_line("(sum-all '((1 2) (3 4)))", &env), "10");
    assert_eq!(eval_line("(sum-all '(1 (2 (3 (4 5)))))", &env), "15");
}
