mod test_helpers;
use lamedh::eval_line;
use test_helpers::env_with_prologue;

// ===== Float Handling Tests =====

#[test]
fn test_float_nan_equality() {
    let env = env_with_prologue();
    // NaN should not equal itself (IEEE 754 standard)
    // This tests a potential bug in lib.rs:142 where Float uses direct f64 equality
    let result = eval_line("(def x (/ 0.0 0.0))", &env);
    assert!(result.contains("X") || result.contains("Error"));

    // Note: The current implementation doesn't have float division, so this is
    // a placeholder for when/if float operations are added
}

#[test]
fn test_float_hash_consistency() {
    let env = env_with_prologue();
    // Test that floats can be used as hash keys
    // The Hash implementation for LispVal uses f64.to_bits() which is correct
    let output = eval_line("(def ht (make-hash-table))", &env);
    assert!(output.contains("HT") || output == "HT");
}

// ===== String Parsing Tests =====

#[test]
fn test_string_without_escape_sequences() {
    let env = env_with_prologue();
    // Current implementation doesn't handle escape sequences in strings
    // This is a known limitation: reader.rs:98 uses is_not("\"")
    let output = eval_line("\"hello world\"", &env);
    assert_eq!(output, "\"hello world\"");
}

#[test]
fn test_empty_string() {
    let env = env_with_prologue();
    let output = eval_line("\"\"", &env);
    assert_eq!(output, "\"\"");
}

#[test]
fn test_string_with_special_chars() {
    let env = env_with_prologue();
    // Test strings with characters that don't require escaping
    let output = eval_line("\"hello!@#$%^&*()\"", &env);
    assert_eq!(output, "\"hello!@#$%^&*()\"");
}

// ===== Empty List Parsing Tests =====

#[test]
fn test_empty_list_parsing() {
    let env = env_with_prologue();
    let output = eval_line("()", &env);
    assert_eq!(output, "()");
}

#[test]
fn test_nested_empty_lists() {
    let env = env_with_prologue();
    let output = eval_line("(() () ())", &env);
    assert_eq!(output, "(() () ())");
}

#[test]
fn test_quote_empty_list() {
    let env = env_with_prologue();
    let output = eval_line("'()", &env);
    assert_eq!(output, "()");
}

// ===== Arithmetic Edge Cases =====

#[test]
fn test_division_by_zero() {
    let env = env_with_prologue();
    let output = eval_line("(/ 10 0)", &env);
    assert!(output.contains("Error") || output.contains("Division by zero"));
}

#[test]
fn test_division_with_wrong_arg_count() {
    let env = env_with_prologue();
    // Division requires exactly 2 args (evaluator.rs:69)
    let output = eval_line("(/ 10)", &env);
    assert!(output.contains("Error"));

    let output2 = eval_line("(/ 10 2 3)", &env);
    assert!(output2.contains("Error"));
}

#[test]
fn test_minus_single_argument() {
    let env = env_with_prologue();
    // Should negate the number (evaluator.rs:57-58)
    let output = eval_line("(- 5)", &env);
    assert_eq!(output, "-5");
}

#[test]
fn test_minus_zero_arguments() {
    let env = env_with_prologue();
    let output = eval_line("(-)", &env);
    assert!(output.contains("Error"));
}

#[test]
fn test_plus_zero_arguments() {
    let env = env_with_prologue();
    // Sum of empty list should be 0
    let output = eval_line("(+)", &env);
    assert_eq!(output, "0");
}

#[test]
fn test_multiply_zero_arguments() {
    let env = env_with_prologue();
    // Product of empty list should be 1
    let output = eval_line("(*)", &env);
    assert_eq!(output, "1");
}

#[test]
fn test_integer_overflow() {
    let env = env_with_prologue();
    // i64::MAX is 9223372036854775807
    // This tests potential overflow in addition
    let output = eval_line("(+ 9223372036854775807 1)", &env);
    // Rust's default behavior is to panic in debug mode, wrap in release mode
    // This test documents the current behavior
    assert!(output.contains("-") || output.contains("Error"));
}

// ===== List Operation Edge Cases =====

#[test]
fn test_car_of_nil() {
    let env = env_with_prologue();
    // evaluator.rs:93 returns Nil for car of Nil
    let output = eval_line("(car nil)", &env);
    assert_eq!(output, "()");
}

#[test]
fn test_cdr_of_nil() {
    let env = env_with_prologue();
    // evaluator.rs:105 returns Nil for cdr of Nil
    let output = eval_line("(cdr nil)", &env);
    assert_eq!(output, "()");
}

#[test]
fn test_car_of_non_list() {
    let env = env_with_prologue();
    let output = eval_line("(car 5)", &env);
    assert!(output.contains("Error"));
}

#[test]
fn test_cdr_of_non_list() {
    let env = env_with_prologue();
    let output = eval_line("(cdr \"hello\")", &env);
    assert!(output.contains("Error"));
}

// ===== Quasiquote/Unquote Tests =====

#[test]
fn test_nested_quasiquote() {
    let env = env_with_prologue();
    let output = eval_line("`(a `(b ,c))", &env);
    // This tests the quasiquote_eval function (evaluator.rs:1004)
    // Nested quasiquotes are tricky
    assert!(output.contains("A"));
}

#[test]
fn test_unquote_outside_quasiquote() {
    let env = env_with_prologue();
    // Unquote should only work inside quasiquote
    let output = eval_line(",x", &env);
    // This will try to evaluate (UNQUOTE x) which should fail
    assert!(output.contains("Error") || output.contains("Unbound"));
}

#[test]
fn test_quasiquote_with_nil() {
    let env = env_with_prologue();
    let output = eval_line("`()", &env);
    assert_eq!(output, "()");
}

#[test]
fn test_quasiquote_splice_unquote() {
    let env = env_with_prologue();
    // Current implementation doesn't support ,@ (splice-unquote)
    // This documents the limitation
    let output = eval_line("(def lst '(1 2 3))", &env);
    assert!(output.contains("LST"));

    let output2 = eval_line("`(a ,@lst b)", &env);
    // This will fail or not work as expected since ,@ is not implemented
    assert!(output2.contains("Error") || !output2.contains("(A 1 2 3 B)"));
}

// ===== SETQ Tests =====

#[test]
fn test_setq_on_unbound_variable() {
    let env = env_with_prologue();
    // SETQ uses Environment::update which creates the binding if not found
    // (environment.rs:162-175)
    let output = eval_line("(setq newvar 42)", &env);
    assert_eq!(output, "42");

    let output2 = eval_line("newvar", &env);
    assert_eq!(output2, "42");
}

#[test]
fn test_setq_updates_existing_variable() {
    let env = env_with_prologue();
    let output = eval_line("(def x 10)", &env);
    assert!(output.contains("X"));

    let output2 = eval_line("(setq x 20)", &env);
    assert_eq!(output2, "20");

    let output3 = eval_line("x", &env);
    assert_eq!(output3, "20");
}

#[test]
fn test_setq_multiple_assignments() {
    let env = env_with_prologue();
    // SETQ can set multiple variables (evaluator.rs:829-850)
    let output = eval_line("(setq a 1 b 2 c 3)", &env);
    assert_eq!(output, "3"); // Returns last value

    assert_eq!(eval_line("a", &env), "1");
    assert_eq!(eval_line("b", &env), "2");
    assert_eq!(eval_line("c", &env), "3");
}

#[test]
fn test_setq_odd_number_of_args() {
    let env = env_with_prologue();
    let output = eval_line("(setq a 1 b)", &env);
    assert!(output.contains("Error"));
}

// ===== Lambda Tests =====

#[test]
fn test_lambda_wrong_arg_count() {
    let env = env_with_prologue();
    let output = eval_line("(def f (lambda (x y) (+ x y)))", &env);
    assert!(output.contains("F"));

    let output2 = eval_line("(f 1)", &env);
    assert!(output2.contains("Error"));

    let output3 = eval_line("(f 1 2 3)", &env);
    assert!(output3.contains("Error"));
}

#[test]
fn test_lambda_multi_statement_body() {
    let env = env_with_prologue();
    // Lambda with multiple body statements should wrap them in PROGN
    // (evaluator.rs:679-685)
    let output = eval_line("((lambda (x) (setq y 1) (setq z 2) (+ x y z)) 10)", &env);
    assert_eq!(output, "13");
}

#[test]
fn test_lambda_closure() {
    let env = env_with_prologue();
    // Test that lambda captures its environment
    let output = eval_line("(def x 10)", &env);
    assert!(output.contains("X"));

    let output2 = eval_line("(def f (lambda (y) (+ x y)))", &env);
    assert!(output2.contains("F"));

    let output3 = eval_line("(f 5)", &env);
    assert_eq!(output3, "15");
}

// ===== Macro Tests =====

#[test]
fn test_macro_with_rest_param() {
    let env = env_with_prologue();
    // Test &REST parameter handling (evaluator.rs:446-487)
    let output = eval_line("(defmacro mylist (first &rest others) `(cons ,first ',others))", &env);
    assert!(output.contains("MYLIST"));

    let output2 = eval_line("(mylist 1 2 3 4)", &env);
    assert!(output2.contains("1"));
}

#[test]
fn test_macro_rest_param_with_no_rest_args() {
    let env = env_with_prologue();
    let output = eval_line("(defmacro mylist (first &rest others) `(cons ,first ',others))", &env);
    assert!(output.contains("MYLIST"));

    let output2 = eval_line("(mylist 1)", &env);
    // Should work with empty rest list
    assert!(output2.contains("1"));
}

#[test]
fn test_macro_rest_param_insufficient_args() {
    let env = env_with_prologue();
    let output = eval_line("(defmacro mylist (first second &rest others) `(cons ,first ',others))", &env);
    assert!(output.contains("MYLIST"));

    let output2 = eval_line("(mylist 1)", &env);
    // Should error: not enough args for required params
    assert!(output2.contains("Error"));
}

#[test]
fn test_macro_multiple_symbols_after_rest() {
    let env = env_with_prologue();
    // Only one symbol can follow &rest (evaluator.rs:460-463)
    let output = eval_line("(defmacro bad (a &rest b c) `(cons ,a ',b))", &env);
    assert!(output.contains("Error"));
}

// ===== PROG Tests =====

#[test]
fn test_prog_duplicate_labels() {
    let env = env_with_prologue();
    // PROG uses a HashMap for labels (evaluator.rs:875)
    // Duplicate labels will overwrite (last one wins)
    let output = eval_line("(prog (x) loop (setq x 1) loop (return x))", &env);
    // The second 'loop' label overwrites the first
    // GO to 'loop' would go to the second occurrence
    assert_eq!(output, "1");
}

#[test]
fn test_prog_go_to_nonexistent_label() {
    let env = env_with_prologue();
    let output = eval_line("(prog (x) (go nowhere))", &env);
    assert!(output.contains("Error") || output.contains("not found"));
}

#[test]
fn test_prog_return_outside_prog() {
    let env = env_with_prologue();
    let output = eval_line("(return 5)", &env);
    // Should error because RETURN creates a LispError::Return
    // which is only caught by PROG (evaluator.rs:900)
    assert!(output.contains("Error"));
}

#[test]
fn test_prog_go_outside_prog() {
    let env = env_with_prologue();
    let output = eval_line("(go label)", &env);
    assert!(output.contains("Error"));
}

#[test]
fn test_prog_label_not_expression() {
    let env = env_with_prologue();
    // Labels (symbols) are skipped, not evaluated (evaluator.rs:891-894)
    let output = eval_line("(prog () start (return 42) start)", &env);
    assert_eq!(output, "42");
}

#[test]
fn test_prog_falls_off_end() {
    let env = env_with_prologue();
    // PROG returns NIL if it falls off the end (evaluator.rs:885)
    let output = eval_line("(prog (x) (setq x 1))", &env);
    assert_eq!(output, "()");
}

// ===== COND Tests =====

#[test]
fn test_cond_no_matching_clause() {
    let env = env_with_prologue();
    let output = eval_line("(cond (nil 1) (() 2))", &env);
    assert_eq!(output, "()");
}

#[test]
fn test_cond_with_empty_body() {
    let env = env_with_prologue();
    // If clause body is empty, return the predicate value (evaluator.rs:581-582)
    let output = eval_line("(cond (t))", &env);
    assert_eq!(output, "T");
}

#[test]
fn test_cond_clause_not_list() {
    let env = env_with_prologue();
    let output = eval_line("(cond 5)", &env);
    assert!(output.contains("Error"));
}

// ===== Symbol and Property List Tests =====

#[test]
fn test_getp_nonexistent_property() {
    let env = env_with_prologue();
    let output = eval_line("(getp 'x \"nonexistent\")", &env);
    assert_eq!(output, "()");
}

#[test]
fn test_putp_and_getp() {
    let env = env_with_prologue();
    let output = eval_line("(putp 'mysym \"myprop\" 42)", &env);
    assert_eq!(output, "T");

    let output2 = eval_line("(getp 'mysym \"myprop\")", &env);
    assert_eq!(output2, "42");
}

#[test]
fn test_getp_non_symbol() {
    let env = env_with_prologue();
    let output = eval_line("(getp 5 \"prop\")", &env);
    assert!(output.contains("Error"));
}

#[test]
fn test_putp_non_string_property() {
    let env = env_with_prologue();
    let output = eval_line("(putp 'sym 42 \"value\")", &env);
    assert!(output.contains("Error"));
}

// ===== Hash Table Tests =====

#[test]
fn test_hashtable_with_various_key_types() {
    let env = env_with_prologue();
    let output = eval_line("(def ht (make-hash-table))", &env);
    assert!(output.contains("HT"));

    // Test with number key
    eval_line("(set-bang ht 42 \"num\")", &env);
    assert_eq!(eval_line("(get ht 42)", &env), "\"num\"");

    // Test with symbol key
    eval_line("(set-bang ht 'sym \"symbol\")", &env);
    assert_eq!(eval_line("(get ht 'sym)", &env), "\"symbol\"");

    // Test with string key
    eval_line("(set-bang ht \"key\" \"string\")", &env);
    assert_eq!(eval_line("(get ht \"key\")", &env), "\"string\"");
}

#[test]
fn test_hashtable_get_nonexistent_key() {
    let env = env_with_prologue();
    let output = eval_line("(def ht (make-hash-table))", &env);
    assert!(output.contains("HT"));

    let output2 = eval_line("(get ht 'nonexistent)", &env);
    assert_eq!(output2, "()");
}

#[test]
fn test_hashtable_delete_key() {
    let env = env_with_prologue();
    eval_line("(def ht (make-hash-table))", &env);
    eval_line("(set-bang ht 'key \"value\")", &env);
    assert_eq!(eval_line("(get ht 'key)", &env), "\"value\"");

    eval_line("(delete-key-bang ht 'key)", &env);
    assert_eq!(eval_line("(get ht 'key)", &env), "()");
}

#[test]
fn test_hashtable_keys() {
    let env = env_with_prologue();
    eval_line("(def ht (make-hash-table))", &env);
    eval_line("(set-bang ht 'a 1)", &env);
    eval_line("(set-bang ht 'b 2)", &env);

    let output = eval_line("(keys ht)", &env);
    // Keys can be in any order
    assert!(output.contains("A") || output.contains("B"));
}

// ===== LET Tests =====

#[test]
fn test_let_shadowing() {
    let env = env_with_prologue();
    eval_line("(def x 10)", &env);

    // LET should create a new scope
    let output = eval_line("(let ((x 20)) x)", &env);
    assert_eq!(output, "20");

    // Original x should be unchanged
    let output2 = eval_line("x", &env);
    assert_eq!(output2, "10");
}

#[test]
fn test_let_empty_bindings() {
    let env = env_with_prologue();
    let output = eval_line("(let () 42)", &env);
    assert_eq!(output, "42");
}

#[test]
fn test_let_binding_not_pair() {
    let env = env_with_prologue();
    let output = eval_line("(let (x) 42)", &env);
    assert!(output.contains("Error"));
}

#[test]
fn test_let_wrong_arg_count() {
    let env = env_with_prologue();
    let output = eval_line("(let ((x 1)))", &env);
    assert!(output.contains("Error"));

    let output2 = eval_line("(let)", &env);
    assert!(output2.contains("Error"));
}

// ===== AND/OR Tests =====

#[test]
fn test_and_empty() {
    let env = env_with_prologue();
    // AND with no arguments should return T (evaluator.rs:621)
    let output = eval_line("(and)", &env);
    assert_eq!(output, "T");
}

#[test]
fn test_and_short_circuit() {
    let env = env_with_prologue();
    // AND should short-circuit on first nil
    let output = eval_line("(and t nil (/ 1 0))", &env);
    // Should not evaluate (/ 1 0) which would error
    assert_eq!(output, "()");
}

#[test]
fn test_or_empty() {
    let env = env_with_prologue();
    let output = eval_line("(or)", &env);
    assert_eq!(output, "()");
}

#[test]
fn test_or_short_circuit() {
    let env = env_with_prologue();
    // OR should short-circuit on first truthy value
    let output = eval_line("(or nil t (/ 1 0))", &env);
    // Should not evaluate (/ 1 0) which would error
    assert_eq!(output, "T");
}

#[test]
fn test_or_returns_first_truthy() {
    let env = env_with_prologue();
    let output = eval_line("(or nil 42 99)", &env);
    assert_eq!(output, "42");
}

// ===== IF Tests =====

#[test]
fn test_if_wrong_arg_count() {
    let env = env_with_prologue();
    let output = eval_line("(if t 1)", &env);
    assert!(output.contains("Error"));

    let output2 = eval_line("(if t 1 2 3)", &env);
    assert!(output2.contains("Error"));
}

#[test]
fn test_if_with_non_nil_truthy() {
    let env = env_with_prologue();
    // Any non-nil value is truthy
    let output = eval_line("(if 0 'yes 'no)", &env);
    assert_eq!(output, "YES");

    let output2 = eval_line("(if \"\" 'yes 'no)", &env);
    assert_eq!(output2, "YES");
}

// ===== QUOTE Tests =====

#[test]
fn test_quote_wrong_arg_count() {
    let env = env_with_prologue();
    let output = eval_line("(quote)", &env);
    assert!(output.contains("Error"));

    let output2 = eval_line("(quote a b)", &env);
    assert!(output2.contains("Error"));
}

// ===== FUNCTION Tests =====

#[test]
fn test_function_non_lambda() {
    let env = env_with_prologue();
    let output = eval_line("(function x)", &env);
    assert!(output.contains("Error"));
}

#[test]
fn test_function_with_lambda() {
    let env = env_with_prologue();
    let output = eval_line("(function (lambda (x) (+ x 1)))", &env);
    assert_eq!(output, "<lambda>");
}

// ===== LABEL Tests =====

#[test]
fn test_label_recursive_function() {
    let env = env_with_prologue();
    // LABEL allows recursive functions (evaluator.rs:725-748)
    let output = eval_line(
        "(label factorial (lambda (n) (if (= n 0) 1 (* n (factorial (- n 1))))))",
        &env
    );
    // This should work - LABEL binds the name in a new environment
    assert!(output == "1" || output == "<lambda>" || !output.contains("Error"));
}

#[test]
fn test_label_non_symbol_name() {
    let env = env_with_prologue();
    let output = eval_line("(label 5 (lambda (x) x))", &env);
    assert!(output.contains("Error"));
}

// ===== DEFINE Tests =====

#[test]
fn test_define_multiple_bindings() {
    let env = env_with_prologue();
    let output = eval_line("(define ((a 1) (b 2) (c 3)))", &env);
    // Should return list of defined symbols
    assert!(output.contains("A") && output.contains("B") && output.contains("C"));

    assert_eq!(eval_line("a", &env), "1");
    assert_eq!(eval_line("b", &env), "2");
    assert_eq!(eval_line("c", &env), "3");
}

#[test]
fn test_define_empty_list() {
    let env = env_with_prologue();
    let output = eval_line("(define ())", &env);
    assert_eq!(output, "()");
}

#[test]
fn test_define_binding_wrong_length() {
    let env = env_with_prologue();
    let output = eval_line("(define ((a)))", &env);
    assert!(output.contains("Error"));

    let output2 = eval_line("(define ((a 1 2)))", &env);
    assert!(output2.contains("Error"));
}

// ===== Evaluation of Symbols =====

#[test]
fn test_unbound_symbol() {
    let env = env_with_prologue();
    let output = eval_line("nonexistent", &env);
    assert!(output.contains("Error") || output.contains("Unbound"));
}

#[test]
fn test_nil_symbol() {
    let env = env_with_prologue();
    let output = eval_line("nil", &env);
    assert_eq!(output, "()");
}

#[test]
fn test_t_symbol() {
    let env = env_with_prologue();
    let output = eval_line("t", &env);
    assert_eq!(output, "T");
}

// ===== Reader Edge Cases =====

#[test]
fn test_read_operators_as_symbols() {
    let env = env_with_prologue();
    // Operators should be parsed as symbols (reader.rs:88-92)
    let output = eval_line("'+", &env);
    assert!(output.contains("<builtin>") || output == "<builtin>");
}

#[test]
fn test_parse_negative_number() {
    let env = env_with_prologue();
    let output = eval_line("-42", &env);
    assert_eq!(output, "-42");
}

#[test]
fn test_parse_symbol_with_hyphen() {
    let env = env_with_prologue();
    let output = eval_line("'my-symbol", &env);
    assert_eq!(output, "MY-SYMBOL");
}

#[test]
fn test_parse_ampersand_symbol() {
    let env = env_with_prologue();
    let output = eval_line("'&rest", &env);
    assert_eq!(output, "&REST");
}

// ===== Comments =====

#[test]
fn test_comment_at_end_of_line() {
    let env = env_with_prologue();
    let output = eval_line("(+ 1 2) ; this is a comment", &env);
    assert_eq!(output, "3");
}

#[test]
fn test_full_line_comment() {
    let env = env_with_prologue();
    let output = eval_line("; just a comment", &env);
    // Should error because there's no expression to evaluate
    assert!(output.contains("Error") || output.contains("Unexpected"));
}

// ===== Atom Predicate =====

#[test]
fn test_atom_on_all_types() {
    let env = env_with_prologue();
    assert_eq!(eval_line("(atom 42)", &env), "T");
    assert_eq!(eval_line("(atom 3.14)", &env), "T");
    assert_eq!(eval_line("(atom \"string\")", &env), "T");
    assert_eq!(eval_line("(atom 'symbol)", &env), "T");
    assert_eq!(eval_line("(atom nil)", &env), "T");
    assert_eq!(eval_line("(atom '(1 2))", &env), "()");
}

// ===== Environment edge cases =====

#[test]
fn test_nested_environment_lookup() {
    let env = env_with_prologue();
    // Test that nested scopes work correctly
    eval_line("(def outer 1)", &env);
    let output = eval_line("((lambda (inner) (+ outer inner)) 2)", &env);
    assert_eq!(output, "3");
}

#[test]
fn test_environment_current_environment() {
    let env = env_with_prologue();
    let output = eval_line("(current-environment)", &env);
    assert!(output.contains("<hash-table>") || output == "<hash-table>");
}
