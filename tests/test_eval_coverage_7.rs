/// Tests targeting specific uncovered paths in evaluator.rs (coverage run 7).
/// Each group corresponds to a specific set of uncovered lines.
mod test_helpers;
use lamedh::{eval_line, with_large_stack};
use test_helpers::env_with_stdlib;

// ============================================================================
// Group 1: current-environment arity error (lines 523-527)
// ============================================================================

#[test]
fn test_current_environment_with_arg_errors() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(current-environment 1)", &env);
        assert!(
            result.contains("Error"),
            "current-environment with arg should error; got: {result}"
        );
        assert!(
            result.contains("current-environment takes no arguments"),
            "expected 'current-environment takes no arguments'; got: {result}"
        );
    });
}

// ============================================================================
// Group 2: float-lessp and float-greaterp first-arg type errors (lines 835-842, 866-873)
// ============================================================================

#[test]
fn test_float_lessp_string_first_arg_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line(r#"(float-lessp "a" 1.0)"#, &env);
        assert!(
            result.contains("Error"),
            "float-lessp with string first arg should error; got: {result}"
        );
    });
}

#[test]
fn test_float_greaterp_string_first_arg_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line(r#"(float-greaterp "a" 1.0)"#, &env);
        assert!(
            result.contains("Error"),
            "float-greaterp with string first arg should error; got: {result}"
        );
    });
}

// ============================================================================
// Group 3: Flag functions with non-symbol/non-string arg (lines 920-923, 939-942, 958-961)
// ============================================================================

#[test]
fn test_set_flag_number_arg_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(set-flag 42)", &env);
        assert!(
            result.contains("Error"),
            "set-flag with number should error; got: {result}"
        );
        assert!(
            result.contains("set-flag requires a symbol or string"),
            "expected 'set-flag requires a symbol or string'; got: {result}"
        );
    });
}

#[test]
fn test_clear_flag_number_arg_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(clear-flag 42)", &env);
        assert!(
            result.contains("Error"),
            "clear-flag with number should error; got: {result}"
        );
        assert!(
            result.contains("clear-flag requires a symbol or string"),
            "expected 'clear-flag requires a symbol or string'; got: {result}"
        );
    });
}

#[test]
fn test_flag_set_p_number_arg_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(flag-set-p 42)", &env);
        assert!(
            result.contains("Error"),
            "flag-set-p with number should error; got: {result}"
        );
        assert!(
            result.contains("flag-set-p requires a symbol or string"),
            "expected 'flag-set-p requires a symbol or string'; got: {result}"
        );
    });
}

// ============================================================================
// Group 4: Flag functions with STRING args (string branch coverage, line ~938)
// ============================================================================

#[test]
fn test_set_flag_with_string_succeeds() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line(
            r#"(progn (set-flag "my-flag") (flag-set-p "my-flag"))"#,
            &env,
        );
        assert_eq!(
            result.trim(),
            "T",
            "set-flag/flag-set-p with string should return T; got: {result}"
        );
    });
}

#[test]
fn test_clear_flag_with_string_succeeds() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line(
            r#"(progn (set-flag "my-flag") (clear-flag "my-flag") (flag-set-p "my-flag"))"#,
            &env,
        );
        assert_eq!(
            result.trim(),
            "()",
            "after set-flag then clear-flag with string, flag-set-p should return (); got: {result}"
        );
    });
}

// ============================================================================
// Group 5: features arity error (lines 1001-1004)
// ============================================================================

#[test]
fn test_features_with_arg_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(features 1)", &env);
        assert!(
            result.contains("Error"),
            "features with arg should error; got: {result}"
        );
        assert!(
            result.contains("features takes no arguments"),
            "expected 'features takes no arguments'; got: {result}"
        );
    });
}

// ============================================================================
// Group 6: DEFEXPR with non-symbol params (lines 1112-1114)
// ============================================================================

#[test]
fn test_defexpr_non_symbol_params_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(defexpr f (1 2) (car args))", &env);
        assert!(
            result.contains("Error"),
            "defexpr with non-symbol params should error; got: {result}"
        );
        assert!(
            result.contains("fexpr parameters must be symbols"),
            "expected 'fexpr parameters must be symbols'; got: {result}"
        );
    });
}

// ============================================================================
// Group 7: DEFMACRO &rest errors and non-symbol params (lines 1141-1143, 1148-1150, 1156-1158)
// ============================================================================

#[test]
fn test_defmacro_rest_with_no_following_symbol_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        // (defmacro m (&rest) body) — &rest not followed by a symbol
        let result = eval_line("(defmacro m (&rest) body)", &env);
        assert!(
            result.contains("Error"),
            "defmacro with bare &rest should error; got: {result}"
        );
        assert!(
            result.contains("&rest must be followed by a symbol"),
            "expected '&rest must be followed by a symbol'; got: {result}"
        );
    });
}

#[test]
fn test_defmacro_rest_with_two_symbols_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        // (defmacro m (&rest a b) body) — only one symbol can follow &rest
        let result = eval_line("(defmacro m (&rest a b) body)", &env);
        assert!(
            result.contains("Error"),
            "defmacro with two symbols after &rest should error; got: {result}"
        );
        assert!(
            result.contains("Only one symbol can follow &rest"),
            "expected 'Only one symbol can follow &rest'; got: {result}"
        );
    });
}

#[test]
fn test_defmacro_non_symbol_params_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        // (defmacro m (1 2) body) — params must be symbols
        let result = eval_line("(defmacro m (1 2) body)", &env);
        assert!(
            result.contains("Error"),
            "defmacro with non-symbol params should error; got: {result}"
        );
        assert!(
            result.contains("macro parameters must be symbols"),
            "expected 'macro parameters must be symbols'; got: {result}"
        );
    });
}

// ============================================================================
// Group 8: DEF arity/docstring errors (lines 1346-1348, 1360-1362)
// ============================================================================

#[test]
fn test_def_too_many_args_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(def x 1 2 3)", &env);
        assert!(
            result.contains("Error"),
            "def with 4 args should error; got: {result}"
        );
        assert!(
            result.contains("def takes two or three arguments"),
            "expected 'def takes two or three arguments'; got: {result}"
        );
    });
}

#[test]
fn test_def_non_string_docstring_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        // Third arg is a number (not a string) — should error
        let result = eval_line("(def x 1 42)", &env);
        assert!(
            result.contains("Error"),
            "def with non-string docstring should error; got: {result}"
        );
        assert!(
            result.contains("docstring must be a string"),
            "expected 'docstring must be a string'; got: {result}"
        );
    });
}

// ============================================================================
// Group 9: DEFDYNAMIC errors
// ============================================================================

#[test]
fn test_defdynamic_zero_args_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(defdynamic)", &env);
        assert!(
            result.contains("Error"),
            "defdynamic with no args should error; got: {result}"
        );
        assert!(
            result.contains("defdynamic requires 2 or 3 arguments"),
            "expected 'defdynamic requires 2 or 3 arguments'; got: {result}"
        );
    });
}

#[test]
fn test_defdynamic_four_args_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line(r#"(defdynamic *x* 1 "doc" extra)"#, &env);
        assert!(
            result.contains("Error"),
            "defdynamic with 4 args should error; got: {result}"
        );
        assert!(
            result.contains("defdynamic requires 2 or 3 arguments"),
            "expected 'defdynamic requires 2 or 3 arguments'; got: {result}"
        );
    });
}

#[test]
fn test_defdynamic_non_symbol_first_arg_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(defdynamic 42 1)", &env);
        assert!(
            result.contains("Error"),
            "defdynamic with non-symbol first arg should error; got: {result}"
        );
        assert!(
            result.contains("defdynamic first argument must be a symbol"),
            "expected 'defdynamic first argument must be a symbol'; got: {result}"
        );
    });
}

#[test]
fn test_defdynamic_non_string_docstring_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(defdynamic *x* 1 42)", &env);
        assert!(
            result.contains("Error"),
            "defdynamic with non-string docstring should error; got: {result}"
        );
        assert!(
            result.contains("defdynamic docstring must be a string"),
            "expected 'defdynamic docstring must be a string'; got: {result}"
        );
    });
}

// ============================================================================
// Group 10: SHELL feature paths
// ============================================================================

#[test]
fn test_shell_disabled_by_default_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        // Shell is disabled by default
        let result = eval_line(r#"(shell "echo" "hello")"#, &env);
        assert!(
            result.contains("Error"),
            "shell without SHELL feature enabled should error; got: {result}"
        );
        assert!(
            result.contains("SHELL capability is not enabled"),
            "expected 'SHELL capability is not enabled'; got: {result}"
        );
    });
}

#[test]
fn test_shell_with_no_args_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        env.enable_feature("SHELL");
        let result = eval_line("(shell)", &env);
        assert!(
            result.contains("Error"),
            "shell with no args should error; got: {result}"
        );
        assert!(
            result.contains("shell requires at least one argument"),
            "expected 'shell requires at least one argument'; got: {result}"
        );
    });
}

#[test]
fn test_shell_with_string_and_number_args() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        env.enable_feature("SHELL");
        // Number coercion path — shell "echo" 42 should run and return 0 exit code
        let result = eval_line(r#"(shell "echo" 42)"#, &env);
        assert!(
            !result.contains("Error"),
            "shell with string and number args should succeed; got: {result}"
        );
        assert!(
            result.contains('0'),
            "shell echo 42 should have exit code 0; got: {result}"
        );
    });
}

#[test]
fn test_shell_with_cons_arg_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        env.enable_feature("SHELL");
        // Passing a cons/list as a shell arg should error
        let result = eval_line("(shell '(1 2))", &env);
        assert!(
            result.contains("Error"),
            "shell with cons arg should error; got: {result}"
        );
        assert!(
            result.contains("shell arguments must be strings, symbols, or numbers"),
            "expected 'shell arguments must be strings, symbols, or numbers'; got: {result}"
        );
    });
}

// ============================================================================
// Group 11: enable-feature/disable-feature are not exposed to Lisp
// ============================================================================

#[test]
fn test_enable_feature_not_callable_from_lisp() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        // enable-feature must be unbound; calling it should be an error, not T
        let result = eval_line("(enable-feature \"SHELL\")", &env);
        assert_ne!(result, "T", "enable-feature must not be callable from Lisp");
        assert!(
            !env.feature_enabled("SHELL"),
            "SHELL must remain disabled after Lisp call attempt"
        );
    });
}

#[test]
fn test_disable_feature_not_callable_from_lisp() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        env.enable_feature("SHELL");
        // disable-feature must be unbound; calling it should not revoke the capability
        let result = eval_line("(disable-feature \"SHELL\")", &env);
        assert_ne!(
            result, "T",
            "disable-feature must not be callable from Lisp"
        );
        assert!(
            env.feature_enabled("SHELL"),
            "SHELL must remain enabled despite Lisp call attempt"
        );
    });
}

#[test]
fn test_feature_enabled_p_no_args_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(feature-enabled-p)", &env);
        assert!(
            result.contains("Error"),
            "feature-enabled-p with no args should error; got: {result}"
        );
        assert!(
            result.contains("feature-enabled-p requires exactly one argument"),
            "expected 'feature-enabled-p requires exactly one argument'; got: {result}"
        );
    });
}
