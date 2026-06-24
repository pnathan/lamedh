/// Tests targeting specific uncovered paths in evaluator.rs.
/// Each group corresponds to a specific set of uncovered lines.
mod test_helpers;
use lamedh::{eval_line, with_large_stack};
use test_helpers::env_with_stdlib;

// ============================================================================
// Group 1: Negation overflow (lines 175-176)
// Negating i64::MIN wraps around (wrapping_neg) to i64::MIN itself.
// The implementation sets an OVERFLOW flag and returns the wrapping result,
// so this should NOT produce an "Error" string.
// ============================================================================

#[test]
fn test_negate_i64_min_wraps_no_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(- -9223372036854775808)", &env);
        assert!(
            !result.contains("Error"),
            "negating i64::MIN should wrap, not error; got: {result}"
        );
        // wrapping_neg of i64::MIN is i64::MIN itself
        assert!(
            result.contains("-9223372036854775808"),
            "expected wrapping result -9223372036854775808, got: {result}"
        );
    });
}

// ============================================================================
// Group 2: NumericEquals (= operator) errors (lines 437-450)
// ============================================================================

#[test]
fn test_numeric_equal_one_arg_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(= 1)", &env);
        assert!(
            result.contains("Error"),
            "= with one arg should error; got: {result}"
        );
    });
}

#[test]
fn test_numeric_equal_three_args_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(= 1 2 3)", &env);
        assert!(
            result.contains("Error"),
            "= with three args should error; got: {result}"
        );
    });
}

#[test]
fn test_numeric_equal_non_numbers_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line(r#"(= "a" "b")"#, &env);
        assert!(
            result.contains("Error"),
            "= with non-numbers should error; got: {result}"
        );
    });
}

// ============================================================================
// Group 3: Hash table arity errors (lines 473-560 area)
// ============================================================================

#[test]
fn test_set_bang_two_args_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(set-bang (make-hash-table) 'k)", &env);
        assert!(
            result.contains("Error"),
            "set-bang with 2 args should error; got: {result}"
        );
    });
}

#[test]
fn test_get_one_arg_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(gethash (make-hash-table))", &env);
        assert!(
            result.contains("Error"),
            "get with 1 arg should error; got: {result}"
        );
    });
}

#[test]
fn test_get_three_args_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(gethash (make-hash-table) 'k 'extra)", &env);
        assert!(
            result.contains("Error"),
            "get with 3 args should error; got: {result}"
        );
    });
}

#[test]
fn test_delete_key_bang_one_arg_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(delete-key-bang (make-hash-table))", &env);
        assert!(
            result.contains("Error"),
            "delete-key-bang with 1 arg should error; got: {result}"
        );
    });
}

#[test]
fn test_keys_extra_arg_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(keys (make-hash-table) 'extra)", &env);
        assert!(
            result.contains("Error"),
            "keys with extra arg should error; got: {result}"
        );
    });
}

// ============================================================================
// Group 4: Float comparison second-arg errors (lines 816-881)
// ============================================================================

#[test]
fn test_float_equal_string_second_arg_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line(r#"(float-equal 1.0 "a")"#, &env);
        assert!(
            result.contains("Error"),
            "float-equal with string second arg should error; got: {result}"
        );
    });
}

#[test]
fn test_float_lessp_string_second_arg_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line(r#"(float-lessp 1.0 "a")"#, &env);
        assert!(
            result.contains("Error"),
            "float-lessp with string second arg should error; got: {result}"
        );
    });
}

#[test]
fn test_float_greaterp_string_second_arg_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line(r#"(float-greaterp 1.0 "a")"#, &env);
        assert!(
            result.contains("Error"),
            "float-greaterp with string second arg should error; got: {result}"
        );
    });
}

/// float-lessp coerces integer second arg to float: 1.0 < 2 → T
#[test]
fn test_float_lessp_integer_second_arg_coerces() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(float-lessp 1.0 2)", &env);
        assert_eq!(
            result.trim(),
            "T",
            "float-lessp 1.0 2 should return T; got: {result}"
        );
    });
}

/// float-greaterp coerces integer second arg to float: 1.0 > 2 → ()
#[test]
fn test_float_greaterp_integer_second_arg_coerces() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(float-greaterp 1.0 2)", &env);
        assert_eq!(
            result.trim(),
            "()",
            "float-greaterp 1.0 2 should return (); got: {result}"
        );
    });
}

/// float-equal coerces integer second arg to float: 1.0 = 1 → T
#[test]
fn test_float_equal_integer_second_arg_coerces() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(float-equal 1.0 1)", &env);
        assert_eq!(
            result.trim(),
            "T",
            "float-equal 1.0 1 should return T; got: {result}"
        );
    });
}

// ============================================================================
// Group 5: Index first-arg type error (lines 291-293)
// ============================================================================

#[test]
fn test_index_non_string_first_arg_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(index 42 0)", &env);
        assert!(
            result.contains("Error"),
            "index with non-string first arg should error; got: {result}"
        );
    });
}

// ============================================================================
// Group 6: Greaterp, remainder, expt type errors (lines 343, 376, 398)
// ============================================================================

#[test]
fn test_greaterp_strings_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line(r#"(greaterp "a" "b")"#, &env);
        assert!(
            result.contains("Error"),
            "greaterp with strings should error; got: {result}"
        );
    });
}

#[test]
fn test_remainder_string_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line(r#"(remainder "a" 2)"#, &env);
        assert!(
            result.contains("Error"),
            "remainder with string should error; got: {result}"
        );
    });
}

#[test]
fn test_expt_string_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line(r#"(expt "a" 2)"#, &env);
        assert!(
            result.contains("Error"),
            "expt with string should error; got: {result}"
        );
    });
}

// ============================================================================
// Group 7: Implode/maknam with non-symbol non-string elements (lines 2648-2651, 2670-2672)
// ============================================================================

#[test]
fn test_implode_with_numbers_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(implode '(1 2 3))", &env);
        assert!(
            result.contains("Error"),
            "implode with numbers should error; got: {result}"
        );
    });
}

#[test]
fn test_maknam_with_numbers_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(maknam '(1 2 3))", &env);
        assert!(
            result.contains("Error"),
            "maknam with numbers should error; got: {result}"
        );
    });
}

// ============================================================================
// Group 8: Plist with non-symbol (lines 2716-2718)
// ============================================================================

#[test]
fn test_plist_non_symbol_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(plist 42)", &env);
        assert!(
            result.contains("Error"),
            "plist with non-symbol should error; got: {result}"
        );
    });
}

// ============================================================================
// Group 9: Ash arity and zero-shift (lines 2736-2738, 2741-2742)
// ============================================================================

#[test]
fn test_ash_one_arg_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(ash 1)", &env);
        assert!(
            result.contains("Error"),
            "ash with one arg should error; got: {result}"
        );
    });
}

#[test]
fn test_ash_zero_shift_returns_unchanged() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(ash 5 0)", &env);
        assert_eq!(
            result.trim(),
            "5",
            "ash with zero shift should return the value unchanged; got: {result}"
        );
    });
}

// ============================================================================
// Group 10: Apply with improper arg list (line 99)
// ============================================================================

#[test]
fn test_apply_dotted_pair_arg_list_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(apply '+ '(1 . 2))", &env);
        assert!(
            result.contains("Error"),
            "apply with dotted pair arg list should error; got: {result}"
        );
    });
}
