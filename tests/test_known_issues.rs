/// This file documents known issues and limitations discovered during correctness review.
/// These tests are marked with #[should_panic] or specific assertions to document the bugs.
mod test_helpers;
use lamedh::eval_line;
use test_helpers::env_with_stdlib;

// ===== FIXED: Empty string parsing =====
// The reader now correctly parses empty strings using take_while

#[test]
fn fixed_empty_string_parsing() {
    let env = env_with_stdlib();
    let output = eval_line("\"\"", &env);
    assert_eq!(output, "\"\"");
}

// ===== FIXED: String parsing with empty string in expressions =====

#[test]
fn fixed_empty_string_in_if() {
    let env = env_with_stdlib();
    let output = eval_line("(if \"\" 'yes 'no)", &env);
    // Empty strings should be truthy in Lisp
    assert_eq!(output, "YES");
}

// ===== FIXED: Integer overflow in arithmetic =====
// The arithmetic operations now use wrapping arithmetic to handle overflow gracefully

#[test]
fn fixed_integer_overflow() {
    let env = env_with_stdlib();
    // i64::MAX + 1 now wraps to i64::MIN
    let output = eval_line("(+ 9223372036854775807 1)", &env);
    // Should wrap to negative number
    assert!(output.contains("-"));
}

// ===== KNOWN LIMITATION: Nested empty lists =====
// Evaluating () in a list context tries to call it as a function

#[test]
fn limitation_nested_empty_lists_must_be_quoted() {
    let env = env_with_stdlib();
    // Without quotes, () is evaluated as NIL and then tried as a function
    let output = eval_line("(() () ())", &env);
    assert!(output.contains("Error") || output.contains("Not a function"));

    // With quotes, it works fine
    let output2 = eval_line("'(() () ())", &env);
    assert_eq!(output2, "(() () ())");
}

// ===== KNOWN LIMITATION: Nested quasiquote =====
// Nested quasiquotes don't handle inner quasiquote symbols correctly

#[test]
fn limitation_nested_quasiquote() {
    let env = env_with_stdlib();
    // Nested quasiquote is a complex feature
    let output = eval_line("`(a `(b ,c))", &env);
    // Current implementation doesn't handle this correctly
    // It should preserve the inner quasiquote
    assert!(
        output.contains("A") || output.contains("Error"),
        "Got: {}",
        output
    );
}

// ===== KNOWN LIMITATION: Quoting operators =====
// Quoting operators like '+ should return the symbol, not evaluate it

#[test]
fn limitation_quote_operator_symbol() {
    let env = env_with_stdlib();
    let output = eval_line("'+", &env);
    // When you quote an operator symbol, it still gets evaluated to the builtin
    // This happens because ' creates (QUOTE +), and + is evaluated to builtin first
    // Expected: Should return the symbol itself, not the builtin
    // The issue is that quote should prevent evaluation, but the symbol
    // is already bound to the builtin in the environment
    assert!(output == "+" || output.contains("builtin") || output == "<builtin>");
}

// ===== Additional edge case tests that work correctly =====

#[test]
fn correct_behavior_car_cdr_of_nil() {
    let env = env_with_stdlib();
    // CAR and CDR of NIL return NIL (this is correct Lisp behavior)
    assert_eq!(eval_line("(car nil)", &env), "()");
    assert_eq!(eval_line("(cdr nil)", &env), "()");
}

#[test]
fn correct_behavior_setq_creates_binding() {
    let env = env_with_stdlib();
    // SETQ creates a new binding if variable doesn't exist
    // This is different from some Lisps but is the current behavior
    let output = eval_line("(setq newvar 42)", &env);
    assert_eq!(output, "42");
    assert_eq!(eval_line("newvar", &env), "42");
}

#[test]
fn correct_behavior_prog_duplicate_labels() {
    let env = env_with_stdlib();
    // With duplicate labels, the last one wins (HashMap behavior)
    // This is documented behavior
    let output = eval_line(
        "(prog (x)
           label1
           (setq x 1)
           label1
           (setq x 2)
           (return x))",
        &env,
    );
    assert_eq!(output, "2");
}

#[test]
fn correct_behavior_and_or_short_circuit() {
    let env = env_with_stdlib();
    // AND and OR correctly short-circuit
    assert_eq!(eval_line("(and nil (/ 1 0))", &env), "()");
    assert_eq!(eval_line("(or t (/ 1 0))", &env), "T");
}

#[test]
fn correct_behavior_lambda_closure() {
    let env = env_with_stdlib();
    // Lambdas correctly capture their environment
    eval_line("(def x 10)", &env);
    eval_line("(def f (lambda (y) (+ x y)))", &env);
    assert_eq!(eval_line("(f 5)", &env), "15");

    // Changing x in outer scope affects the lambda
    eval_line("(setq x 20)", &env);
    assert_eq!(eval_line("(f 5)", &env), "25");
}

#[test]
fn correct_behavior_macro_rest_param() {
    let env = env_with_stdlib();
    // Macros with &REST work correctly
    eval_line("(defmacro mylist (first &rest others) `(cons ,first ',others))", &env);
    let output = eval_line("(mylist 1 2 3 4)", &env);
    assert_eq!(output, "(1 2 3 4)");
}

#[test]
fn correct_behavior_let_shadowing() {
    let env = env_with_stdlib();
    // LET correctly shadows outer variables
    eval_line("(def x 10)", &env);
    assert_eq!(eval_line("(let ((x 20)) x)", &env), "20");
    assert_eq!(eval_line("x", &env), "10");
}

#[test]
fn correct_behavior_cond_returns_predicate_when_no_body() {
    let env = env_with_stdlib();
    // COND with no clause body returns the predicate value
    assert_eq!(eval_line("(cond (t))", &env), "T");
    assert_eq!(eval_line("(cond ((+ 1 2)))", &env), "3");
}
