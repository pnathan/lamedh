/// Tests for LispVal::Native and Environment::register_fn (issue #54).
use lamedh::environment::Environment;
use lamedh::{LispVal, NativeFn, Shared, eval_line, with_large_stack};

// -------------------------------------------------------------------------
// Basic registration and call
// -------------------------------------------------------------------------

#[test]
fn test_register_fn_basic_call() {
    with_large_stack(|| {
        let env = Environment::new_with_builtins();
        env.register_fn("HOST-ADD", |args, _env| {
            if args.len() != 2 {
                return Err(lamedh::LispError::Generic(
                    "HOST-ADD requires 2 args".to_string(),
                ));
            }
            match (&args[0], &args[1]) {
                (LispVal::Number(a), LispVal::Number(b)) => Ok(LispVal::Number(a + b)),
                _ => Err(lamedh::LispError::Generic("integers required".to_string())),
            }
        });
        let result = eval_line("(host-add 3 4)", &env);
        assert_eq!(result, "7", "HOST-ADD 3 4 should return 7; got: {result}");
    });
}

#[test]
fn test_register_fn_name_uppercased() {
    with_large_stack(|| {
        let env = Environment::new_with_builtins();
        env.register_fn("my-fn", |_args, _env| Ok(LispVal::Number(42)));
        let result = eval_line("(my-fn)", &env);
        assert_eq!(
            result, "42",
            "lowercase name should be uppercased; got: {result}"
        );
    });
}

#[test]
fn test_register_fn_returns_string() {
    with_large_stack(|| {
        let env = Environment::new_with_builtins();
        env.register_fn("GREETING", |_args, _env| {
            Ok(LispVal::String("hello".to_string()))
        });
        let result = eval_line("(greeting)", &env);
        assert_eq!(result, "\"hello\"");
    });
}

#[test]
fn test_register_fn_propagates_error() {
    with_large_stack(|| {
        let env = Environment::new_with_builtins();
        env.register_fn("FAIL-FN", |_args, _env| {
            Err(lamedh::LispError::Generic(
                "intentional failure".to_string(),
            ))
        });
        let result = eval_line("(fail-fn)", &env);
        assert!(
            result.contains("Error"),
            "error from native fn should propagate; got: {result}"
        );
        assert!(result.contains("intentional failure"), "got: {result}");
    });
}

#[test]
fn test_register_fn_receives_evaluated_args() {
    with_large_stack(|| {
        let env = Environment::new_with_builtins();
        env.register_fn("DOUBLE", |args, _env| {
            if let Some(LispVal::Number(n)) = args.first() {
                Ok(LispVal::Number(n * 2))
            } else {
                Err(lamedh::LispError::Generic("number required".to_string()))
            }
        });
        // Arg is an expression that evaluates to 5
        let result = eval_line("(double (+ 2 3))", &env);
        assert_eq!(
            result, "10",
            "native fn should get evaluated args; got: {result}"
        );
    });
}

// -------------------------------------------------------------------------
// LispVal::Native type predicates and equality
// -------------------------------------------------------------------------

#[test]
fn test_native_is_functionp() {
    with_large_stack(|| {
        let env = Environment::new_with_builtins();
        env.register_fn("NOOP", |_args, _env| Ok(LispVal::Nil));
        // noop (unquoted) evaluates to the native fn value
        let result = eval_line("(functionp noop)", &env);
        assert_eq!(
            result, "T",
            "native fn should satisfy functionp; got: {result}"
        );
    });
}

#[test]
fn test_native_ptr_equality() {
    // Two independently created Shared<NativeFn> are not equal (pointer check).
    let f1: Shared<NativeFn> = Shared::new(|_args, _env| Ok(LispVal::Nil));
    let f2: Shared<NativeFn> = Shared::new(|_args, _env| Ok(LispVal::Nil));
    let v1 = LispVal::Native(f1.clone());
    let v2 = LispVal::Native(f2.clone());
    assert_ne!(v1, v2, "different native fns should not be equal");

    let v3 = LispVal::Native(f1.clone());
    assert_eq!(v1, v3, "same Shared should be equal");
}

#[test]
fn test_native_clone_shares_pointer() {
    let f: Shared<NativeFn> = Shared::new(|_args, _env| Ok(LispVal::Number(1)));
    let v1 = LispVal::Native(f.clone());
    let v2 = v1.clone();
    assert_eq!(v1, v2, "clone of Native should compare equal");
}

// -------------------------------------------------------------------------
// Printer
// -------------------------------------------------------------------------

#[test]
fn test_native_prints_as_native() {
    with_large_stack(|| {
        let env = Environment::new_with_builtins();
        env.register_fn("SHOW-ME", |_args, _env| Ok(LispVal::Nil));
        // Evaluating the symbol bound to a native fn should print "<native>"
        let result = eval_line("show-me", &env);
        assert_eq!(
            result, "<native>",
            "native fn should print as <native>; got: {result}"
        );
    });
}

// -------------------------------------------------------------------------
// Used in higher-order functions
// -------------------------------------------------------------------------

#[test]
fn test_native_in_mapcar() {
    with_large_stack(|| {
        let env = Environment::new_with_builtins();
        env.register_fn("INC", |args, _env| {
            if let Some(LispVal::Number(n)) = args.first() {
                Ok(LispVal::Number(n + 1))
            } else {
                Err(lamedh::LispError::Generic("number required".to_string()))
            }
        });
        // inc (unquoted) evaluates to the native fn value
        let result = eval_line("(mapcar inc '(1 2 3))", &env);
        assert_eq!(result, "(2 3 4)", "mapcar with native fn; got: {result}");
    });
}

#[test]
fn test_native_in_funcall() {
    with_large_stack(|| {
        let env = Environment::new_with_builtins();
        env.register_fn("SQUARE", |args, _env| {
            if let Some(LispVal::Number(n)) = args.first() {
                Ok(LispVal::Number(n * n))
            } else {
                Err(lamedh::LispError::Generic("number required".to_string()))
            }
        });
        let result = eval_line("(funcall 'square 5)", &env);
        assert_eq!(result, "25", "funcall with native fn; got: {result}");
    });
}
