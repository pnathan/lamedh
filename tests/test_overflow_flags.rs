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

// ---------------------------------------------------------------------------
// GCD/LCM checked arithmetic (issue #272): `gcd_i64`'s `.abs()` calls and
// `lcm`'s `(a / g * b).abs()` used unchecked arithmetic, which panics in
// debug builds (and silently wraps in release) when an operand is
// `i64::MIN`, since `i64::MIN.abs()` cannot be represented in `i64`.
// ---------------------------------------------------------------------------

#[test]
fn test_gcd_happy_path_no_overflow() {
    let input = r#"
        (PROGN
            (CLEAR-ALL-FLAGS)
            (LIST (GCD 48 18) (FLAG-SET-P 'OVERFLOW)))
    "#;
    let result = eval_str(input).unwrap();
    let items = lisp_list_to_vec(&result);
    assert_eq!(items[0], LispVal::Number(6));
    assert!(!is_truthy_symbol(&items[1]), "no overflow expected");
}

#[test]
fn test_gcd_min_and_positive_sets_overflow() {
    // gcd(i64::MIN, 5) requires |i64::MIN|, which is unrepresentable in i64,
    // so OVERFLOW is set — but the Euclid loop preserves the magnitude and
    // the sign is normalized, so the *true* gcd is still returned.
    let input = r#"
        (PROGN
            (CLEAR-ALL-FLAGS)
            (LIST (GCD -9223372036854775808 5) (FLAG-SET-P 'OVERFLOW)))
    "#;
    let result = eval_str(input);
    assert!(result.is_ok(), "must not panic/abort: {result:?}");
    let items = lisp_list_to_vec(&result.unwrap());
    assert_eq!(items[0], LispVal::Number(1), "gcd(i64::MIN, 5) must be 1");
    assert!(is_truthy_symbol(&items[1]), "overflow flag must be set");
}

#[test]
fn test_gcd_min_and_six_sets_overflow_with_true_gcd() {
    // gcd(i64::MIN, 6): |i64::MIN| = 2^63 shares a factor of 2 with 6, so the
    // true gcd is 2. OVERFLOW is still set because |i64::MIN| overflowed.
    let input = r#"
        (PROGN
            (CLEAR-ALL-FLAGS)
            (LIST (GCD -9223372036854775808 6) (FLAG-SET-P 'OVERFLOW)))
    "#;
    let result = eval_str(input);
    assert!(result.is_ok(), "must not panic/abort: {result:?}");
    let items = lisp_list_to_vec(&result.unwrap());
    assert_eq!(items[0], LispVal::Number(2), "gcd(i64::MIN, 6) must be 2");
    assert!(is_truthy_symbol(&items[1]), "overflow flag must be set");
}

#[test]
fn test_gcd_min_and_zero_sets_overflow() {
    // gcd(i64::MIN, 0) = |i64::MIN|, which is unrepresentable.
    let input = r#"
        (PROGN
            (CLEAR-ALL-FLAGS)
            (LIST (GCD -9223372036854775808 0) (FLAG-SET-P 'OVERFLOW)))
    "#;
    let result = eval_str(input).unwrap();
    let items = lisp_list_to_vec(&result);
    // Wrapped-abs semantics (matches the +/-/* overflow idiom): the wrapped
    // magnitude of i64::MIN is i64::MIN itself.
    assert_eq!(items[0], LispVal::Number(i64::MIN));
    assert!(is_truthy_symbol(&items[1]), "overflow flag must be set");
}

#[test]
fn test_gcd_min_and_min_sets_overflow() {
    // gcd(i64::MIN, i64::MIN) also requires the unrepresentable |i64::MIN|.
    let input = r#"
        (PROGN
            (CLEAR-ALL-FLAGS)
            (LIST (GCD -9223372036854775808 -9223372036854775808) (FLAG-SET-P 'OVERFLOW)))
    "#;
    let result = eval_str(input).unwrap();
    let items = lisp_list_to_vec(&result);
    assert_eq!(items[0], LispVal::Number(i64::MIN));
    assert!(is_truthy_symbol(&items[1]), "overflow flag must be set");
}

#[test]
fn test_lcm_happy_path_no_overflow() {
    let input = r#"
        (PROGN
            (CLEAR-ALL-FLAGS)
            (LIST (LCM 4 6) (FLAG-SET-P 'OVERFLOW)))
    "#;
    let result = eval_str(input).unwrap();
    let items = lisp_list_to_vec(&result);
    assert_eq!(items[0], LispVal::Number(12));
    assert!(!is_truthy_symbol(&items[1]), "no overflow expected");
}

#[test]
fn test_lcm_zero_argument_no_overflow() {
    let input = r#"
        (PROGN
            (CLEAR-ALL-FLAGS)
            (LIST (LCM 0 9223372036854775807) (FLAG-SET-P 'OVERFLOW)))
    "#;
    let result = eval_str(input).unwrap();
    let items = lisp_list_to_vec(&result);
    assert_eq!(items[0], LispVal::Number(0));
    assert!(!is_truthy_symbol(&items[1]), "no overflow expected");
}

#[test]
fn test_lcm_large_coprime_inputs_sets_overflow() {
    // lcm of two large near-MAX coprime numbers overflows i64 during the
    // product step.
    let input = r#"
        (PROGN
            (CLEAR-ALL-FLAGS)
            (LCM 9223372036854775807 9223372036854775806)
            (FLAG-SET-P 'OVERFLOW))
    "#;
    let result = eval_str(input);
    assert!(result.is_ok(), "must not panic/abort: {result:?}");
    match result.unwrap() {
        LispVal::Symbol(s) => assert_eq!(s.borrow().name, "T"),
        _ => panic!("Expected T (overflow flag set)"),
    }
}

#[test]
fn test_lcm_min_operand_sets_overflow() {
    // lcm(i64::MIN, 3): gcd already overflows on the |i64::MIN| computation.
    let input = r#"
        (PROGN
            (CLEAR-ALL-FLAGS)
            (LCM -9223372036854775808 3)
            (FLAG-SET-P 'OVERFLOW))
    "#;
    let result = eval_str(input);
    assert!(result.is_ok(), "must not panic/abort: {result:?}");
    match result.unwrap() {
        LispVal::Symbol(s) => assert_eq!(s.borrow().name, "T"),
        _ => panic!("Expected T (overflow flag set)"),
    }
}

fn lisp_list_to_vec(val: &LispVal) -> Vec<LispVal> {
    let mut out = Vec::new();
    let mut cur = val.clone();
    loop {
        match cur {
            LispVal::Cons { car, cdr } => {
                out.push((*car).clone());
                cur = (*cdr).clone();
            }
            LispVal::Nil => break,
            other => {
                out.push(other);
                break;
            }
        }
    }
    out
}

fn is_truthy_symbol(val: &LispVal) -> bool {
    match val {
        LispVal::Symbol(s) => s.borrow().name == "T",
        _ => false,
    }
}
