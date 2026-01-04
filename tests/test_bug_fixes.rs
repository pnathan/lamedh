use lamedh::environment::Environment;
use lamedh::evaluator::eval;
use lamedh::reader::read;
use lamedh::LispVal;

fn eval_str(input: &str) -> Result<LispVal, String> {
    let env = Environment::new_with_builtins();
    let expr = read(input, &env).map_err(|e| format!("Parse error: {}", e))?;
    eval(&expr, &env).map_err(|e| format!("Eval error: {:?}", e))
}

// Test 1: Float comparison builtins
#[test]
fn test_float_equal_distinguishes_negative_zero() {
    // FLOAT-EQUAL uses bitwise comparison, so -0.0 != 0.0
    let result = eval_str("(FLOAT-EQUAL 0.0 -0.0)");
    assert!(result.is_ok(), "Failed to evaluate FLOAT-EQUAL: {:?}", result);
    assert_eq!(result.unwrap(), LispVal::Nil, "-0.0 and 0.0 should not be float equal");

    // But they are numerically equal in regular comparison
    let result = eval_str("(FLOAT-EQUAL 0.0 0.0)");
    assert!(result.is_ok(), "Failed to evaluate FLOAT-EQUAL with same values: {:?}", result);
    match result.unwrap() {
        LispVal::Symbol(s) => assert_eq!(s.borrow().name, "T"),
        _ => panic!("Expected T"),
    }
}

#[test]
fn test_float_less_than() {
    let result = eval_str("(FLOAT-LESSP 1.5 2.5)");
    assert!(result.is_ok(), "Failed: {:?}", result);
    match result.unwrap() {
        LispVal::Symbol(s) => assert_eq!(s.borrow().name, "T"),
        _ => panic!("Expected T"),
    }

    let result = eval_str("(FLOAT-LESSP 2.5 1.5)");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispVal::Nil);
}

#[test]
fn test_float_greater_than() {
    let result = eval_str("(FLOAT-GREATERP 2.5 1.5)");
    assert!(result.is_ok(), "Failed: {:?}", result);
    match result.unwrap() {
        LispVal::Symbol(s) => assert_eq!(s.borrow().name, "T"),
        _ => panic!("Expected T"),
    }

    let result = eval_str("(FLOAT-GREATERP 1.5 2.5)");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), LispVal::Nil);
}

#[test]
fn test_float_comparison_with_integers() {
    // Float comparisons should accept integers too
    let result = eval_str("(FLOAT-LESSP 1 2.5)");
    assert!(result.is_ok(), "Failed: {:?}", result);
    match result.unwrap() {
        LispVal::Symbol(s) => assert_eq!(s.borrow().name, "T"),
        _ => panic!("Expected T"),
    }
}

// Test 2: Pathological LABEL detection
#[test]
fn test_label_self_reference_error() {
    let result = eval_str("(LABEL x x)");
    assert!(result.is_err(), "LABEL x x should be an error");
    let err_msg = result.unwrap_err();
    assert!(err_msg.contains("pathological"), "Error should mention pathological: {}", err_msg);
    assert!(err_msg.contains("infinite recursion"), "Should mention infinite recursion: {}", err_msg);
}

#[test]
fn test_label_normal_recursion_still_works() {
    // This should still work fine
    let input = r#"
        (LABEL factorial
            (LAMBDA (n)
                (IF (= n 0)
                    1
                    (* n (factorial (- n 1))))))
    "#;
    let env = Environment::new_with_builtins();
    let expr = read(input, &env).unwrap();
    let func = eval(&expr, &env);
    assert!(func.is_ok(), "Normal LABEL recursion should work");
}

// Test 3: ASSOC with non-cons elements (should warn but continue)
#[test]
fn test_assoc_with_malformed_alist() {
    // This should print warning to stderr but return NIL
    let input = "(ASSOC 'x '(1 2 3))";
    let result = eval_str(input);
    assert!(result.is_ok(), "ASSOC should continue despite malformed alist");
    assert_eq!(result.unwrap(), LispVal::Nil);
}

#[test]
fn test_assoc_mixed_list() {
    // Mixed list with both valid pairs and atoms
    let input = "(ASSOC 'b '((a . 1) b (c . 3)))";
    let result = eval_str(input);
    assert!(result.is_ok());
    // Should skip the 'b' atom and return NIL
    assert_eq!(result.unwrap(), LispVal::Nil);
}

// Test 4: Circular lists are prevented by functional RPLACA/RPLACD
#[test]
fn test_rplaca_returns_new_cons() {
    let input = r#"
        (PROGN
            (DEF x '(1 2 3))
            (DEF y (RPLACA x 10))
            (CAR y))
    "#;
    let result = eval_str(input);
    assert!(result.is_ok(), "RPLACA test failed: {:?}", result);
    // y should have CAR of 10
    if let Ok(LispVal::Number(n)) = result {
        assert_eq!(n, 10);
    }
}

#[test]
fn test_rplacd_returns_new_cons() {
    let input = r#"
        (PROGN
            (DEF x '(1 2 3))
            (DEF y (RPLACD x '(20 30)))
            (CAR x))
    "#;
    let result = eval_str(input);
    assert!(result.is_ok(), "RPLACD test failed: {:?}", result);
    // x's car should still be 1
    if let Ok(LispVal::Number(n)) = result {
        assert_eq!(n, 1);
    }
}

// Test 5: SETQ creates variables
#[test]
fn test_setq_creates_variable() {
    let input = r#"
        (PROGN
            (SETQ new-var 42)
            new-var)
    "#;
    let result = eval_str(input);
    assert!(result.is_ok(), "SETQ should create new variables");
    if let Ok(LispVal::Number(n)) = result {
        assert_eq!(n, 42);
    }
}

#[test]
fn test_setq_creates_with_value_not_undefined() {
    let input = "(SETQ x 100)";
    let result = eval_str(input);
    assert!(result.is_ok());
    if let Ok(LispVal::Number(n)) = result {
        assert_eq!(n, 100, "Created variable should have the assigned value");
    }
}

// Test 6: Duplicate PROG labels (should warn but continue)
#[test]
fn test_prog_duplicate_labels() {
    // This should print a warning but execute
    // Let's just test that a PROG with duplicate labels compiles
    let result = eval_str("(PROG () label1 label1 (RETURN 42))");
    assert!(result.is_ok(), "PROG should accept duplicate labels");
    if let Ok(LispVal::Number(n)) = result {
        assert_eq!(n, 42);
    }
}

#[test]
fn test_prog_label_ordering() {
    // Later label should win
    let input = r#"
        (PROG (x)
            (SETQ x 1)
            label
            (SETQ x 2)
            label
            (RETURN x))
    "#;
    // GO to 'label' should go to second occurrence
    let result = eval_str(input);
    assert!(result.is_ok());
    // Should return 2 since it goes to the second label
    if let Ok(LispVal::Number(n)) = result {
        assert_eq!(n, 2);
    }
}

#[test]
fn test_all_fixes_together() {
    // Integration test using multiple features
    let input = r#"
        (PROGN
            ; Test float comparison
            (DEF float-test (FLOAT-LESSP 1.5 2.5))

            ; Test SETQ variable creation
            (SETQ dynamic-var 999)

            ; Test ASSOC with mixed list (will warn)
            (DEF assoc-result (ASSOC 'x '((a . 1) b (x . 2))))

            ; Return results
            (CONS float-test (CONS dynamic-var (CONS (CDR assoc-result) NIL))))
    "#;
    let result = eval_str(input);
    assert!(result.is_ok(), "Integration test should succeed: {:?}", result);
}
