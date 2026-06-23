use lamedh::{LispError, LispVal, eval_all, eval_line, eval_str, load_directory, load_file, repl_loop};
use lamedh::environment::Environment;
use std::collections::HashSet;
use std::io::BufReader;

// ---------------------------------------------------------------------------
// LispError::Display
// ---------------------------------------------------------------------------

#[test]
fn test_lisperror_display_generic() {
    let e = LispError::Generic("something went wrong".to_string());
    assert_eq!(format!("{e}"), "Error: something went wrong");
}

#[test]
fn test_lisperror_display_return() {
    let e = LispError::Return(Box::new(LispVal::Nil));
    assert_eq!(
        format!("{e}"),
        "Internal LispError: RETURN used outside of PROG."
    );
}

#[test]
fn test_lisperror_display_go() {
    let e = LispError::Go("LABEL".to_string());
    assert_eq!(
        format!("{e}"),
        "Internal LispError: GO used outside of PROG."
    );
}

// ---------------------------------------------------------------------------
// LispError::PartialEq
// ---------------------------------------------------------------------------

#[test]
fn test_lisperror_partialeq_generic_same() {
    let a = LispError::Generic("msg".to_string());
    let b = LispError::Generic("msg".to_string());
    assert_eq!(a, b);
}

#[test]
fn test_lisperror_partialeq_generic_different() {
    let a = LispError::Generic("foo".to_string());
    let b = LispError::Generic("bar".to_string());
    assert_ne!(a, b);
}

#[test]
fn test_lisperror_partialeq_return_equal() {
    let a = LispError::Return(Box::new(LispVal::Number(1)));
    let b = LispError::Return(Box::new(LispVal::Number(1)));
    assert_eq!(a, b);
}

#[test]
fn test_lisperror_partialeq_return_different() {
    let a = LispError::Return(Box::new(LispVal::Number(1)));
    let b = LispError::Return(Box::new(LispVal::Number(2)));
    assert_ne!(a, b);
}

#[test]
fn test_lisperror_partialeq_go_equal() {
    let a = LispError::Go("NEXT".to_string());
    let b = LispError::Go("NEXT".to_string());
    assert_eq!(a, b);
}

#[test]
fn test_lisperror_partialeq_go_different() {
    let a = LispError::Go("NEXT".to_string());
    let b = LispError::Go("DONE".to_string());
    assert_ne!(a, b);
}

#[test]
fn test_lisperror_partialeq_cross_variants() {
    let generic = LispError::Generic("x".to_string());
    let ret = LispError::Return(Box::new(LispVal::Nil));
    let go = LispError::Go("X".to_string());
    // Different variant pairs must not be equal.
    assert_ne!(generic, ret);
    assert_ne!(generic, go);
    assert_ne!(ret, go);
}

// ---------------------------------------------------------------------------
// LispVal::PartialEq — various branches
// ---------------------------------------------------------------------------

#[test]
fn test_lispval_number_eq() {
    assert_eq!(LispVal::Number(42), LispVal::Number(42));
    assert_ne!(LispVal::Number(1), LispVal::Number(2));
}

#[test]
fn test_lispval_float_eq() {
    assert_eq!(LispVal::Float(3.14), LispVal::Float(3.14));
    assert_ne!(LispVal::Float(1.0), LispVal::Float(2.0));
}

#[test]
fn test_lispval_string_eq() {
    assert_eq!(
        LispVal::String("hello".to_string()),
        LispVal::String("hello".to_string())
    );
    assert_ne!(
        LispVal::String("a".to_string()),
        LispVal::String("b".to_string())
    );
}

#[test]
fn test_lispval_nil_eq() {
    assert_eq!(LispVal::Nil, LispVal::Nil);
}

#[test]
fn test_lispval_cross_type_not_eq() {
    // Number vs Float — different variants → false
    assert_ne!(LispVal::Number(1), LispVal::Float(1.0));
    // Number vs Nil
    assert_ne!(LispVal::Number(0), LispVal::Nil);
    // String vs Nil
    assert_ne!(LispVal::String("".to_string()), LispVal::Nil);
    // Float vs Nil
    assert_ne!(LispVal::Float(0.0), LispVal::Nil);
    // Nil vs Number
    assert_ne!(LispVal::Nil, LispVal::Number(0));
}

#[test]
fn test_lispval_cons_eq() {
    let c1 = LispVal::Cons {
        car: Box::new(LispVal::Number(1)),
        cdr: Box::new(LispVal::Nil),
    };
    let c2 = LispVal::Cons {
        car: Box::new(LispVal::Number(1)),
        cdr: Box::new(LispVal::Nil),
    };
    assert_eq!(c1, c2);
}

#[test]
fn test_lispval_cons_neq_car() {
    let c1 = LispVal::Cons {
        car: Box::new(LispVal::Number(1)),
        cdr: Box::new(LispVal::Nil),
    };
    let c2 = LispVal::Cons {
        car: Box::new(LispVal::Number(2)),
        cdr: Box::new(LispVal::Nil),
    };
    assert_ne!(c1, c2);
}

#[test]
fn test_lispval_cons_neq_nil() {
    let c = LispVal::Cons {
        car: Box::new(LispVal::Number(1)),
        cdr: Box::new(LispVal::Nil),
    };
    assert_ne!(c, LispVal::Nil);
}

#[test]
fn test_lispval_builtin_eq() {
    use lamedh::BuiltinFunc;
    assert_eq!(LispVal::Builtin(BuiltinFunc::Plus), LispVal::Builtin(BuiltinFunc::Plus));
    assert_ne!(LispVal::Builtin(BuiltinFunc::Plus), LispVal::Builtin(BuiltinFunc::Minus));
}

#[test]
fn test_lispval_symbol_same_intern() {
    // Two intern_symbol calls with the same name return the same Rc, so ptr_eq is true.
    let env = Environment::new_with_builtins();
    let s1 = env.intern_symbol("MYSYM");
    let s2 = env.intern_symbol("MYSYM");
    let v1 = LispVal::Symbol(s1);
    let v2 = LispVal::Symbol(s2);
    assert_eq!(v1, v2);
}

#[test]
fn test_lispval_symbol_different_intern() {
    // Two differently-named symbols are distinct pointers → not equal.
    let env = Environment::new_with_builtins();
    let s1 = env.intern_symbol("FOO");
    let s2 = env.intern_symbol("BAR");
    let v1 = LispVal::Symbol(s1);
    let v2 = LispVal::Symbol(s2);
    assert_ne!(v1, v2);
}

#[test]
fn test_lispval_hashtable_pointer_equality() {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;

    // Two separately created hash tables are not equal (pointer comparison).
    let ht1 = LispVal::HashTable(Rc::new(RefCell::new(HashMap::new())));
    let ht2 = LispVal::HashTable(Rc::new(RefCell::new(HashMap::new())));
    assert_ne!(ht1, ht2);

    // A clone of the same Rc IS equal (same pointer).
    let ht3 = ht1.clone();
    assert_eq!(ht1, ht3);
}

// ---------------------------------------------------------------------------
// Lambda / Fexpr / Macro PartialEq
// ---------------------------------------------------------------------------

#[test]
fn test_lambda_partialeq_same_env() {
    use lamedh::LispVal;
    use std::rc::Rc;

    let env = Environment::new_with_builtins();
    let lam1 = lamedh::Lambda {
        params: vec!["X".to_string()],
        rest_param: None,
        body: Box::new(LispVal::Number(1)),
        env: Rc::clone(&env),
    };
    let lam2 = lamedh::Lambda {
        params: vec!["X".to_string()],
        rest_param: None,
        body: Box::new(LispVal::Number(1)),
        env: Rc::clone(&env),
    };
    assert_eq!(LispVal::Lambda(lam1), LispVal::Lambda(lam2));
}

#[test]
fn test_lambda_partialeq_different_env() {
    use lamedh::LispVal;
    use std::rc::Rc;

    // Two independently created environments → different Rc pointers → not equal.
    let env1 = Environment::new_with_builtins();
    let env2 = Environment::new_with_builtins();
    let lam1 = lamedh::Lambda {
        params: vec![],
        rest_param: None,
        body: Box::new(LispVal::Nil),
        env: Rc::clone(&env1),
    };
    let lam2 = lamedh::Lambda {
        params: vec![],
        rest_param: None,
        body: Box::new(LispVal::Nil),
        env: Rc::clone(&env2),
    };
    assert_ne!(LispVal::Lambda(lam1), LispVal::Lambda(lam2));
}

#[test]
fn test_fexpr_partialeq_same_env() {
    use std::rc::Rc;

    let env = Environment::new_with_builtins();
    let f1 = lamedh::Fexpr {
        params: vec!["ARGS".to_string()],
        body: Box::new(LispVal::Nil),
        env: Rc::clone(&env),
    };
    let f2 = lamedh::Fexpr {
        params: vec!["ARGS".to_string()],
        body: Box::new(LispVal::Nil),
        env: Rc::clone(&env),
    };
    assert_eq!(LispVal::Fexpr(f1), LispVal::Fexpr(f2));
}

#[test]
fn test_macro_partialeq_same_env() {
    use std::rc::Rc;

    let env = Environment::new_with_builtins();
    let m1 = lamedh::Macro {
        params: vec!["X".to_string()],
        rest_param: Some("REST".to_string()),
        body: Box::new(LispVal::Number(99)),
        env: Rc::clone(&env),
    };
    let m2 = lamedh::Macro {
        params: vec!["X".to_string()],
        rest_param: Some("REST".to_string()),
        body: Box::new(LispVal::Number(99)),
        env: Rc::clone(&env),
    };
    assert_eq!(LispVal::Macro(m1), LispVal::Macro(m2));
}

// ---------------------------------------------------------------------------
// LispVal::Hash — compile-and-run smoke tests
// ---------------------------------------------------------------------------

#[test]
fn test_lispval_hash_number() {
    let mut set = HashSet::new();
    set.insert(LispVal::Number(42));
    set.insert(LispVal::Number(42));
    set.insert(LispVal::Number(100));
    assert!(set.contains(&LispVal::Number(42)));
    assert_eq!(set.len(), 2);
}

#[test]
fn test_lispval_hash_float() {
    let mut set: HashSet<LispVal> = HashSet::new();
    set.insert(LispVal::Float(1.5));
    set.insert(LispVal::Float(2.5));
    assert!(set.contains(&LispVal::Float(1.5)));
}

#[test]
fn test_lispval_hash_string() {
    let mut set: HashSet<LispVal> = HashSet::new();
    set.insert(LispVal::String("hello".to_string()));
    assert!(set.contains(&LispVal::String("hello".to_string())));
}

#[test]
fn test_lispval_hash_nil() {
    let mut set: HashSet<LispVal> = HashSet::new();
    set.insert(LispVal::Nil);
    set.insert(LispVal::Nil);
    assert_eq!(set.len(), 1);
}

#[test]
fn test_lispval_hash_cons() {
    let mut set: HashSet<LispVal> = HashSet::new();
    let c = LispVal::Cons {
        car: Box::new(LispVal::Number(1)),
        cdr: Box::new(LispVal::Nil),
    };
    set.insert(c.clone());
    assert!(set.contains(&c));
}

#[test]
fn test_lispval_hash_symbol() {
    let env = Environment::new_with_builtins();
    let s = env.intern_symbol("HASHSYM");
    let v = LispVal::Symbol(s);
    let mut set: HashSet<LispVal> = HashSet::new();
    set.insert(v.clone());
    assert!(set.contains(&v));
}

#[test]
fn test_lispval_hash_hashtable_no_panic() {
    use std::cell::RefCell;
    use std::collections::HashMap;
    use std::rc::Rc;

    let ht = LispVal::HashTable(Rc::new(RefCell::new(HashMap::new())));
    let mut set: HashSet<LispVal> = HashSet::new();
    // Should not panic.
    set.insert(ht.clone());
    assert!(set.contains(&ht));
}

#[test]
fn test_lispval_hash_builtin_no_panic() {
    use lamedh::BuiltinFunc;
    // Builtin hash is a no-op, so two distinct builtins land in the same bucket.
    // Inserting should not panic.
    let mut set: HashSet<LispVal> = HashSet::new();
    set.insert(LispVal::Builtin(BuiltinFunc::Plus));
    set.insert(LispVal::Builtin(BuiltinFunc::Minus));
    // Both inserted (PartialEq distinguishes them even though hash collides).
    assert!(set.contains(&LispVal::Builtin(BuiltinFunc::Plus)));
}

#[test]
fn test_lispval_hash_lambda_no_panic() {
    use std::rc::Rc;

    let env = Environment::new_with_builtins();
    let lam = LispVal::Lambda(lamedh::Lambda {
        params: vec![],
        rest_param: None,
        body: Box::new(LispVal::Nil),
        env: Rc::clone(&env),
    });
    let mut set: HashSet<LispVal> = HashSet::new();
    set.insert(lam);
    // No panic is the goal.
}

// ---------------------------------------------------------------------------
// load_file — error path
// ---------------------------------------------------------------------------

#[test]
fn test_load_file_nonexistent() {
    let env = Environment::new_with_builtins();
    let result = load_file("/nonexistent/path/file.lisp", &env);
    assert!(result.is_err());
    match result.unwrap_err() {
        LispError::Generic(msg) => {
            assert!(
                msg.contains("Failed to read file"),
                "expected 'Failed to read file' in: {msg}"
            );
        }
        other => panic!("Expected Generic error, got: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// load_directory — error path
// ---------------------------------------------------------------------------

#[test]
fn test_load_directory_nonexistent() {
    let env = Environment::new_with_builtins();
    let result = load_directory("/nonexistent/dir", &env);
    assert!(result.is_err());
    match result.unwrap_err() {
        LispError::Generic(msg) => {
            assert!(
                msg.contains("Failed to read directory"),
                "expected 'Failed to read directory' in: {msg}"
            );
        }
        other => panic!("Expected Generic error, got: {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// repl_loop
// ---------------------------------------------------------------------------

#[test]
fn test_repl_loop_basic_addition() {
    let input = "(+ 1 2)\n";
    let mut reader = BufReader::new(input.as_bytes());
    let mut output = Vec::new();
    repl_loop(&mut reader, &mut output).unwrap();
    let output_str = String::from_utf8(output).unwrap();
    assert!(
        output_str.contains('3'),
        "expected '3' in output, got: {output_str:?}"
    );
}

#[test]
fn test_repl_loop_empty_input() {
    let input = "";
    let mut reader = BufReader::new(input.as_bytes());
    let mut output = Vec::new();
    repl_loop(&mut reader, &mut output).unwrap();
    let output_str = String::from_utf8(output).unwrap();
    // Nothing should have been written.
    assert!(output_str.is_empty());
}

#[test]
fn test_repl_loop_whitespace_only_lines_skipped() {
    let input = "   \n\t\n   \n";
    let mut reader = BufReader::new(input.as_bytes());
    let mut output = Vec::new();
    repl_loop(&mut reader, &mut output).unwrap();
    let output_str = String::from_utf8(output).unwrap();
    // Whitespace-only lines are skipped → no output.
    assert!(output_str.is_empty());
}

#[test]
fn test_repl_loop_multiple_expressions() {
    let input = "(+ 1 2)\n(* 3 4)\n";
    let mut reader = BufReader::new(input.as_bytes());
    let mut output = Vec::new();
    repl_loop(&mut reader, &mut output).unwrap();
    let output_str = String::from_utf8(output).unwrap();
    assert!(
        output_str.contains('3'),
        "expected '3' in output, got: {output_str:?}"
    );
    assert!(
        output_str.contains("12"),
        "expected '12' in output, got: {output_str:?}"
    );
}

#[test]
fn test_repl_loop_expression_error() {
    // (car) is missing an argument — should produce an error message, not panic.
    let input = "(car)\n";
    let mut reader = BufReader::new(input.as_bytes());
    let mut output = Vec::new();
    repl_loop(&mut reader, &mut output).unwrap();
    let output_str = String::from_utf8(output).unwrap();
    assert!(
        output_str.contains("Error"),
        "expected 'Error' in output, got: {output_str:?}"
    );
}

#[test]
fn test_repl_loop_returns_ok() {
    // Even a mix of valid and erroring lines completes without I/O failure.
    let input = "(+ 1 1)\n(bad-expr)\n(* 2 3)\n";
    let mut reader = BufReader::new(input.as_bytes());
    let mut output = Vec::new();
    let result = repl_loop(&mut reader, &mut output);
    assert!(result.is_ok());
}

// ---------------------------------------------------------------------------
// eval_str / eval_all / eval_line API tests
// ---------------------------------------------------------------------------

#[test]
fn test_eval_str_success() {
    let env = Environment::new_with_builtins();
    let result = eval_str("(+ 2 3)", &env);
    assert_eq!(result, Ok(LispVal::Number(5)));
}

#[test]
fn test_eval_str_error() {
    let env = Environment::new_with_builtins();
    let result = eval_str("(car)", &env);
    assert!(result.is_err(), "car with no args should error; got: {result:?}");
}

#[test]
fn test_eval_str_parse_error() {
    let env = Environment::new_with_builtins();
    let result = eval_str("(", &env);
    assert!(result.is_err(), "unclosed paren should error");
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("Parse error") || msg.contains("Error"), "got: {msg}");
}

#[test]
fn test_eval_all_multiple_forms() {
    let env = Environment::new_with_builtins();
    let result = eval_all("(+ 1 2) (* 3 4)", &env).expect("eval_all should succeed");
    assert_eq!(result.len(), 2);
    assert_eq!(result[0], LispVal::Number(3));
    assert_eq!(result[1], LispVal::Number(12));
}

#[test]
fn test_eval_all_single_form() {
    let env = Environment::new_with_builtins();
    let result = eval_all("(+ 10 20)", &env).expect("eval_all should succeed");
    assert_eq!(result, vec![LispVal::Number(30)]);
}

#[test]
fn test_eval_all_stops_on_first_error() {
    let env = Environment::new_with_builtins();
    let result = eval_all("(car) (+ 1 2)", &env);
    assert!(result.is_err(), "eval_all should stop on error");
}

#[test]
fn test_eval_all_parse_error() {
    let env = Environment::new_with_builtins();
    let result = eval_all("(", &env);
    assert!(result.is_err(), "unclosed paren should error");
}

#[test]
fn test_eval_all_empty() {
    let env = Environment::new_with_builtins();
    let result = eval_all("", &env).expect("empty input should return empty vec");
    assert!(result.is_empty());
}

#[test]
fn test_eval_line_simple() {
    let env = Environment::new_with_builtins();
    assert_eq!(eval_line("(+ 2 3)", &env), "5");
}

#[test]
fn test_eval_line_error_returns_string() {
    let env = Environment::new_with_builtins();
    let result = eval_line("(car)", &env);
    assert!(result.starts_with("Error"), "got: {result}");
}

// ---------------------------------------------------------------------------
// Environment::with_stdlib — embedded standard library
// ---------------------------------------------------------------------------

#[test]
fn test_with_stdlib_append() {
    // Acceptance criterion from issue #55: a test using only the library API
    // (no CWD files) can call (append (list 1) (list 2)).
    let env = Environment::with_stdlib();
    let result = eval_line("(append (list 1) (list 2))", &env);
    assert_eq!(result, "(1 2)", "append should work with embedded stdlib; got: {result}");
}

#[test]
fn test_with_stdlib_defun() {
    let env = Environment::with_stdlib();
    eval_line("(defun square (x) (* x x))", &env);
    let result = eval_line("(square 7)", &env);
    assert_eq!(result, "49", "defun should work with embedded stdlib; got: {result}");
}

#[test]
fn test_with_stdlib_equal() {
    let env = Environment::with_stdlib();
    let result = eval_line("(equal '(1 2 3) '(1 2 3))", &env);
    assert_eq!(result, "T", "equal should work with embedded stdlib; got: {result}");
}
