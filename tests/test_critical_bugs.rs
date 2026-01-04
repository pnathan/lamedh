/// Comprehensive test suite for critical bugs and edge cases
/// This file contains tests that SHOULD FAIL or expose bugs in the current implementation
use lamedh::environment::Environment;
use lamedh::evaluator::eval;
use lamedh::reader::read;
use lamedh::{load_directory, LispVal};

fn eval_str(input: &str) -> Result<LispVal, String> {
    let env = Environment::new_with_builtins();
    // Load the standard library
    load_directory("lib", &env).map_err(|e| format!("Load error: {:?}", e))?;
    let expr = read(input, &env).map_err(|e| format!("Parse error: {}", e))?;
    eval(&expr, &env).map_err(|e| format!("Eval error: {:?}", e))
}

// ============================================================================
// CRITICAL SEVERITY - BUG #1: Negative Index Wraparound
// ============================================================================

#[test]
fn test_index_negative_number_errors_gracefully() {
    // FIXED: Now properly returns an error instead of panicking
    let result = eval_str("(INDEX \"hello\" -1)");
    println!("Negative index result: {:?}", result);
    assert!(result.is_err(), "Negative index should error gracefully");
    assert!(result.unwrap_err().contains("out of bounds"), "Error should mention bounds");
}

#[test]
fn test_index_negative_number_should_error() {
    let result = eval_str("(INDEX \"hello\" -1)");
    // This test documents the EXPECTED behavior
    assert!(
        result.is_err(),
        "Negative index should return an error, got: {:?}",
        result
    );
}

#[test]
fn test_index_large_negative() {
    let result = eval_str("(INDEX \"test\" -999999)");
    assert!(
        result.is_err(),
        "Large negative index should error, got: {:?}",
        result
    );
}

#[test]
fn test_index_boundary_cases() {
    // Valid indices
    let result = eval_str("(INDEX \"hello\" 0)");
    assert!(result.is_ok());
    if let Ok(LispVal::String(s)) = result {
        assert_eq!(s, "h");
    }

    let result = eval_str("(INDEX \"hello\" 4)");
    assert!(result.is_ok());
    if let Ok(LispVal::String(s)) = result {
        assert_eq!(s, "o");
    }

    // Out of bounds positive
    let result = eval_str("(INDEX \"hello\" 10)");
    assert!(result.is_err(), "Index beyond string length should error");
}

// ============================================================================
// CRITICAL SEVERITY - BUG #2: Circular List Infinite Loop
// ============================================================================

#[test]
#[ignore] // Ignored because it would hang the test suite
fn test_deeply_nested_list_hangs() {
    // This test documents that deeply nested lists could cause issues
    // In a real circular list, this would hang forever
    let mut deep = "(+ ".to_string();
    for _ in 0..100000 {
        deep.push_str("1 ");
    }
    deep.push(')');

    let result = eval_str(&deep);
    println!("Deep nesting result: {:?}", result);
}

// ============================================================================
// CRITICAL SEVERITY - BUG #3: Float -0.0 and 0.0 HashMap Issues
// ============================================================================

#[test]
fn test_float_negative_zero_hash_key() {
    let input = r#"
        (PROGN
            (DEF h (MAKE-HASH-TABLE))
            (SET-BANG h 0.0 "positive zero")
            (GET h -0.0))
    "#;
    let result = eval_str(input);
    println!("Get -0.0 after setting 0.0: {:?}", result);

    // BUG: -0.0 and 0.0 have different hashes but compare equal
    // This violates HashMap invariants
}

#[test]
fn test_float_zero_overwrite() {
    let input = r#"
        (PROGN
            (DEF h (MAKE-HASH-TABLE))
            (SET-BANG h 0.0 "first")
            (SET-BANG h -0.0 "second")
            (KEYS h))
    "#;
    let result = eval_str(input);
    println!("Keys after setting both zeros: {:?}", result);

    // Expected: Only one key (they should be treated as same)
    // Actual: Might have two keys with different hashes
}

// ============================================================================
// HIGH SEVERITY - BUG #4: Integer Overflow in Arithmetic
// ============================================================================

#[test]
fn test_plus_overflow_sets_flag() {
    // Now uses wrapping arithmetic with OVERFLOW flag
    let input = r#"
        (PROGN
            (CLEAR-ALL-FLAGS)
            (PLUS 9223372036854775807 1)
            (FLAG-SET-P 'OVERFLOW))
    "#;
    let result = eval_str(input);
    assert!(result.is_ok(), "Overflow should wrap and set flag, not panic");
    match result.unwrap() {
        LispVal::Symbol(s) => assert_eq!(s.borrow().name, "T"),
        _ => panic!("Expected OVERFLOW flag to be set"),
    }
}

#[test]
#[cfg(not(debug_assertions))]
fn test_plus_overflow_release_wraps() {
    // In release mode, this silently wraps to negative
    let result = eval_str("(PLUS 9223372036854775807 1)");
    println!("i64::MAX + 1 = {:?}", result);

    if let Ok(LispVal::Number(n)) = result {
        // BUG: Wrapped to i64::MIN instead of erroring
        assert_eq!(n, i64::MIN, "Silent wraparound occurred");
    }
}

#[test]
fn test_times_overflow() {
    let result = eval_str("(TIMES 9223372036854775807 2)");
    println!("i64::MAX * 2 = {:?}", result);
    // Should error, not wrap or panic
}

#[test]
fn test_minus_underflow() {
    let result = eval_str("(DIFFERENCE -9223372036854775808 1)");
    println!("i64::MIN - 1 = {:?}", result);
    // Should error, not wrap or panic
}

#[test]
fn test_cascading_overflow() {
    // Multiple operations that compound overflow
    let result = eval_str("(PLUS 1000000000000 1000000000000 1000000000000 1000000000000 1000000000000 1000000000000 1000000000000 1000000000000 1000000000000 1000000000000)");
    println!("Cascading addition overflow: {:?}", result);
}

// ============================================================================
// HIGH SEVERITY - BUG #5: Division i64::MIN / -1 Panic
// ============================================================================

#[test]
fn test_division_min_by_neg_one_sets_flag() {
    // Now wraps and sets OVERFLOW flag instead of panicking
    let input = r#"
        (PROGN
            (CLEAR-ALL-FLAGS)
            (QUOTIENT -9223372036854775808 -1)
            (FLAG-SET-P 'OVERFLOW))
    "#;
    let result = eval_str(input);
    assert!(result.is_ok(), "i64::MIN / -1 should wrap and set flag, not panic");
    match result.unwrap() {
        LispVal::Symbol(s) => assert_eq!(s.borrow().name, "T"),
        _ => panic!("Expected OVERFLOW flag to be set"),
    }
}

#[test]
fn test_division_min_by_neg_one_wraps_with_flag() {
    // Now returns Ok with wrapped value and OVERFLOW flag set
    let input = r#"
        (PROGN
            (CLEAR-ALL-FLAGS)
            (DEF result (QUOTIENT -9223372036854775808 -1))
            (CONS (FLAG-SET-P 'OVERFLOW) result))
    "#;
    let result = eval_str(input);
    assert!(result.is_ok(), "i64::MIN / -1 should succeed with wrapping");
    // The flag should be set (T in CAR) and result wrapped in CDR
    if let Ok(LispVal::Cons { car, cdr: _ }) = result {
        match *car {
            LispVal::Symbol(ref s) => assert_eq!(s.borrow().name, "T", "OVERFLOW flag should be set"),
            _ => panic!("Expected OVERFLOW flag (T) in car"),
        }
    } else {
        panic!("Expected cons of (flag . result)");
    }
}

#[test]
fn test_remainder_min_by_neg_one() {
    let result = eval_str("(REMAINDER -9223372036854775808 -1)");
    println!("i64::MIN % -1 = {:?}", result);
    // This also has the same overflow issue
}

// ============================================================================
// HIGH SEVERITY - BUG #6: Bit Shift Overflow
// ============================================================================

#[test]
fn test_leftshift_64_sets_flag() {
    // Shifting by 64 bits now wraps and sets OVERFLOW flag
    let input = r#"
        (PROGN
            (CLEAR-ALL-FLAGS)
            (LEFTSHIFT 1 64)
            (FLAG-SET-P 'OVERFLOW))
    "#;
    let result = eval_str(input);
    assert!(result.is_ok(), "Shift by 64 should wrap and set flag, not panic");
    match result.unwrap() {
        LispVal::Symbol(s) => assert_eq!(s.borrow().name, "T"),
        _ => panic!("Expected OVERFLOW flag to be set"),
    }
}

#[test]
fn test_leftshift_large_amount_sets_flag() {
    // Large shift amounts now wrap and set OVERFLOW flag
    let input = r#"
        (PROGN
            (CLEAR-ALL-FLAGS)
            (LEFTSHIFT 5 100)
            (FLAG-SET-P 'OVERFLOW))
    "#;
    let result = eval_str(input);
    assert!(result.is_ok(), "Large shift should wrap and set flag, not panic");
    match result.unwrap() {
        LispVal::Symbol(s) => assert_eq!(s.borrow().name, "T"),
        _ => panic!("Expected OVERFLOW flag to be set"),
    }
}

#[test]
fn test_leftshift_large_amount_wraps_with_flag() {
    // Now returns Ok with wrapped value and OVERFLOW flag set
    let input = r#"
        (PROGN
            (CLEAR-ALL-FLAGS)
            (DEF result (LEFTSHIFT 5 100))
            (CONS (FLAG-SET-P 'OVERFLOW) result))
    "#;
    let result = eval_str(input);
    assert!(result.is_ok(), "Large shift should succeed with wrapping");
    // The flag should be set (T in CAR)
    if let Ok(LispVal::Cons { car, cdr: _ }) = result {
        match *car {
            LispVal::Symbol(ref s) => assert_eq!(s.borrow().name, "T", "OVERFLOW flag should be set"),
            _ => panic!("Expected OVERFLOW flag (T) in car"),
        }
    } else {
        panic!("Expected cons of (flag . result)");
    }
}

#[test]
fn test_leftshift_negative_large() {
    // Right shift by large amount
    let result = eval_str("(LEFTSHIFT 128 -100)");
    println!("Right shift by 100: {:?}", result);
    // Might also panic
}

#[test]
fn test_leftshift_63_boundary() {
    // Shifting by 63 should work
    let result = eval_str("(LEFTSHIFT 1 63)");
    if let Ok(LispVal::Number(n)) = result {
        assert_eq!(n, i64::MIN); // 1 << 63 = -9223372036854775808
    } else {
        panic!("Shift by 63 should work: {:?}", result);
    }
}

// ============================================================================
// HIGH SEVERITY - BUG #7: LABEL Infinite Recursion
// ============================================================================

#[test]
#[ignore] // Ignored because it causes stack overflow
fn test_label_self_reference_stack_overflow() {
    let result = eval_str("(LABEL x x)");
    println!("LABEL x x: {:?}", result);
    // This will cause stack overflow
}

#[test]
#[ignore] // Ignored because it causes stack overflow
fn test_label_circular_reference() {
    let input = "(LABEL a (LABEL b a))";
    let result = eval_str(input);
    println!("Circular LABEL: {:?}", result);
}

// ============================================================================
// MEDIUM SEVERITY - BUG #8: ASSOC with Malformed List
// ============================================================================

#[test]
fn test_assoc_with_atoms() {
    let result = eval_str("(ASSOC 'X '(1 2 3))");
    println!("ASSOC with atoms: {:?}", result);
    // Should return NIL or error, not panic
    assert!(
        result.is_ok(),
        "ASSOC should handle non-pair lists gracefully"
    );
}

#[test]
fn test_assoc_mixed_list() {
    let input = "(ASSOC 'A '((A . 1) B (C . 3)))";
    let result = eval_str(input);
    println!("ASSOC mixed list: {:?}", result);
    // Should find (A . 1) or handle 'B' gracefully
}

#[test]
fn test_assoc_with_nested_atoms() {
    let result = eval_str("(ASSOC 5 '((1 . 2) 3 4 (5 . 6)))");
    println!("ASSOC with nested atoms: {:?}", result);
}

// ============================================================================
// MEDIUM SEVERITY - BUG #9: SETQ Creates Variables
// ============================================================================

#[test]
fn test_setq_creates_undefined_variable() {
    let env = Environment::new_with_builtins();

    // Try to SETQ a variable that doesn't exist
    let expr = read("(SETQ brand-new-var 999)", &env).unwrap();
    let result = eval(&expr, &env);

    println!("SETQ undefined variable: {:?}", result);
    // BUG: This succeeds and creates the variable
    // Expected in most Lisps: error

    // Verify it was created
    let lookup = read("brand-new-var", &env).unwrap();
    let value = eval(&lookup, &env);
    println!("Value after SETQ: {:?}", value);

    // In most Lisps, SETQ should error on undefined variables
    // Only DEF/DEFVAR should create new bindings
}

// ============================================================================
// MEDIUM SEVERITY - BUG #10: DEFINE with Non-List
// ============================================================================

#[test]
fn test_define_with_atom() {
    let result = eval_str("(DEFINE 42)");
    assert!(result.is_err(), "DEFINE with atom should error");
    println!("DEFINE with atom error: {:?}", result);
}

#[test]
fn test_define_with_symbol() {
    let result = eval_str("(DEFINE 'x)");
    assert!(result.is_err(), "DEFINE with symbol should error");
    println!("DEFINE with symbol error: {:?}", result);
}

#[test]
fn test_define_empty_list() {
    // DEFINE is a special form that doesn't evaluate its argument
    // so '() (which is (QUOTE ())) doesn't work. Use () directly.
    let result = eval_str("(DEFINE ())");
    assert!(result.is_ok(), "DEFINE with empty list should work");
    assert_eq!(result.unwrap(), LispVal::Nil, "Empty DEFINE should return NIL");
}

// ============================================================================
// MEDIUM SEVERITY - BUG #11: DEFLIST with Malformed Input
// ============================================================================

#[test]
fn test_deflist_with_atoms() {
    let result = eval_str("(DEFLIST '(a b c) \"prop\")");
    println!("DEFLIST with atoms: {:?}", result);
    // Should error or silently skip
}

#[test]
fn test_deflist_incomplete_pairs() {
    let result = eval_str("(DEFLIST '((a) (b)) \"prop\")");
    println!("DEFLIST incomplete pairs: {:?}", result);
    // Each pair should have both symbol and value
}

#[test]
fn test_deflist_valid_then_check() {
    let env = Environment::new_with_builtins();

    // Set properties
    let setup = read("(DEFLIST '((X 1) (Y 2)) \"test-prop\")", &env).unwrap();
    let _ = eval(&setup, &env);

    // Check if properties were set
    let check_x = read("(GETP 'X \"test-prop\")", &env).unwrap();
    let result = eval(&check_x, &env);
    println!("GETP after DEFLIST: {:?}", result);
}

// ============================================================================
// MEDIUM SEVERITY - BUG #12: PROG Duplicate Labels
// ============================================================================

#[test]
fn test_prog_duplicate_labels() {
    let input = r#"
        (PROG (result)
          (SETQ result 1)
          label1
          (SETQ result (PLUS result 10))
          label1
          (SETQ result (PLUS result 100))
          (RETURN result))
    "#;
    let result = eval_str(input);
    println!("PROG duplicate labels result: {:?}", result);

    // The second label1 overwrites the first
    // Result should be 111 (1 + 10 + 100)
    if let Ok(LispVal::Number(n)) = result {
        assert_eq!(n, 111);
    }
}

#[test]
fn test_prog_go_to_duplicate_label() {
    let input = r#"
        (PROG (count)
          (SETQ count 0)
          start
          (SETQ count (PLUS count 1))
          start
          (SETQ count (PLUS count 10))
          (IF (= count 11) (RETURN count) (GO start)))
    "#;
    let result = eval_str(input);
    println!("PROG GO to duplicate: {:?}", result);

    // Which 'start' does GO jump to? The second one (last inserted)
}

// ============================================================================
// MEDIUM SEVERITY - BUG #13: Stack Overflow on Deep Nesting
// ============================================================================

#[test]
#[ignore] // Might stack overflow
fn test_deeply_nested_quasiquote() {
    let mut s = "`".to_string();
    for _ in 0..1000 {
        s.push_str("(a ");
    }
    for _ in 0..1000 {
        s.push(')');
    }

    let result = eval_str(&s);
    println!("Deep quasiquote result (len): {:?}", result);
}

#[test]
fn test_moderately_nested_quasiquote() {
    let mut s = "`".to_string();
    for _ in 0..100 {
        s.push_str("(a ");
    }
    for _ in 0..100 {
        s.push(')');
    }

    let result = eval_str(&s);
    println!("Moderate nesting: {:?}", result.is_ok());
    assert!(result.is_ok(), "100 levels should work");
}

// ============================================================================
// LOW SEVERITY - BUG #14 & #15: Parser Edge Cases
// ============================================================================

#[test]
fn test_string_with_escaped_quote() {
    // This will fail to parse because escaping isn't supported
    let result = eval_str(r#""hello \"world\"""#);
    println!("String with escape: {:?}", result);
    assert!(result.is_err(), "Escaped quotes not supported");
}

#[test]
fn test_minus_standalone() {
    let env = Environment::new_with_builtins();
    let result = read("-", &env);
    println!("Standalone minus: {:?}", result);
    // Should parse as the minus symbol/function
    assert!(result.is_ok());
}

#[test]
fn test_minus_in_expression() {
    let result = eval_str("(- 10 3)");
    assert!(result.is_ok());
    if let Ok(LispVal::Number(n)) = result {
        assert_eq!(n, 7);
    }
}

// ============================================================================
// Additional Edge Cases
// ============================================================================

#[test]
fn test_empty_string_index() {
    let result = eval_str("(INDEX \"\" 0)");
    assert!(result.is_err(), "Index into empty string should error");
}

#[test]
fn test_car_on_non_list() {
    let result = eval_str("(CAR 42)");
    assert!(result.is_err(), "CAR on atom should error");
}

#[test]
fn test_cdr_on_non_list() {
    let result = eval_str("(CDR 'symbol)");
    assert!(result.is_err(), "CDR on symbol should error");
}

#[test]
fn test_remainder_by_zero() {
    let result = eval_str("(REMAINDER 10 0)");
    assert!(result.is_err(), "Remainder by zero should error");
    assert!(result.unwrap_err().contains("zero"));
}

#[test]
fn test_expt_negative_exponent() {
    let result = eval_str("(EXPT 2 -1)");
    assert!(
        result.is_err(),
        "Negative exponent should error (not supported)"
    );
}

#[test]
fn test_if_with_wrong_arg_count() {
    let result = eval_str("(IF T 1)");
    assert!(result.is_err(), "IF with 2 args should error (needs 3)");

    let result = eval_str("(IF T 1 2 3)");
    assert!(result.is_err(), "IF with 4 args should error (needs 3)");
}

#[test]
fn test_lambda_wrong_arg_count() {
    let result = eval_str(r#"
        (PROGN
            (DEF f (LAMBDA (x y) (+ x y)))
            (f 1))
    "#);
    assert!(result.is_err(), "Calling lambda with wrong arg count should error");
}

#[test]
fn test_lambda_too_many_args() {
    let result = eval_str(r#"
        (PROGN
            (DEF f (LAMBDA (x) x))
            (f 1 2 3))
    "#);
    assert!(result.is_err(), "Too many args to lambda should error");
}

#[test]
fn test_macro_expansion_error_handling() {
    let env = Environment::new_with_builtins();

    // Define a macro that errors during expansion
    let def = read("(DEFMACRO bad-macro (x) (ERROR \"macro error\"))", &env).unwrap();
    eval(&def, &env).unwrap();

    // Try to use it
    let call = read("(bad-macro foo)", &env).unwrap();
    let result = eval(&call, &env);

    println!("Macro expansion error: {:?}", result);
    assert!(result.is_err(), "Macro expansion error should propagate");
}

#[test]
fn test_apply_with_non_list() {
    let result = eval_str("(APPLY + 42)");
    assert!(result.is_err(), "APPLY with non-list should error");
}

#[test]
fn test_apply_with_improper_list() {
    let result = eval_str("(APPLY + '(1 2 . 3))");
    assert!(result.is_err(), "APPLY with improper list should error");
}
