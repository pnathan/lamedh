mod test_helpers;
use lamedh::eval_line;
use test_helpers::env_with_stdlib;

// FIXP tests
#[test]
fn test_fixp_with_integer() {
    let env = env_with_stdlib();
    let output = eval_line("(fixp 42)", &env);
    assert_eq!(output, "T");
}

#[test]
fn test_fixp_with_zero() {
    let env = env_with_stdlib();
    let output = eval_line("(fixp 0)", &env);
    assert_eq!(output, "T");
}

#[test]
fn test_fixp_with_negative() {
    let env = env_with_stdlib();
    let output = eval_line("(fixp -123)", &env);
    assert_eq!(output, "T");
}

#[test]
fn test_fixp_with_large_integer() {
    let env = env_with_stdlib();
    let output = eval_line("(fixp 999999999)", &env);
    assert_eq!(output, "T");
}

#[test]
fn test_fixp_with_float() {
    let env = env_with_stdlib();
    let output = eval_line("(fixp 3.14)", &env);
    assert_eq!(output, "()");
}

#[test]
fn test_fixp_with_string() {
    let env = env_with_stdlib();
    let output = eval_line("(fixp \"hello\")", &env);
    assert_eq!(output, "()");
}

#[test]
fn test_fixp_with_symbol() {
    let env = env_with_stdlib();
    let output = eval_line("(fixp 'symbol)", &env);
    assert_eq!(output, "()");
}

#[test]
fn test_fixp_with_list() {
    let env = env_with_stdlib();
    let output = eval_line("(fixp '(1 2 3))", &env);
    assert_eq!(output, "()");
}

#[test]
fn test_fixp_with_nil() {
    let env = env_with_stdlib();
    let output = eval_line("(fixp nil)", &env);
    assert_eq!(output, "()");
}

#[test]
fn test_fixp_with_expression_result() {
    let env = env_with_stdlib();
    let output = eval_line("(fixp (+ 1 2))", &env);
    assert_eq!(output, "T");
}

// FLOATP tests
#[test]
fn test_floatp_with_float() {
    let env = env_with_stdlib();
    let output = eval_line("(floatp 3.14)", &env);
    assert_eq!(output, "T");
}

#[test]
fn test_floatp_with_zero_float() {
    let env = env_with_stdlib();
    let output = eval_line("(floatp 0.0)", &env);
    assert_eq!(output, "T");
}

#[test]
fn test_floatp_with_negative_float() {
    let env = env_with_stdlib();
    let output = eval_line("(floatp -2.5)", &env);
    assert_eq!(output, "T");
}

#[test]
fn test_floatp_with_integer() {
    let env = env_with_stdlib();
    let output = eval_line("(floatp 42)", &env);
    assert_eq!(output, "()");
}

#[test]
fn test_floatp_with_string() {
    let env = env_with_stdlib();
    let output = eval_line("(floatp \"3.14\")", &env);
    assert_eq!(output, "()");
}

#[test]
fn test_floatp_with_symbol() {
    let env = env_with_stdlib();
    let output = eval_line("(floatp 'pi)", &env);
    assert_eq!(output, "()");
}

#[test]
fn test_floatp_with_list() {
    let env = env_with_stdlib();
    let output = eval_line("(floatp '(1.5 2.5))", &env);
    assert_eq!(output, "()");
}

#[test]
fn test_floatp_with_nil() {
    let env = env_with_stdlib();
    let output = eval_line("(floatp nil)", &env);
    assert_eq!(output, "()");
}

// NUMBERP tests (verify it works with both)
#[test]
fn test_numberp_with_integer() {
    let env = env_with_stdlib();
    let output = eval_line("(numberp 42)", &env);
    assert_eq!(output, "T");
}

#[test]
fn test_numberp_with_float() {
    let env = env_with_stdlib();
    let output = eval_line("(numberp 3.14)", &env);
    assert_eq!(output, "T");
}

#[test]
fn test_numberp_with_string() {
    let env = env_with_stdlib();
    let output = eval_line("(numberp \"123\")", &env);
    assert_eq!(output, "()");
}

#[test]
fn test_numberp_with_zero() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(numberp 0)", &env), "T");
    assert_eq!(eval_line("(numberp 0.0)", &env), "T");
}

#[test]
fn test_numberp_with_negative() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(numberp -5)", &env), "T");
    assert_eq!(eval_line("(numberp -5.5)", &env), "T");
}

// Combined type checking
#[test]
fn test_type_checking_combination() {
    let env = env_with_stdlib();
    eval_line("(def x 42)", &env);
    eval_line("(def y 3.14)", &env);

    assert_eq!(eval_line("(fixp x)", &env), "T");
    assert_eq!(eval_line("(floatp x)", &env), "()");
    assert_eq!(eval_line("(numberp x)", &env), "T");

    assert_eq!(eval_line("(fixp y)", &env), "()");
    assert_eq!(eval_line("(floatp y)", &env), "T");
    assert_eq!(eval_line("(numberp y)", &env), "T");
}

#[test]
fn test_type_predicate_in_conditional() {
    let env = env_with_stdlib();
    let output = eval_line("(if (fixp 42) 'integer 'not-integer)", &env);
    assert_eq!(output, "INTEGER");

    let output = eval_line("(if (floatp 42) 'float 'not-float)", &env);
    assert_eq!(output, "NOT-FLOAT");
}

#[test]
fn test_type_predicate_with_mapcar() {
    let env = env_with_stdlib();
    let output = eval_line("(mapcar fixp '(1 2.5 3 4.0))", &env);
    assert_eq!(output, "(T () T ())");

    let output = eval_line("(mapcar floatp '(1 2.5 3 4.0))", &env);
    assert_eq!(output, "(() T () T)");
}

#[test]
fn test_type_predicate_filter() {
    let env = env_with_stdlib();
    // Define a simple filter function
    eval_line(
        "(defun filter-fixp (lst) (if (null lst) nil (if (fixp (car lst)) (cons (car lst) (filter-fixp (cdr lst))) (filter-fixp (cdr lst)))))",
        &env,
    );

    let output = eval_line("(filter-fixp '(1 2.5 3 4.0 5))", &env);
    assert_eq!(output, "(1 3 5)");
}

#[test]
fn test_all_type_predicates_together() {
    let env = env_with_stdlib();
    eval_line("(def test-val 100)", &env);

    assert_eq!(eval_line("(atom test-val)", &env), "T");
    assert_eq!(eval_line("(numberp test-val)", &env), "T");
    assert_eq!(eval_line("(fixp test-val)", &env), "T");
    assert_eq!(eval_line("(floatp test-val)", &env), "()");
    assert_eq!(eval_line("(stringp test-val)", &env), "()");
}

#[test]
fn test_type_predicates_on_arithmetic_results() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(fixp (+ 1 2))", &env), "T");
    assert_eq!(eval_line("(fixp (* 3 4))", &env), "T");
    assert_eq!(eval_line("(fixp (/ 10 2))", &env), "T");
}

#[test]
fn test_type_check_before_operation() {
    let env = env_with_stdlib();
    // Safe division that checks types
    eval_line(
        "(defun safe-div (a b) (if (and (numberp a) (numberp b) (not (zerop b))) (/ a b) nil))",
        &env,
    );

    assert_eq!(eval_line("(safe-div 10 2)", &env), "5");
    assert_eq!(eval_line("(safe-div 10 0)", &env), "()");
}

#[test]
fn test_numberp_vs_fixp_floatp() {
    let env = env_with_stdlib();
    // numberp should be true for both fixp and floatp
    eval_line(
        "(defun is-number-type (x) (and (numberp x) (or (fixp x) (floatp x))))",
        &env,
    );

    assert_eq!(eval_line("(is-number-type 42)", &env), "T");
    assert_eq!(eval_line("(is-number-type 3.14)", &env), "T");
    assert_eq!(eval_line("(is-number-type 'symbol)", &env), "()");
}

#[test]
fn test_type_dispatch() {
    let env = env_with_stdlib();
    eval_line(
        "(defun describe-type (x) (cond ((fixp x) 'integer) ((floatp x) 'float) ((stringp x) 'string) ((atom x) 'atom) (t 'list)))",
        &env,
    );

    assert_eq!(eval_line("(describe-type 42)", &env), "INTEGER");
    assert_eq!(eval_line("(describe-type 3.14)", &env), "FLOAT");
    assert_eq!(eval_line("(describe-type \"hi\")", &env), "STRING");
    assert_eq!(eval_line("(describe-type 'sym)", &env), "ATOM");
    assert_eq!(eval_line("(describe-type '(1 2))", &env), "LIST");
}
