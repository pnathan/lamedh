/// Tests for arity and type error branches in evaluator builtins
mod test_helpers;
use lamedh::eval_line;
use lamedh::{eval_depth_limit, DEFAULT_EVAL_DEPTH_LIMIT};
use test_helpers::env_with_stdlib;

// ============================================================================
// Division arity
// ============================================================================

#[test]
fn test_div_one_arg_error() {
    let env = env_with_stdlib();
    let result = eval_line("(/ 1)", &env);
    assert!(result.contains("Error") || result.contains("error"),
        "expected error for (/ 1), got: {result}");
}

// ============================================================================
// CAR / CDR / CONS arity errors
// ============================================================================

#[test]
fn test_car_two_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(car 1 2)", &env);
    assert!(result.contains("Error") || result.contains("error"),
        "expected error for (car 1 2), got: {result}");
}

#[test]
fn test_cdr_no_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(cdr)", &env);
    assert!(result.contains("Error") || result.contains("error"),
        "expected error for (cdr), got: {result}");
}

#[test]
fn test_cons_one_arg_error() {
    let env = env_with_stdlib();
    let result = eval_line("(cons 1)", &env);
    assert!(result.contains("Error") || result.contains("error"),
        "expected error for (cons 1), got: {result}");
}

// ============================================================================
// STRING operations type errors
// ============================================================================

#[test]
fn test_concat_non_string_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(concat 1 2)", &env);
    assert!(result.contains("Error") || result.contains("error"),
        "expected error for (concat 1 2), got: {result}");
}

#[test]
fn test_index_no_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(index)", &env);
    assert!(result.contains("Error") || result.contains("error"),
        "expected error for (index), got: {result}");
}

#[test]
fn test_index_non_integer_index_error() {
    let env = env_with_stdlib();
    let result = eval_line("(index \"hi\" \"x\")", &env);
    assert!(result.contains("Error") || result.contains("error"),
        "expected error for (index \"hi\" \"x\"), got: {result}");
}

// ============================================================================
// LESSP / GREATERP errors
// ============================================================================

#[test]
fn test_lessp_one_arg_error() {
    let env = env_with_stdlib();
    let result = eval_line("(lessp 1)", &env);
    assert!(result.contains("Error") || result.contains("error"),
        "expected error for (lessp 1), got: {result}");
}

#[test]
fn test_lessp_non_numeric_error() {
    let env = env_with_stdlib();
    let result = eval_line("(lessp \"a\" \"b\")", &env);
    assert!(result.contains("Error") || result.contains("error"),
        "expected error for (lessp \"a\" \"b\"), got: {result}");
}

#[test]
fn test_greaterp_no_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(greaterp)", &env);
    assert!(result.contains("Error") || result.contains("error"),
        "expected error for (greaterp), got: {result}");
}

// ============================================================================
// ZEROP errors
// ============================================================================

#[test]
fn test_zerop_no_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(zerop)", &env);
    assert!(result.contains("Error") || result.contains("error"),
        "expected error for (zerop), got: {result}");
}

#[test]
fn test_zerop_non_numeric_error() {
    let env = env_with_stdlib();
    let result = eval_line("(zerop \"a\")", &env);
    assert!(result.contains("Error") || result.contains("error"),
        "expected error for (zerop \"a\"), got: {result}");
}

// ============================================================================
// REMAINDER / EXPT errors
// ============================================================================

#[test]
fn test_remainder_no_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(remainder)", &env);
    assert!(result.contains("Error") || result.contains("error"),
        "expected error for (remainder), got: {result}");
}

#[test]
fn test_expt_no_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(expt)", &env);
    assert!(result.contains("Error") || result.contains("error"),
        "expected error for (expt), got: {result}");
}

// ============================================================================
// EQ / NOT / ATOM errors
// ============================================================================

#[test]
fn test_eq_no_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(eq)", &env);
    assert!(result.contains("Error") || result.contains("error"),
        "expected error for (eq), got: {result}");
}

#[test]
fn test_not_no_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(not)", &env);
    assert!(result.contains("Error") || result.contains("error"),
        "expected error for (not), got: {result}");
}

#[test]
fn test_not_two_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(not 1 2)", &env);
    assert!(result.contains("Error") || result.contains("error"),
        "expected error for (not 1 2), got: {result}");
}

#[test]
fn test_atom_no_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(atom)", &env);
    assert!(result.contains("Error") || result.contains("error"),
        "expected error for (atom), got: {result}");
}

// ============================================================================
// Type predicate errors
// ============================================================================

#[test]
fn test_stringp_no_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(stringp)", &env);
    assert!(result.contains("Error") || result.contains("error"),
        "expected error for (stringp), got: {result}");
}

#[test]
fn test_numberp_no_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(numberp)", &env);
    assert!(result.contains("Error") || result.contains("error"),
        "expected error for (numberp), got: {result}");
}

// ============================================================================
// EVAL errors
// ============================================================================

#[test]
fn test_eval_no_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(eval)", &env);
    assert!(result.contains("Error") || result.contains("error"),
        "expected error for (eval), got: {result}");
}

// ============================================================================
// Flag errors
// ============================================================================

#[test]
fn test_set_flag_no_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(set-flag)", &env);
    assert!(result.contains("Error") || result.contains("error"),
        "expected error for (set-flag), got: {result}");
}

#[test]
fn test_clear_flag_no_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(clear-flag)", &env);
    assert!(result.contains("Error") || result.contains("error"),
        "expected error for (clear-flag), got: {result}");
}

#[test]
fn test_flag_set_p_no_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(flag-set-p)", &env);
    assert!(result.contains("Error") || result.contains("error"),
        "expected error for (flag-set-p), got: {result}");
}

#[test]
fn test_clear_all_flags_with_arg_error() {
    let env = env_with_stdlib();
    let result = eval_line("(clear-all-flags 1)", &env);
    assert!(result.contains("Error") || result.contains("error"),
        "expected error for (clear-all-flags 1), got: {result}");
}

// ============================================================================
// LOAD-FILE errors
// ============================================================================

#[test]
fn test_load_file_no_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(load-file)", &env);
    assert!(result.contains("Error") || result.contains("error"),
        "expected error for (load-file), got: {result}");
}

#[test]
fn test_load_file_non_string_error() {
    let env = env_with_stdlib();
    let result = eval_line("(load-file 42)", &env);
    assert!(result.contains("Error") || result.contains("error"),
        "expected error for (load-file 42), got: {result}");
}

// ============================================================================
// Calling a non-function
// ============================================================================

#[test]
fn test_call_number_as_function_error() {
    let env = env_with_stdlib();
    let result = eval_line("(42)", &env);
    assert!(result.contains("Error") || result.contains("error"),
        "expected error for (42), got: {result}");
}

// ============================================================================
// eval_depth_limit getter
// ============================================================================

#[test]
fn test_eval_depth_limit_default() {
    assert_eq!(eval_depth_limit(), DEFAULT_EVAL_DEPTH_LIMIT);
}
