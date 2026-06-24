/// Tests for hash table, float comparison, and I/O builtin coverage
mod test_helpers;
use lamedh::eval_line;
use test_helpers::env_with_stdlib;

// ============================================================================
// CURRENT-ENVIRONMENT
// ============================================================================

#[test]
fn test_current_environment_returns_something() {
    let env = env_with_stdlib();
    let result = eval_line("(current-environment)", &env);
    // Should not be an error — returns a hash table
    assert!(
        !result.contains("Error"),
        "expected non-error for (current-environment), got: {result}"
    );
}

// ============================================================================
// Hash table type errors
// ============================================================================

#[test]
fn test_set_bang_non_hashtable_error() {
    let env = env_with_stdlib();
    let result = eval_line("(set-bang 42 \"k\" \"v\")", &env);
    assert!(
        result.contains("Error") || result.contains("error"),
        "expected error for (set-bang 42 \"k\" \"v\"), got: {result}"
    );
}

#[test]
fn test_get_non_hashtable_error() {
    let env = env_with_stdlib();
    // GET is now Lisp 1.5 plist lookup (symbol required); GETHASH is for hash-tables
    let result = eval_line("(gethash 42 \"k\")", &env);
    assert!(
        result.contains("Error") || result.contains("error"),
        "expected error for (gethash 42 \"k\"), got: {result}"
    );
}

#[test]
fn test_delete_key_bang_non_hashtable_error() {
    let env = env_with_stdlib();
    let result = eval_line("(delete-key-bang 42 \"k\")", &env);
    assert!(
        result.contains("Error") || result.contains("error"),
        "expected error for (delete-key-bang 42 \"k\"), got: {result}"
    );
}

#[test]
fn test_keys_non_hashtable_error() {
    let env = env_with_stdlib();
    let result = eval_line("(keys 42)", &env);
    assert!(
        result.contains("Error") || result.contains("error"),
        "expected error for (keys 42), got: {result}"
    );
}

#[test]
fn test_make_hash_table_with_arg_error() {
    let env = env_with_stdlib();
    let result = eval_line("(make-hash-table 1)", &env);
    assert!(
        result.contains("Error") || result.contains("error"),
        "expected error for (make-hash-table 1), got: {result}"
    );
}

// ============================================================================
// Float comparisons with integer coercion (should work)
// ============================================================================

#[test]
fn test_float_equal_int_and_float() {
    let env = env_with_stdlib();
    // 1 != 2.0 so should return NIL
    let result = eval_line("(float-equal 1 2.0)", &env);
    assert!(
        !result.contains("Error"),
        "expected non-error for (float-equal 1 2.0), got: {result}"
    );
    assert_eq!(result, "()", "1 and 2.0 are not equal, expected NIL");
}

#[test]
fn test_float_equal_int_coercion_true() {
    let env = env_with_stdlib();
    // 1.0 == 1 (integer coerced to float)
    let result = eval_line("(float-equal 1.0 1)", &env);
    assert!(
        !result.contains("Error"),
        "expected non-error for (float-equal 1.0 1), got: {result}"
    );
    assert_eq!(result, "T", "1.0 and 1 should be float-equal");
}

#[test]
fn test_float_lessp_int_and_float() {
    let env = env_with_stdlib();
    // 0 < 1.5 should be T
    let result = eval_line("(float-lessp 0 1.5)", &env);
    assert!(
        !result.contains("Error"),
        "expected non-error for (float-lessp 0 1.5), got: {result}"
    );
    assert_eq!(result, "T", "0 should be less than 1.5");
}

#[test]
fn test_float_greaterp_int_and_float() {
    let env = env_with_stdlib();
    // 3 > 1.0 should be T
    let result = eval_line("(float-greaterp 3 1.0)", &env);
    assert!(
        !result.contains("Error"),
        "expected non-error for (float-greaterp 3 1.0), got: {result}"
    );
    assert_eq!(result, "T", "3 should be greater than 1.0");
}

// ============================================================================
// Float comparison errors
// ============================================================================

#[test]
fn test_float_equal_non_numeric_error() {
    let env = env_with_stdlib();
    let result = eval_line("(float-equal \"a\" 1.0)", &env);
    assert!(
        result.contains("Error") || result.contains("error"),
        "expected error for (float-equal \"a\" 1.0), got: {result}"
    );
}

#[test]
fn test_float_equal_no_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(float-equal)", &env);
    assert!(
        result.contains("Error") || result.contains("error"),
        "expected error for (float-equal), got: {result}"
    );
}

#[test]
fn test_float_lessp_one_arg_error() {
    let env = env_with_stdlib();
    let result = eval_line("(float-lessp 1.0)", &env);
    assert!(
        result.contains("Error") || result.contains("error"),
        "expected error for (float-lessp 1.0), got: {result}"
    );
}

#[test]
fn test_float_greaterp_no_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(float-greaterp)", &env);
    assert!(
        result.contains("Error") || result.contains("error"),
        "expected error for (float-greaterp), got: {result}"
    );
}

// ============================================================================
// I/O builtins — success cases
// ============================================================================

#[test]
fn test_prin1_returns_value() {
    let env = env_with_stdlib();
    // prin1 prints to stdout and returns its argument
    let result = eval_line("(prin1 42)", &env);
    assert!(
        !result.contains("Error"),
        "expected non-error for (prin1 42), got: {result}"
    );
    assert_eq!(result, "42", "prin1 should return the printed value");
}

#[test]
fn test_princ_string_no_error() {
    let env = env_with_stdlib();
    let result = eval_line("(princ \"hello\")", &env);
    assert!(
        !result.contains("Error"),
        "expected non-error for (princ \"hello\"), got: {result}"
    );
}

#[test]
fn test_terpri_no_error() {
    let env = env_with_stdlib();
    let result = eval_line("(terpri)", &env);
    assert!(
        !result.contains("Error"),
        "expected non-error for (terpri), got: {result}"
    );
}

// ============================================================================
// I/O builtins — error cases
// ============================================================================

#[test]
fn test_prin1_no_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(prin1)", &env);
    assert!(
        result.contains("Error") || result.contains("error"),
        "expected error for (prin1), got: {result}"
    );
}

#[test]
fn test_prin1_too_many_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(prin1 1 2)", &env);
    assert!(
        result.contains("Error") || result.contains("error"),
        "expected error for (prin1 1 2), got: {result}"
    );
}

#[test]
fn test_princ_no_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(princ)", &env);
    assert!(
        result.contains("Error") || result.contains("error"),
        "expected error for (princ), got: {result}"
    );
}

#[test]
fn test_terpri_with_arg_error() {
    let env = env_with_stdlib();
    let result = eval_line("(terpri 1)", &env);
    assert!(
        result.contains("Error") || result.contains("error"),
        "expected error for (terpri 1), got: {result}"
    );
}

// ============================================================================
// Hash table — normal operations (coverage for success paths)
// ============================================================================

#[test]
fn test_make_hash_table_and_set_get() {
    let env = env_with_stdlib();
    // Create a hash table, insert a key, then retrieve it
    let result = eval_line(
        "(progn (def h (make-hash-table)) (set-bang h \"key\" \"val\") (gethash h \"key\"))",
        &env,
    );
    assert!(
        !result.contains("Error"),
        "expected non-error for hash table set/get, got: {result}"
    );
    assert_eq!(result, "\"val\"");
}

#[test]
fn test_make_hash_table_keys() {
    let env = env_with_stdlib();
    let result = eval_line(
        "(progn (def h (make-hash-table)) (set-bang h 'a 1) (set-bang h 'b 2) (keys h))",
        &env,
    );
    assert!(
        !result.contains("Error"),
        "expected non-error for (keys h), got: {result}"
    );
}

#[test]
fn test_delete_key_bang_removes_entry() {
    let env = env_with_stdlib();
    let result = eval_line(
        "(progn (def h (make-hash-table)) (set-bang h 'x 99) (delete-key-bang h 'x) (gethash h 'x))",
        &env,
    );
    // After deletion, get returns NIL
    assert!(
        !result.contains("Error"),
        "expected non-error for delete-key-bang flow, got: {result}"
    );
    assert_eq!(result, "()");
}
