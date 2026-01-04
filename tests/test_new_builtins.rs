use lamedh::environment::Environment;
use lamedh::{eval_line, load_directory};

fn eval(expr: &str) -> String {
    let env = Environment::new_with_builtins();
    // Load the standard library
    load_directory("lib", &env).expect("Failed to load lib directory");
    eval_line(expr, &env)
}

fn eval_with_env(expr: &str, env: &std::rc::Rc<Environment>) -> String {
    eval_line(expr, env)
}

// ============================================================================
// LIST OPERATIONS
// ============================================================================

#[test]
fn test_list_empty() {
    assert_eq!(eval("(list)"), "()");
}

#[test]
fn test_list_single() {
    assert_eq!(eval("(list 1)"), "(1)");
}

#[test]
fn test_list_multiple() {
    assert_eq!(eval("(list 1 2 3)"), "(1 2 3)");
}

#[test]
fn test_list_nested() {
    assert_eq!(eval("(list 'a (list 'b 'c) 'd)"), "(A (B C) D)");
}

#[test]
fn test_last_simple() {
    assert_eq!(eval("(last '(1 2 3))"), "(3)");
}

#[test]
fn test_last_single() {
    assert_eq!(eval("(last '(1))"), "(1)");
}

#[test]
fn test_last_empty() {
    assert_eq!(eval("(last '())"), "()");
}

#[test]
fn test_nth_first() {
    assert_eq!(eval("(nth 0 '(a b c d))"), "A");
}

#[test]
fn test_nth_middle() {
    assert_eq!(eval("(nth 2 '(a b c d))"), "C");
}

#[test]
fn test_nth_last() {
    assert_eq!(eval("(nth 3 '(a b c d))"), "D");
}

#[test]
fn test_nth_out_of_bounds() {
    assert_eq!(eval("(nth 10 '(a b c))"), "()");
}

#[test]
fn test_nthcdr_zero() {
    assert_eq!(eval("(nthcdr 0 '(a b c d))"), "(A B C D)");
}

#[test]
fn test_nthcdr_middle() {
    assert_eq!(eval("(nthcdr 2 '(a b c d))"), "(C D)");
}

#[test]
fn test_nthcdr_all() {
    assert_eq!(eval("(nthcdr 4 '(a b c d))"), "()");
}

#[test]
fn test_nthcdr_out_of_bounds() {
    assert_eq!(eval("(nthcdr 10 '(a b c))"), "()");
}

#[test]
fn test_efface_first() {
    assert_eq!(eval("(efface 'a '(a b c))"), "(B C)");
}

#[test]
fn test_efface_middle() {
    assert_eq!(eval("(efface 'b '(a b c))"), "(A C)");
}

#[test]
fn test_efface_last() {
    assert_eq!(eval("(efface 'c '(a b c))"), "(A B)");
}

#[test]
fn test_efface_not_found() {
    assert_eq!(eval("(efface 'x '(a b c))"), "(A B C)");
}

#[test]
fn test_efface_only_first() {
    assert_eq!(eval("(efface 'b '(a b c b d))"), "(A C B D)");
}

#[test]
fn test_delete_alias() {
    assert_eq!(eval("(delete 'b '(a b c))"), "(A C)");
}

// ============================================================================
// NUMERIC OPERATIONS
// ============================================================================

#[test]
fn test_mod_positive() {
    assert_eq!(eval("(mod 7 3)"), "1");
}

#[test]
fn test_mod_exact() {
    assert_eq!(eval("(mod 6 3)"), "0");
}

#[test]
fn test_mod_negative_dividend() {
    // MOD uses floored division, so -7 mod 3 = 2 (not -1 like remainder)
    assert_eq!(eval("(mod -7 3)"), "2");
}

#[test]
fn test_mod_negative_divisor() {
    // rem_euclid always returns non-negative result
    // 7 = -3 * (-2) + 1, so result is 1
    assert_eq!(eval("(mod 7 -3)"), "1");
}

#[test]
fn test_remainder_vs_mod() {
    // Remainder and mod differ for negative numbers
    assert_eq!(eval("(remainder -7 3)"), "-1");
    assert_eq!(eval("(mod -7 3)"), "2");
}

#[test]
fn test_plusp_positive() {
    assert_eq!(eval("(plusp 5)"), "T");
}

#[test]
fn test_plusp_zero() {
    assert_eq!(eval("(plusp 0)"), "()");
}

#[test]
fn test_plusp_negative() {
    assert_eq!(eval("(plusp -5)"), "()");
}

#[test]
fn test_plusp_float() {
    assert_eq!(eval("(plusp 0.5)"), "T");
}

#[test]
fn test_evenp_even() {
    assert_eq!(eval("(evenp 4)"), "T");
}

#[test]
fn test_evenp_odd() {
    assert_eq!(eval("(evenp 5)"), "()");
}

#[test]
fn test_evenp_zero() {
    assert_eq!(eval("(evenp 0)"), "T");
}

#[test]
fn test_evenp_negative() {
    assert_eq!(eval("(evenp -4)"), "T");
}

#[test]
fn test_oddp_odd() {
    assert_eq!(eval("(oddp 5)"), "T");
}

#[test]
fn test_oddp_even() {
    assert_eq!(eval("(oddp 4)"), "()");
}

#[test]
fn test_oddp_negative() {
    assert_eq!(eval("(oddp -3)"), "T");
}

#[test]
fn test_add1_integer() {
    assert_eq!(eval("(add1 5)"), "6");
}

#[test]
fn test_add1_negative() {
    assert_eq!(eval("(add1 -1)"), "0");
}

#[test]
fn test_add1_float() {
    assert_eq!(eval("(add1 1.5)"), "2.5");
}

#[test]
fn test_sub1_integer() {
    assert_eq!(eval("(sub1 5)"), "4");
}

#[test]
fn test_sub1_zero() {
    assert_eq!(eval("(sub1 0)"), "-1");
}

#[test]
fn test_sub1_float() {
    assert_eq!(eval("(sub1 1.5)"), "0.5");
}

#[test]
fn test_1_plus_symbol() {
    assert_eq!(eval("(1+ 5)"), "6");
}

#[test]
fn test_1_minus_symbol() {
    assert_eq!(eval("(1- 5)"), "4");
}

#[test]
fn test_random_range() {
    let env = Environment::new_with_builtins();
    let result = eval_with_env("(random 100)", &env);
    let num: i64 = result.parse().expect("Expected a number");
    assert!(num >= 0 && num < 100);
}

// ============================================================================
// TYPE PREDICATES
// ============================================================================

#[test]
fn test_symbolp_symbol() {
    assert_eq!(eval("(symbolp 'foo)"), "T");
}

#[test]
fn test_symbolp_number() {
    assert_eq!(eval("(symbolp 42)"), "()");
}

#[test]
fn test_symbolp_string() {
    assert_eq!(eval("(symbolp \"hello\")"), "()");
}

#[test]
fn test_symbolp_list() {
    assert_eq!(eval("(symbolp '(a b))"), "()");
}

#[test]
fn test_symbolp_nil() {
    assert_eq!(eval("(symbolp nil)"), "()");
}

#[test]
fn test_boundp_bound() {
    assert_eq!(eval("(boundp 'car)"), "T");
}

#[test]
fn test_boundp_unbound() {
    assert_eq!(eval("(boundp 'undefined-xyz-123)"), "()");
}

#[test]
fn test_boundp_t() {
    assert_eq!(eval("(boundp 't)"), "T");
}

#[test]
fn test_functionp_builtin() {
    assert_eq!(eval("(functionp car)"), "T");
}

#[test]
fn test_functionp_lambda() {
    assert_eq!(eval("(functionp (lambda (x) x))"), "T");
}

#[test]
fn test_functionp_number() {
    assert_eq!(eval("(functionp 42)"), "()");
}

#[test]
fn test_functionp_symbol() {
    assert_eq!(eval("(functionp 'foo)"), "()");
}

#[test]
fn test_macrop_macro() {
    let env = Environment::new_with_builtins();
    eval_with_env("(defmacro test-macro (x) x)", &env);
    assert_eq!(eval_with_env("(macrop test-macro)", &env), "T");
}

#[test]
fn test_macrop_function() {
    assert_eq!(eval("(macrop car)"), "()");
}

#[test]
fn test_macrop_lambda() {
    assert_eq!(eval("(macrop (lambda (x) x))"), "()");
}

// ============================================================================
// FUNCTION OPERATIONS
// ============================================================================

#[test]
fn test_funcall_builtin() {
    assert_eq!(eval("(funcall '+ 1 2 3)"), "6");
}

#[test]
fn test_funcall_with_function_value() {
    assert_eq!(eval("(funcall + 1 2 3)"), "6");
}

#[test]
fn test_funcall_lambda() {
    assert_eq!(eval("(funcall (lambda (x y) (+ x y)) 3 4)"), "7");
}

#[test]
fn test_funcall_car() {
    assert_eq!(eval("(funcall 'car '(a b c))"), "A");
}

#[test]
fn test_funcall_cons() {
    assert_eq!(eval("(funcall 'cons 'a '(b c))"), "(A B C)");
}

#[test]
fn test_macroexpand_non_macro() {
    assert_eq!(eval("(macroexpand '(+ 1 2))"), "(+ 1 2)");
}

#[test]
fn test_macroexpand_macro() {
    let env = Environment::new_with_builtins();
    eval_with_env("(defmacro my-when (test expr) `(if ,test ,expr nil))", &env);
    assert_eq!(
        eval_with_env("(macroexpand '(my-when t (print 1)))", &env),
        "(IF T (PRINT 1) ())"
    );
}

// ============================================================================
// STRING/SYMBOL OPERATIONS
// ============================================================================

#[test]
fn test_explode_symbol() {
    assert_eq!(eval("(explode 'foo)"), "(F O O)");
}

#[test]
fn test_explode_string() {
    // String explode preserves case (doesn't uppercase like symbols)
    assert_eq!(eval("(explode \"hello\")"), "(h e l l o)");
}

#[test]
fn test_explode_number() {
    assert_eq!(eval("(explode 123)"), "(1 2 3)");
}

#[test]
fn test_explode_single_char() {
    assert_eq!(eval("(explode 'x)"), "(X)");
}

#[test]
fn test_implode_simple() {
    assert_eq!(eval("(implode '(f o o))"), "FOO");
}

#[test]
fn test_implode_single() {
    assert_eq!(eval("(implode '(x))"), "X");
}

#[test]
fn test_implode_empty() {
    assert_eq!(eval("(implode '())"), "");
}

#[test]
fn test_maknam_simple() {
    assert_eq!(eval("(maknam '(b a r))"), "BAR");
}

#[test]
fn test_explode_implode_roundtrip() {
    assert_eq!(eval("(implode (explode 'hello))"), "HELLO");
}

#[test]
fn test_gensym_unique() {
    let env = Environment::new_with_builtins();
    let g1 = eval_with_env("(gensym)", &env);
    let g2 = eval_with_env("(gensym)", &env);
    assert_ne!(g1, g2);
}

#[test]
fn test_gensym_format() {
    let result = eval("(gensym)");
    assert!(result.starts_with("G"));
}

#[test]
fn test_intern_string() {
    assert_eq!(eval("(intern \"hello\")"), "HELLO");
}

#[test]
fn test_intern_uppercase() {
    assert_eq!(eval("(intern \"WORLD\")"), "WORLD");
}

#[test]
fn test_intern_symbol() {
    assert_eq!(eval("(intern 'foo)"), "FOO");
}

#[test]
fn test_plist_empty() {
    assert_eq!(eval("(plist 'new-symbol-xyz)"), "()");
}

#[test]
fn test_plist_with_properties() {
    let env = Environment::new_with_builtins();
    eval_with_env("(putp 'test-sym \"prop1\" 123)", &env);
    let result = eval_with_env("(plist 'test-sym)", &env);
    assert!(result.contains("prop1"));
    assert!(result.contains("123"));
}

// ============================================================================
// BITWISE OPERATIONS
// ============================================================================

#[test]
fn test_ash_left() {
    assert_eq!(eval("(ash 1 4)"), "16");
}

#[test]
fn test_ash_right() {
    assert_eq!(eval("(ash 16 -2)"), "4");
}

#[test]
fn test_ash_zero() {
    assert_eq!(eval("(ash 5 0)"), "5");
}

#[test]
fn test_ash_large_shift() {
    assert_eq!(eval("(ash 1 10)"), "1024");
}

#[test]
fn test_lognot_zero() {
    assert_eq!(eval("(lognot 0)"), "-1");
}

#[test]
fn test_lognot_positive() {
    assert_eq!(eval("(lognot 1)"), "-2");
}

#[test]
fn test_lognot_negative() {
    assert_eq!(eval("(lognot -1)"), "0");
}

#[test]
fn test_lognot_double() {
    assert_eq!(eval("(lognot (lognot 42))"), "42");
}

#[test]
fn test_rot_left() {
    // Rotate 1 left by 4 positions
    assert_eq!(eval("(rot 1 4)"), "16");
}

#[test]
fn test_rot_zero() {
    assert_eq!(eval("(rot 5 0)"), "5");
}

// ============================================================================
// PUT ALIAS
// ============================================================================

#[test]
fn test_put_alias() {
    let env = Environment::new_with_builtins();
    eval_with_env("(put 'my-sym \"indicator\" 'value)", &env);
    assert_eq!(eval_with_env("(getp 'my-sym \"indicator\")", &env), "VALUE");
}

// ============================================================================
// ERROR HANDLING
// ============================================================================

#[test]
fn test_nth_wrong_arg_type() {
    let result = eval("(nth 'x '(a b c))");
    assert!(result.contains("Error"));
}

#[test]
fn test_mod_division_by_zero() {
    let result = eval("(mod 5 0)");
    assert!(result.contains("Error"));
}

#[test]
fn test_boundp_non_symbol() {
    let result = eval("(boundp 42)");
    assert!(result.contains("Error"));
}

#[test]
fn test_funcall_no_args() {
    let result = eval("(funcall)");
    assert!(result.contains("Error"));
}

#[test]
fn test_random_negative() {
    let result = eval("(random -5)");
    assert!(result.contains("Error"));
}

#[test]
fn test_random_zero() {
    let result = eval("(random 0)");
    assert!(result.contains("Error"));
}
