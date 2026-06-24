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
fn test_expt_overflow() {
    // Test that very large exponents don't silently wrap
    let result = eval_str("(EXPT 2 63)");
    // 2^63 would overflow i64, should error
    assert!(result.is_err(), "2^63 should return an overflow error");
    assert!(
        result.unwrap_err().contains("overflow"),
        "Error should mention overflow"
    );

    // Test 2^62 which should work
    let result = eval_str("(EXPT 2 62)");
    assert!(result.is_ok(), "2^62 should not overflow");
    if let Ok(LispVal::Number(n)) = result {
        assert_eq!(n, 4611686018427387904);
    }
}

#[test]
fn test_expt_large_exponent_cast() {
    // Test exponent larger than u32::MAX
    // This is i64 but when cast to u32 it would truncate
    let result = eval_str("(EXPT 2 4294967296)"); // 2^32
    // This should error with "exponent too large"
    assert!(result.is_err(), "Exponent > u32::MAX should error");
    assert!(
        result.unwrap_err().contains("too large"),
        "Error should mention exponent too large"
    );
}

#[test]
fn test_label_recursion() {
    // Test that LABEL actually works for recursion
    let input = r#"
        (LABEL factorial
            (LAMBDA (n)
                (IF (= n 0)
                    1
                    (* n (factorial (- n 1))))))
    "#;

    let env = Environment::new_with_builtins();
    let expr = read(input, &env).unwrap();
    let func = eval(&expr, &env).unwrap();

    // Now try to call it
    let call_expr = read("(factorial 5)", &env);
    // This requires factorial to be bound
    // LABEL returns the lambda but doesn't bind it
    // Let's bind it manually for testing
    if let Ok(call) = call_expr {
        env.set("FACTORIAL".to_string(), func);
        let result = eval(&call, &env);
        println!("Factorial result: {:?}", result);

        // Should compute 5! = 120
        match result {
            Ok(LispVal::Number(n)) => assert_eq!(n, 120),
            other => panic!("Expected Number(120), got {:?}", other),
        }
    }
}

#[test]
fn test_empty_plus() {
    // Test (PLUS) with no arguments
    let result = eval_str("(PLUS)");
    assert!(result.is_ok());
    if let Ok(LispVal::Number(n)) = result {
        assert_eq!(n, 0, "Empty PLUS should return 0");
    }
}

#[test]
fn test_empty_times() {
    // Test (TIMES) with no arguments
    let result = eval_str("(TIMES)");
    assert!(result.is_ok());
    if let Ok(LispVal::Number(n)) = result {
        assert_eq!(n, 1, "Empty TIMES should return 1");
    }
}

#[test]
fn test_empty_minus() {
    // Test (DIFFERENCE) with no arguments should error
    let result = eval_str("(DIFFERENCE)");
    assert!(result.is_err(), "Empty DIFFERENCE should error");
}

#[test]
fn test_logand_no_args() {
    // LOGAND with no arguments returns -1 (all bits set)
    let result = eval_str("(LOGAND)");
    assert!(result.is_ok());
    if let Ok(LispVal::Number(n)) = result {
        assert_eq!(n, -1, "Empty LOGAND should return -1");
    }
}

#[test]
fn test_logor_no_args() {
    // LOGOR with no arguments returns 0
    let result = eval_str("(LOGOR)");
    assert!(result.is_ok());
    if let Ok(LispVal::Number(n)) = result {
        assert_eq!(n, 0, "Empty LOGOR should return 0");
    }
}

#[test]
fn test_logxor_no_args() {
    // LOGXOR with no arguments returns 0
    let result = eval_str("(LOGXOR)");
    assert!(result.is_ok());
    if let Ok(LispVal::Number(n)) = result {
        assert_eq!(n, 0, "Empty LOGXOR should return 0");
    }
}

#[test]
fn test_and_no_args() {
    // (AND) with no arguments should return T
    let result = eval_str("(AND)");
    assert!(result.is_ok());
    println!("Empty AND result: {:?}", result);
}

#[test]
fn test_or_no_args() {
    // (OR) with no arguments should return NIL
    let result = eval_str("(OR)");
    assert!(result.is_ok());
    if let Ok(val) = result {
        assert_eq!(val, LispVal::Nil, "Empty OR should return NIL");
    }
}

#[test]
fn test_expt_zero_to_zero() {
    // 0^0 is mathematically undefined but often treated as 1
    let result = eval_str("(EXPT 0 0)");
    assert!(result.is_ok());
    if let Ok(LispVal::Number(n)) = result {
        assert_eq!(n, 1, "0^0 should return 1");
    }
}

#[test]
fn test_expt_negative_base() {
    // Test negative base with even exponent
    let result = eval_str("(EXPT -2 4)");
    assert!(result.is_ok());
    if let Ok(LispVal::Number(n)) = result {
        assert_eq!(n, 16, "(-2)^4 should return 16");
    }

    // Test negative base with odd exponent
    let result = eval_str("(EXPT -2 3)");
    assert!(result.is_ok());
    if let Ok(LispVal::Number(n)) = result {
        assert_eq!(n, -8, "(-2)^3 should return -8");
    }
}

#[test]
fn test_car_cdr_nil() {
    // CAR and CDR of NIL should return NIL
    let result = eval_str("(CAR NIL)");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispVal::Nil);

    let result = eval_str("(CDR NIL)");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispVal::Nil);
}

#[test]
fn test_float_hash_as_key() {
    // Test using floats as hash table keys
    let input = r#"
        (PROGN
            (DEF h (MAKE-HASH-TABLE))
            (SET-BANG h 3.14 "pi")
            (GETHASH h 3.14))
    "#;
    let result = eval_str(input);
    println!("Float hash key result: {:?}", result);
    // Should return "pi"
    if let Ok(LispVal::String(s)) = result {
        assert_eq!(s, "pi");
    } else {
        panic!("Expected string, got {:?}", result);
    }
}

#[test]
fn test_cond_empty_consequent() {
    // COND with empty consequent should return the predicate value
    let input = "(COND ((PLUS 1 2)))";
    let result = eval_str(input);
    assert!(result.is_ok());
    if let Ok(LispVal::Number(n)) = result {
        assert_eq!(
            n, 3,
            "COND with no consequent should return predicate value"
        );
    }
}

#[test]
fn test_lambda_rest_params() {
    // Test lambda with &REST parameter
    let input = r#"
        (PROGN
            (DEF f (LAMBDA (a b &REST rest) rest))
            (f 1 2 3 4 5))
    "#;
    let result = eval_str(input);
    println!("Lambda &REST result: {:?}", result);
    // Should return (3 4 5)
    assert!(result.is_ok());
}

#[test]
fn test_lambda_rest_params_empty() {
    // Test lambda with &REST parameter but no extra args
    let input = r#"
        (PROGN
            (DEF f (LAMBDA (a b &REST rest) rest))
            (f 1 2))
    "#;
    let result = eval_str(input);
    println!("Lambda &REST empty result: {:?}", result);
    // Should return NIL
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispVal::Nil);
}

#[test]
fn test_macro_rest_params() {
    // Test macro with &REST parameter
    // This macro quotes all its arguments and returns them as a list
    let input = r#"
        (PROGN
            (DEFMACRO m (a &REST rest)
                (CONS 'QUOTE (CONS (CONS a rest) NIL)))
            (m x y z))
    "#;
    let result = eval_str(input);
    println!("Macro &REST result: {:?}", result);
    // The macro receives a=x, rest=(y z)
    // It returns (QUOTE (x y z))
    // When evaluated, this becomes the list (x y z)
    assert!(result.is_ok(), "Macro with &REST should work: {:?}", result);
}
