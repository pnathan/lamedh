use lamedh::LispVal;
use lamedh::environment::Environment;
use lamedh::evaluator::eval;
use lamedh::reader::read;

fn eval_str(input: &str) -> Result<LispVal, String> {
    let env = Environment::new_with_builtins();
    let expr = read(input, &env).map_err(|e| format!("Parse error: {}", e))?;
    eval(&expr, &env).map_err(|e| format!("Eval error: {:?}", e))
}

#[test]
fn test_overflow_flag_on_addition() {
    let input = r#"
        (PROGN
            (CLEAR-ALL-FLAGS)
            (PLUS 9223372036854775807 1)
            (FLAG-SET-P 'OVERFLOW))
    "#;
    let result = eval_str(input);
    assert!(result.is_ok());
    // Should return T because overflow occurred
    match result.unwrap() {
        LispVal::Symbol(s) => assert_eq!(s.borrow().name, "T"),
        _ => panic!("Expected T (overflow flag set)"),
    }
}

#[test]
fn test_overflow_flag_on_multiplication() {
    let input = r#"
        (PROGN
            (CLEAR-ALL-FLAGS)
            (TIMES 9223372036854775807 2)
            (FLAG-SET-P 'OVERFLOW))
    "#;
    let result = eval_str(input);
    assert!(result.is_ok());
    match result.unwrap() {
        LispVal::Symbol(s) => assert_eq!(s.borrow().name, "T"),
        _ => panic!("Expected T (overflow flag set)"),
    }
}

#[test]
fn test_overflow_flag_on_subtraction() {
    let input = r#"
        (PROGN
            (CLEAR-ALL-FLAGS)
            (DIFFERENCE -9223372036854775808 1)
            (FLAG-SET-P 'OVERFLOW))
    "#;
    let result = eval_str(input);
    assert!(result.is_ok());
    match result.unwrap() {
        LispVal::Symbol(s) => assert_eq!(s.borrow().name, "T"),
        _ => panic!("Expected T (overflow flag set)"),
    }
}

#[test]
fn test_overflow_flag_on_division_min_neg_one() {
    let input = r#"
        (PROGN
            (CLEAR-ALL-FLAGS)
            (QUOTIENT -9223372036854775808 -1)
            (FLAG-SET-P 'OVERFLOW))
    "#;
    let result = eval_str(input);
    assert!(result.is_ok());
    match result.unwrap() {
        LispVal::Symbol(s) => assert_eq!(s.borrow().name, "T"),
        _ => panic!("Expected T (overflow flag set)"),
    }
}

#[test]
fn test_overflow_flag_on_remainder() {
    let input = r#"
        (PROGN
            (CLEAR-ALL-FLAGS)
            (REMAINDER -9223372036854775808 -1)
            (FLAG-SET-P 'OVERFLOW))
    "#;
    let result = eval_str(input);
    assert!(result.is_ok());
    match result.unwrap() {
        LispVal::Symbol(s) => assert_eq!(s.borrow().name, "T"),
        _ => panic!("Expected T (overflow flag set)"),
    }
}

#[test]
fn test_overflow_flag_on_leftshift() {
    let input = r#"
        (PROGN
            (CLEAR-ALL-FLAGS)
            (LEFTSHIFT 1 64)
            (FLAG-SET-P 'OVERFLOW))
    "#;
    let result = eval_str(input);
    assert!(result.is_ok());
    match result.unwrap() {
        LispVal::Symbol(s) => assert_eq!(s.borrow().name, "T"),
        _ => panic!("Expected T (overflow flag set)"),
    }
}

#[test]
fn test_no_overflow_flag_on_normal_operation() {
    let input = r#"
        (PROGN
            (CLEAR-ALL-FLAGS)
            (PLUS 1 2 3)
            (FLAG-SET-P 'OVERFLOW))
    "#;
    let result = eval_str(input);
    assert!(result.is_ok());
    // Should return NIL because no overflow
    assert_eq!(result.unwrap(), LispVal::Nil);
}

#[test]
fn test_clear_flag() {
    let input = r#"
        (PROGN
            (SET-FLAG 'OVERFLOW)
            (CLEAR-FLAG 'OVERFLOW)
            (FLAG-SET-P 'OVERFLOW))
    "#;
    let result = eval_str(input);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispVal::Nil);
}

#[test]
fn test_multiple_flags() {
    let input = r#"
        (PROGN
            (SET-FLAG 'OVERFLOW)
            (SET-FLAG 'CUSTOM-FLAG)
            (AND (FLAG-SET-P 'OVERFLOW) (FLAG-SET-P 'CUSTOM-FLAG)))
    "#;
    let result = eval_str(input);
    assert!(result.is_ok());
    match result.unwrap() {
        LispVal::Symbol(s) => assert_eq!(s.borrow().name, "T"),
        _ => panic!("Expected T (both flags set)"),
    }
}

#[test]
fn test_overflow_flag_persists_across_operations() {
    let input = r#"
        (PROGN
            (CLEAR-ALL-FLAGS)
            (PLUS 9223372036854775807 1)
            (PLUS 2 3)
            (FLAG-SET-P 'OVERFLOW))
    "#;
    let result = eval_str(input);
    assert!(result.is_ok());
    // Flag should still be set even after normal operation
    match result.unwrap() {
        LispVal::Symbol(s) => assert_eq!(s.borrow().name, "T"),
        _ => panic!("Expected T (overflow flag persists)"),
    }
}

#[test]
fn test_interactive_overflow_handling() {
    // Demonstrate interactive overflow handling pattern
    let input = r#"
        (PROGN
            (CLEAR-ALL-FLAGS)
            (DEF result (PLUS 9223372036854775807 1))
            (IF (FLAG-SET-P 'OVERFLOW)
                "Overflow detected!"
                "All good"))
    "#;
    let result = eval_str(input);
    assert!(result.is_ok());
    match result.unwrap() {
        LispVal::String(s) => assert_eq!(s, "Overflow detected!"),
        _ => panic!("Expected overflow message"),
    }
}

#[test]
fn test_flag_with_string_name() {
    let input = r#"
        (PROGN
            (SET-FLAG "MY-FLAG")
            (FLAG-SET-P "MY-FLAG"))
    "#;
    let result = eval_str(input);
    assert!(result.is_ok());
    match result.unwrap() {
        LispVal::Symbol(s) => assert_eq!(s.borrow().name, "T"),
        _ => panic!("Expected T (string flag names work)"),
    }
}

#[test]
fn test_computation_continues_after_overflow() {
    // The key feature: computation doesn't stop on overflow
    let input = r#"
        (PROGN
            (CLEAR-ALL-FLAGS)
            (DEF x (PLUS 9223372036854775807 1))
            (DEF y (PLUS x 10))
            (FLAG-SET-P 'OVERFLOW))
    "#;
    let result = eval_str(input);
    assert!(result.is_ok());
    // Should return T (overflow flag still set)
    match result.unwrap() {
        LispVal::Symbol(s) => assert_eq!(s.borrow().name, "T"),
        _ => panic!("Expected T after continued computation"),
    }
}
