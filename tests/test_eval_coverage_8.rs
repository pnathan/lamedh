/// Tests targeting specific uncovered paths in evaluator.rs (coverage run 8).
/// Each group corresponds to a specific set of uncovered lines.
mod test_helpers;
use lamedh::{eval_line, with_large_stack};
use test_helpers::env_with_stdlib;

// ============================================================================
// Group 1: PROG errors (lines 1653-1669)
// ============================================================================

#[test]
fn test_prog_no_args_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(prog)", &env);
        assert!(
            result.contains("Error"),
            "prog with no args should error; got: {result}"
        );
        assert!(
            result.contains("PROG requires at least a var list"),
            "expected 'PROG requires at least a var list'; got: {result}"
        );
    });
}

#[test]
fn test_prog_non_symbol_var_list_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(prog (1 2 3) (return 0))", &env);
        assert!(
            result.contains("Error"),
            "prog with non-symbol var list should error; got: {result}"
        );
        assert!(
            result.contains("PROG variable list must contain only symbols"),
            "expected 'PROG variable list must contain only symbols'; got: {result}"
        );
    });
}

// ============================================================================
// Group 2: RETURN/GO wrong arity (lines 1728-1740)
// ============================================================================

#[test]
fn test_return_no_args_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(prog () (return))", &env);
        assert!(
            result.contains("Error"),
            "return with no args should error; got: {result}"
        );
        assert!(
            result.contains("RETURN takes exactly one argument"),
            "expected 'RETURN takes exactly one argument'; got: {result}"
        );
    });
}

#[test]
fn test_return_two_args_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(prog () (return 1 2))", &env);
        assert!(
            result.contains("Error"),
            "return with two args should error; got: {result}"
        );
        assert!(
            result.contains("RETURN takes exactly one argument"),
            "expected 'RETURN takes exactly one argument'; got: {result}"
        );
    });
}

#[test]
fn test_go_no_args_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(prog () (go))", &env);
        assert!(
            result.contains("Error"),
            "go with no args should error; got: {result}"
        );
        assert!(
            result.contains("GO takes exactly one argument"),
            "expected 'GO takes exactly one argument'; got: {result}"
        );
    });
}

// ============================================================================
// Group 3: LET binding not a pair (line 1765-1767)
// ============================================================================

#[test]
fn test_let_binding_empty_list_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        // () is an empty list — length is 0, not 2 → "let binding must be a pair"
        let result = eval_line("(let ((x 1) ()) x)", &env);
        assert!(
            result.contains("Error"),
            "let with empty-list binding should error; got: {result}"
        );
        assert!(
            result.contains("let binding must be a pair"),
            "expected 'let binding must be a pair'; got: {result}"
        );
    });
}

#[test]
fn test_let_binding_single_element_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        // (y) has length 1, not 2 → "let binding must be a pair"
        let result = eval_line("(let ((x 1) (y)) x)", &env);
        assert!(
            result.contains("Error"),
            "let with single-element binding should error; got: {result}"
        );
        assert!(
            result.contains("let binding must be a pair"),
            "expected 'let binding must be a pair'; got: {result}"
        );
    });
}

// ============================================================================
// Group 4: Fexpr parameter error in APPLY path (line 1788)
// ============================================================================

#[test]
fn test_fexpr_two_params_direct_call_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        // defexpr with two params, then call it directly
        // fexpr must have exactly one parameter for the list of arguments
        let result =
            eval_line("(progn (defexpr f (a b) (list a b)) (f 1 2))", &env);
        assert!(
            result.contains("Error"),
            "fexpr with 2 params called directly should error; got: {result}"
        );
        assert!(
            result.contains("fexpr must have exactly one parameter"),
            "expected 'fexpr must have exactly one parameter...'; got: {result}"
        );
    });
}

// ============================================================================
// Group 5: UNQUOTE wrong arity (lines 1824-1826)
// ============================================================================

#[test]
fn test_unquote_zero_args_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        // (quasiquote (unquote)) triggers the "unquote takes exactly one argument" error
        let result = eval_line("(quasiquote (unquote))", &env);
        assert!(
            result.contains("Error"),
            "unquote with zero args should error; got: {result}"
        );
        assert!(
            result.contains("unquote takes exactly one argument"),
            "expected 'unquote takes exactly one argument'; got: {result}"
        );
    });
}

// ============================================================================
// Group 6: GETP/PUTP errors (lines 1847-1888)
// ============================================================================

#[test]
fn test_getp_zero_args_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(getp)", &env);
        assert!(
            result.contains("Error"),
            "getp with no args should error; got: {result}"
        );
        assert!(
            result.contains("get-p takes exactly two arguments"),
            "expected 'get-p takes exactly two arguments'; got: {result}"
        );
    });
}

#[test]
fn test_getp_three_args_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line(r#"(getp 'sym "k" 'extra)"#, &env);
        assert!(
            result.contains("Error"),
            "getp with 3 args should error; got: {result}"
        );
        assert!(
            result.contains("get-p takes exactly two arguments"),
            "expected 'get-p takes exactly two arguments'; got: {result}"
        );
    });
}

#[test]
fn test_getp_non_symbol_first_arg_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line(r#"(getp 42 "k")"#, &env);
        assert!(
            result.contains("Error"),
            "getp with number first arg should error; got: {result}"
        );
        assert!(
            result.contains("get-p requires a symbol as its first argument"),
            "expected 'get-p requires a symbol as its first argument'; got: {result}"
        );
    });
}

#[test]
fn test_getp_non_string_second_arg_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(getp 'sym 42)", &env);
        assert!(
            result.contains("Error"),
            "getp with number second arg should error; got: {result}"
        );
        assert!(
            result.contains("get-p requires a string as its second argument"),
            "expected 'get-p requires a string as its second argument'; got: {result}"
        );
    });
}

#[test]
fn test_putp_zero_args_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(putp)", &env);
        assert!(
            result.contains("Error"),
            "putp with no args should error; got: {result}"
        );
        assert!(
            result.contains("put-p takes exactly three arguments"),
            "expected 'put-p takes exactly three arguments'; got: {result}"
        );
    });
}

#[test]
fn test_putp_non_symbol_first_arg_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line(r#"(putp 42 "k" 1)"#, &env);
        assert!(
            result.contains("Error"),
            "putp with number first arg should error; got: {result}"
        );
        assert!(
            result.contains("put-p requires a symbol as its first argument"),
            "expected 'put-p requires a symbol as its first argument'; got: {result}"
        );
    });
}

#[test]
fn test_putp_non_string_second_arg_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(putp 'sym 42 1)", &env);
        assert!(
            result.contains("Error"),
            "putp with number second arg should error; got: {result}"
        );
        assert!(
            result.contains("put-p requires a string as its second argument"),
            "expected 'put-p requires a string as its second argument'; got: {result}"
        );
    });
}

// ============================================================================
// Group 7: READ with args (lines 1903-1905)
// ============================================================================

#[test]
fn test_read_with_arg_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(read 1)", &env);
        assert!(
            result.contains("Error"),
            "read with arg should error; got: {result}"
        );
        assert!(
            result.contains("read takes no arguments"),
            "expected 'read takes no arguments'; got: {result}"
        );
    });
}

// ============================================================================
// Group 8: prin1 on a list (line 1937)
// ============================================================================

#[test]
fn test_prin1_on_list_succeeds() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        // prin1 on a list uses printer::print path (not the string-shortcut path)
        let result = eval_line("(prin1 '(1 2 3))", &env);
        assert!(
            !result.contains("Error"),
            "prin1 on a list should succeed; got: {result}"
        );
        assert!(
            result.contains("1") && result.contains("2") && result.contains("3"),
            "prin1 should output list elements; got: {result}"
        );
    });
}

// ============================================================================
// Group 9: error/errorset wrong arity (lines 1966, 1977-1979)
// ============================================================================

#[test]
fn test_error_zero_args() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        // (error) with 0 args raises Generic("Error")
        let result = eval_line("(error)", &env);
        assert!(
            result.contains("Error"),
            "error with no args should produce an error; got: {result}"
        );
    });
}

#[test]
fn test_errorset_zero_args_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(errorset)", &env);
        assert!(
            result.contains("Error"),
            "errorset with no args should error; got: {result}"
        );
        assert!(
            result.contains("errorset requires one or two arguments"),
            "expected 'errorset requires one or two arguments'; got: {result}"
        );
    });
}

#[test]
fn test_errorset_three_args_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(errorset '(+ 1 1) nil 'extra)", &env);
        assert!(
            result.contains("Error"),
            "errorset with 3 args should error; got: {result}"
        );
        assert!(
            result.contains("errorset requires one or two arguments"),
            "expected 'errorset requires one or two arguments'; got: {result}"
        );
    });
}

// ============================================================================
// Group 10: subst/sublis/assoc arity errors (lines 2002-2004, 2028-2030, 2079-2081)
// ============================================================================

#[test]
fn test_subst_zero_args_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(subst)", &env);
        assert!(
            result.contains("Error"),
            "subst with no args should error; got: {result}"
        );
        assert!(
            result.contains("subst requires exactly three arguments"),
            "expected 'subst requires exactly three arguments'; got: {result}"
        );
    });
}

#[test]
fn test_subst_two_args_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(subst 1 2)", &env);
        assert!(
            result.contains("Error"),
            "subst with 2 args should error; got: {result}"
        );
        assert!(
            result.contains("subst requires exactly three arguments"),
            "expected 'subst requires exactly three arguments'; got: {result}"
        );
    });
}

#[test]
fn test_sublis_zero_args_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(sublis)", &env);
        assert!(
            result.contains("Error"),
            "sublis with no args should error; got: {result}"
        );
        assert!(
            result.contains("sublis requires exactly two arguments"),
            "expected 'sublis requires exactly two arguments'; got: {result}"
        );
    });
}

#[test]
fn test_sublis_one_arg_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(sublis '((a . b)))", &env);
        assert!(
            result.contains("Error"),
            "sublis with 1 arg should error; got: {result}"
        );
        assert!(
            result.contains("sublis requires exactly two arguments"),
            "expected 'sublis requires exactly two arguments'; got: {result}"
        );
    });
}

#[test]
fn test_assoc_zero_args_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(assoc)", &env);
        assert!(
            result.contains("Error"),
            "assoc with no args should error; got: {result}"
        );
        assert!(
            result.contains("assoc requires exactly two arguments"),
            "expected 'assoc requires exactly two arguments'; got: {result}"
        );
    });
}

#[test]
fn test_assoc_one_arg_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(assoc 'a)", &env);
        assert!(
            result.contains("Error"),
            "assoc with 1 arg should error; got: {result}"
        );
        assert!(
            result.contains("assoc requires exactly two arguments"),
            "expected 'assoc requires exactly two arguments'; got: {result}"
        );
    });
}

// ============================================================================
// Group 11: SHELL symbol/float coercion (lines 593-595)
// ============================================================================

#[test]
fn test_shell_symbol_arg_coercion() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        // Symbol arg → coerced to string via symbol name
        let result = eval_line(
            r#"(progn (enable-feature "SHELL") (shell "echo" 'hello))"#,
            &env,
        );
        assert!(
            !result.contains("Error"),
            "shell with symbol arg should succeed; got: {result}"
        );
        // Should return (exit-code stdout stderr) with exit code 0
        assert!(
            result.contains('0'),
            "shell echo with symbol arg should exit 0; got: {result}"
        );
    });
}

#[test]
fn test_shell_float_arg_coercion() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        // Float arg → coerced to string via to_string()
        let result = eval_line(
            r#"(progn (enable-feature "SHELL") (shell "echo" 1.5))"#,
            &env,
        );
        assert!(
            !result.contains("Error"),
            "shell with float arg should succeed; got: {result}"
        );
        assert!(
            result.contains('0'),
            "shell echo with float arg should exit 0; got: {result}"
        );
    });
}

// ============================================================================
// Group 12: DEFEXPR/DEFMACRO arity and docstring errors (lines 1571-1574, 1587-1589)
// ============================================================================

#[test]
fn test_defexpr_zero_args_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(defexpr)", &env);
        assert!(
            result.contains("Error"),
            "defexpr with no args should error; got: {result}"
        );
        assert!(
            result.contains("DEFEXPR takes three or four arguments"),
            "expected 'DEFEXPR takes three or four arguments'; got: {result}"
        );
    });
}

#[test]
fn test_defexpr_one_arg_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(defexpr f)", &env);
        assert!(
            result.contains("Error"),
            "defexpr with 1 arg should error; got: {result}"
        );
        assert!(
            result.contains("DEFEXPR takes three or four arguments"),
            "expected 'DEFEXPR takes three or four arguments'; got: {result}"
        );
    });
}

#[test]
fn test_defexpr_five_args_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line(r#"(defexpr f (a) "doc" body extra)"#, &env);
        assert!(
            result.contains("Error"),
            "defexpr with 5 args should error; got: {result}"
        );
        assert!(
            result.contains("DEFEXPR takes three or four arguments"),
            "expected 'DEFEXPR takes three or four arguments'; got: {result}"
        );
    });
}

#[test]
fn test_defexpr_non_string_docstring_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        // 4 args but 3rd arg (docstring) is a number, not a string
        let result = eval_line("(defexpr f (a) 42 body)", &env);
        assert!(
            result.contains("Error"),
            "defexpr with non-string docstring should error; got: {result}"
        );
        assert!(
            result.contains("docstring must be a string"),
            "expected 'docstring must be a string'; got: {result}"
        );
    });
}

#[test]
fn test_defmacro_zero_args_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let result = eval_line("(defmacro)", &env);
        assert!(
            result.contains("Error"),
            "defmacro with no args should error; got: {result}"
        );
        assert!(
            result.contains("DEFMACRO takes three or four arguments"),
            "expected 'DEFMACRO takes three or four arguments'; got: {result}"
        );
    });
}

// ============================================================================
// Group 12b: DEFDYNAMIC non-string docstring (line 1574 region)
// ============================================================================

#[test]
fn test_defdynamic_non_string_docstring_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        // defdynamic with 3 args but docstring is not a string
        let result = eval_line("(defdynamic *x* 1 123)", &env);
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

#[test]
fn test_defdynamic_too_many_args_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        // defdynamic with 4+ args
        let result = eval_line(r#"(defdynamic *x* 1 "doc" 5 6)"#, &env);
        assert!(
            result.contains("Error"),
            "defdynamic with too many args should error; got: {result}"
        );
        assert!(
            result.contains("defdynamic requires 2 or 3 arguments"),
            "expected 'defdynamic requires 2 or 3 arguments'; got: {result}"
        );
    });
}
