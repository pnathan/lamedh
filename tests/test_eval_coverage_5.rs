/// Tests for remaining uncovered error branches in evaluator.rs and built-in functions.
/// All tests assert that the given expression produces an error result.
mod test_helpers;
use lamedh::eval_line;
use test_helpers::env_with_stdlib;

// ============================================================================
// Group 1: Special form error edges
// ============================================================================

/// DEFEXPR with a non-symbol (integer) as its name should error.
#[test]
fn test_defexpr_non_symbol_name_error() {
    let env = env_with_stdlib();
    let result = eval_line("(defexpr 123 (x) x)", &env);
    assert!(
        result.contains("Error"),
        "expected error for DEFEXPR with non-symbol name, got: {result}"
    );
}

/// DEFMACRO with a non-symbol (integer) as its name should error.
#[test]
fn test_defmacro_non_symbol_name_error() {
    let env = env_with_stdlib();
    let result = eval_line("(defmacro 456 (x) x)", &env);
    assert!(
        result.contains("Error"),
        "expected error for DEFMACRO with non-symbol name, got: {result}"
    );
}

/// QUOTE with two arguments (wrong arity) should error.
#[test]
fn test_quote_two_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(eval '(quote a b))", &env);
    assert!(
        result.contains("Error"),
        "expected error for (quote a b) — quote takes exactly one argument, got: {result}"
    );
}

/// QUOTE with zero arguments should error.
#[test]
fn test_quote_zero_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(eval '(quote))", &env);
    assert!(
        result.contains("Error"),
        "expected error for (quote) — quote takes exactly one argument, got: {result}"
    );
}

/// QUASIQUOTE with two arguments (wrong arity) should error.
#[test]
fn test_quasiquote_two_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(eval '(quasiquote a b))", &env);
    assert!(
        result.contains("Error"),
        "expected error for (quasiquote a b) — quasiquote takes exactly one argument, got: {result}"
    );
}

/// LAMBDA with no arguments at all should error.
#[test]
fn test_lambda_no_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(lambda)", &env);
    assert!(
        result.contains("Error"),
        "expected error for (lambda) — requires params and body, got: {result}"
    );
}

/// FUNCTION with no arguments should error.
#[test]
fn test_function_no_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(function)", &env);
    assert!(
        result.contains("Error"),
        "expected error for (function) — FUNCTION takes exactly one argument, got: {result}"
    );
}

// ============================================================================
// Group 2: Lambda/macro arity errors with &REST
// ============================================================================

/// Lambda with &REST requires the fixed positional args to be satisfied.
/// Providing fewer fixed args than required should error.
#[test]
fn test_lambda_rest_insufficient_fixed_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("((lambda (a b &rest c) a) 1)", &env);
    assert!(
        result.contains("Error"),
        "expected error — lambda needs at least 2 fixed args but got 1, got: {result}"
    );
}

/// Macro with &REST also requires the fixed positional args to be satisfied.
/// Providing fewer than the required fixed args should error.
#[test]
fn test_macro_rest_insufficient_fixed_args_error() {
    let env = env_with_stdlib();
    let result = eval_line(
        "(progn (defmacro mymac1 (a b &rest c) (list a b)) (mymac1 1))",
        &env,
    );
    assert!(
        result.contains("Error"),
        "expected error — macro needs at least 2 fixed args but got 1, got: {result}"
    );
}

// ============================================================================
// Group 3: Macro wrong exact arity (no &REST)
// ============================================================================

/// Macro with fixed arity 2 called with 3 arguments should error.
#[test]
fn test_macro_too_many_args_error() {
    let env = env_with_stdlib();
    let result = eval_line(
        "(progn (defmacro mymac2 (a b) (list a b)) (mymac2 1 2 3))",
        &env,
    );
    assert!(
        result.contains("Error"),
        "expected error — macro takes 2 args but got 3, got: {result}"
    );
}

/// Macro with fixed arity 2 called with 1 argument should error.
#[test]
fn test_macro_too_few_args_error() {
    let env = env_with_stdlib();
    let result = eval_line(
        "(progn (defmacro mymac3 (a b) (list a b)) (mymac3 1))",
        &env,
    );
    assert!(
        result.contains("Error"),
        "expected error — macro takes 2 args but got 1, got: {result}"
    );
}

// ============================================================================
// Group 4: apply_apply fexpr multi-param (now supported)
// ============================================================================

/// Multi-param fexprs via APPLY: each element of the arg list is bound to its
/// corresponding parameter (already-evaluated since APPLY evaluates its arg list).
#[test]
fn test_apply_fexpr_multi_param_works() {
    let env = env_with_stdlib();
    // fexpr2 has params (a b); apply with '(1 2) → a=1, b=2 → returns a=1
    let result = eval_line(
        "(progn (defexpr fexpr2 (a b) a) (apply 'fexpr2 '(1 2)))",
        &env,
    );
    assert_eq!(
        result, "1",
        "multi-param fexpr via APPLY should bind each arg, got: {result}"
    );
}

/// Multi-param fexpr via APPLY: arity mismatch should still error.
#[test]
fn test_apply_fexpr_multi_param_arity_error() {
    let env = env_with_stdlib();
    let result = eval_line(
        "(progn (defexpr fexpr2 (a b) a) (apply 'fexpr2 '(1)))",
        &env,
    );
    assert!(
        result.contains("Error"),
        "wrong arg count for multi-param fexpr should error, got: {result}"
    );
}

// ============================================================================
// Group 5: DEFDYNAMIC wrong arity
// ============================================================================

/// DEFDYNAMIC with only one argument (missing value) should error.
#[test]
fn test_defdynamic_one_arg_error() {
    let env = env_with_stdlib();
    let result = eval_line("(defdynamic *x*)", &env);
    assert!(
        result.contains("Error"),
        "expected error — defdynamic requires 2-3 arguments, got: {result}"
    );
}

// ============================================================================
// Group 6: Type error branches for builtins
// ============================================================================

/// logor requires integer arguments — passing a string should error.
#[test]
fn test_logor_non_integer_error() {
    let env = env_with_stdlib();
    let result = eval_line(r#"(logior "a" 1)"#, &env);
    assert!(
        result.contains("Error"),
        "expected error for (logior \"a\" 1), got: {result}"
    );
}

/// logand requires integer arguments — passing a string should error.
#[test]
fn test_logand_non_integer_error() {
    let env = env_with_stdlib();
    let result = eval_line(r#"(logand "a" 1)"#, &env);
    assert!(
        result.contains("Error"),
        "expected error for (logand \"a\" 1), got: {result}"
    );
}

/// logxor requires integer arguments — passing a string should error.
#[test]
fn test_logxor_non_integer_error() {
    let env = env_with_stdlib();
    let result = eval_line(r#"(logxor "a" 1)"#, &env);
    assert!(
        result.contains("Error"),
        "expected error for (logxor \"a\" 1), got: {result}"
    );
}

/// lognot requires an integer argument — passing a string should error.
#[test]
fn test_lognot_non_integer_error() {
    let env = env_with_stdlib();
    let result = eval_line(r#"(lognot "a")"#, &env);
    assert!(
        result.contains("Error"),
        "expected error for (lognot \"a\"), got: {result}"
    );
}

/// ash requires integer arguments — passing a string as the value should error.
#[test]
fn test_ash_non_integer_value_error() {
    let env = env_with_stdlib();
    let result = eval_line(r#"(ash "a" 1)"#, &env);
    assert!(
        result.contains("Error"),
        "expected error for (ash \"a\" 1), got: {result}"
    );
}

/// ash requires integer arguments — passing a string as the shift should error.
#[test]
fn test_ash_non_integer_shift_error() {
    let env = env_with_stdlib();
    let result = eval_line(r#"(ash 1 "a")"#, &env);
    assert!(
        result.contains("Error"),
        "expected error for (ash 1 \"a\"), got: {result}"
    );
}

/// leftshift requires integer arguments — passing a string should error.
#[test]
fn test_leftshift_non_integer_error() {
    let env = env_with_stdlib();
    let result = eval_line(r#"(leftshift "a" 1)"#, &env);
    assert!(
        result.contains("Error"),
        "expected error for (leftshift \"a\" 1), got: {result}"
    );
}

/// rot requires integer arguments — passing a string should error.
#[test]
fn test_rot_non_integer_error() {
    let env = env_with_stdlib();
    let result = eval_line(r#"(rot "a" 1)"#, &env);
    assert!(
        result.contains("Error"),
        "expected error for (rot \"a\" 1), got: {result}"
    );
}

/// random requires an integer — passing a string should error.
#[test]
fn test_random_non_integer_error() {
    let env = env_with_stdlib();
    let result = eval_line(r#"(random "a")"#, &env);
    assert!(
        result.contains("Error"),
        "expected error for (random \"a\"), got: {result}"
    );
}

/// mod requires integer arguments — passing a string as dividend should error.
#[test]
fn test_mod_non_integer_dividend_error() {
    let env = env_with_stdlib();
    let result = eval_line(r#"(mod "a" 2)"#, &env);
    assert!(
        result.contains("Error"),
        "expected error for (mod \"a\" 2), got: {result}"
    );
}

/// mod requires integer arguments — passing a string as divisor should error.
#[test]
fn test_mod_non_integer_divisor_error() {
    let env = env_with_stdlib();
    let result = eval_line(r#"(mod 1 "a")"#, &env);
    assert!(
        result.contains("Error"),
        "expected error for (mod 1 \"a\"), got: {result}"
    );
}

/// plusp requires a number — passing a string should error.
#[test]
fn test_plusp_non_number_error() {
    let env = env_with_stdlib();
    let result = eval_line(r#"(plusp "a")"#, &env);
    assert!(
        result.contains("Error"),
        "expected error for (plusp \"a\"), got: {result}"
    );
}

/// evenp requires an integer — passing a string should error.
#[test]
fn test_evenp_non_integer_error() {
    let env = env_with_stdlib();
    let result = eval_line(r#"(evenp "a")"#, &env);
    assert!(
        result.contains("Error"),
        "expected error for (evenp \"a\"), got: {result}"
    );
}

/// oddp requires an integer — passing a string should error.
#[test]
fn test_oddp_non_integer_error() {
    let env = env_with_stdlib();
    let result = eval_line(r#"(oddp "a")"#, &env);
    assert!(
        result.contains("Error"),
        "expected error for (oddp \"a\"), got: {result}"
    );
}

/// add1 requires a number — passing a string should error.
#[test]
fn test_add1_non_number_error() {
    let env = env_with_stdlib();
    let result = eval_line(r#"(add1 "a")"#, &env);
    assert!(
        result.contains("Error"),
        "expected error for (add1 \"a\"), got: {result}"
    );
}

/// sub1 requires a number — passing a string should error.
#[test]
fn test_sub1_non_number_error() {
    let env = env_with_stdlib();
    let result = eval_line(r#"(sub1 "a")"#, &env);
    assert!(
        result.contains("Error"),
        "expected error for (sub1 \"a\"), got: {result}"
    );
}

/// fixp requires exactly one argument — calling with zero args should error.
#[test]
fn test_fixp_zero_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(fixp)", &env);
    assert!(
        result.contains("Error"),
        "expected error for (fixp) — requires exactly one argument, got: {result}"
    );
}

/// floatp requires exactly one argument — calling with zero args should error.
#[test]
fn test_floatp_zero_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(floatp)", &env);
    assert!(
        result.contains("Error"),
        "expected error for (floatp) — requires exactly one argument, got: {result}"
    );
}

/// functionp requires exactly one argument — calling with zero args should error.
#[test]
fn test_functionp_zero_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(functionp)", &env);
    assert!(
        result.contains("Error"),
        "expected error for (functionp) — requires exactly one argument, got: {result}"
    );
}

/// macrop requires exactly one argument — calling with zero args should error.
#[test]
fn test_macrop_zero_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(macrop)", &env);
    assert!(
        result.contains("Error"),
        "expected error for (macrop) — requires exactly one argument, got: {result}"
    );
}

/// symbolp requires exactly one argument — calling with zero args should error.
#[test]
fn test_symbolp_zero_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(symbolp)", &env);
    assert!(
        result.contains("Error"),
        "expected error for (symbolp) — requires exactly one argument, got: {result}"
    );
}

// Note: (last "not-a-list") returns () rather than an error, so it is omitted.

/// nth requires a number as the first argument — passing a string should error.
#[test]
fn test_nth_non_integer_index_error() {
    let env = env_with_stdlib();
    let result = eval_line(r#"(nth "a" '(1 2 3))"#, &env);
    assert!(
        result.contains("Error"),
        "expected error for (nth \"a\" '(1 2 3)), got: {result}"
    );
}

/// nthcdr requires a number as the first argument — passing a string should error.
#[test]
fn test_nthcdr_non_integer_index_error() {
    let env = env_with_stdlib();
    let result = eval_line(r#"(nthcdr "a" '(1 2 3))"#, &env);
    assert!(
        result.contains("Error"),
        "expected error for (nthcdr \"a\" '(1 2 3)), got: {result}"
    );
}

/// efface requires exactly two arguments — passing three should error.
#[test]
fn test_efface_wrong_arity_error() {
    let env = env_with_stdlib();
    let result = eval_line("(efface 1 2 3)", &env);
    assert!(
        result.contains("Error"),
        "expected error for (efface 1 2 3) — requires exactly two arguments, got: {result}"
    );
}

/// intern requires a string or symbol — passing an integer should error.
#[test]
fn test_intern_non_string_error() {
    let env = env_with_stdlib();
    let result = eval_line("(intern 42)", &env);
    assert!(
        result.contains("Error"),
        "expected error for (intern 42) — intern requires a string or symbol, got: {result}"
    );
}

/// implode requires a list — passing a string should error.
#[test]
fn test_implode_non_list_error() {
    let env = env_with_stdlib();
    let result = eval_line(r#"(implode "a")"#, &env);
    assert!(
        result.contains("Error"),
        "expected error for (implode \"a\") — implode requires a list, got: {result}"
    );
}

/// maknam requires a list — passing a string should error.
#[test]
fn test_maknam_non_list_error() {
    let env = env_with_stdlib();
    let result = eval_line(r#"(maknam "a")"#, &env);
    assert!(
        result.contains("Error"),
        "expected error for (maknam \"a\") — maknam requires a list, got: {result}"
    );
}

/// explode with a float argument should error.
#[test]
fn test_explode_float_error() {
    let env = env_with_stdlib();
    let result = eval_line("(explode 1.5)", &env);
    assert!(
        result.contains("Error"),
        "expected error for (explode 1.5) — explode requires symbol, string, or integer, got: {result}"
    );
}

// ============================================================================
// Group 7: Shell/feature error branches
// ============================================================================

/// enable-feature requires a string or symbol — passing an integer should error.
#[test]
fn test_enable_feature_non_string_error() {
    let env = env_with_stdlib();
    let result = eval_line("(enable-feature 42)", &env);
    assert!(
        result.contains("Error"),
        "expected error for (enable-feature 42), got: {result}"
    );
}

/// disable-feature requires a string or symbol — passing an integer should error.
#[test]
fn test_disable_feature_non_string_error() {
    let env = env_with_stdlib();
    let result = eval_line("(disable-feature 42)", &env);
    assert!(
        result.contains("Error"),
        "expected error for (disable-feature 42), got: {result}"
    );
}

/// feature-enabled-p requires a string or symbol — passing an integer should error.
#[test]
fn test_feature_enabled_p_non_string_error() {
    let env = env_with_stdlib();
    let result = eval_line("(feature-enabled-p 42)", &env);
    assert!(
        result.contains("Error"),
        "expected error for (feature-enabled-p 42), got: {result}"
    );
}
