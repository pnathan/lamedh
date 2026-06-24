/// Tests for special form error edges in the evaluator
mod test_helpers;
use lamedh::eval_line;
use test_helpers::env_with_stdlib;

// ============================================================================
// DEF errors
// ============================================================================

#[test]
fn test_def_non_symbol_name_error() {
    let env = env_with_stdlib();
    let result = eval_line("(def 42 1)", &env);
    assert!(
        result.contains("Error") || result.contains("error"),
        "expected error for (def 42 1), got: {result}"
    );
}

// ============================================================================
// SETQ errors
// ============================================================================

#[test]
fn test_setq_odd_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(setq x)", &env);
    assert!(
        result.contains("Error") || result.contains("error"),
        "expected error for (setq x), got: {result}"
    );
}

#[test]
fn test_setq_non_symbol_var_error() {
    let env = env_with_stdlib();
    let result = eval_line("(setq 42 1)", &env);
    assert!(
        result.contains("Error") || result.contains("error"),
        "expected error for (setq 42 1), got: {result}"
    );
}

// ============================================================================
// LET errors
// ============================================================================

#[test]
fn test_let_no_args_error() {
    let env = env_with_stdlib();
    let result = eval_line("(let)", &env);
    assert!(
        result.contains("Error") || result.contains("error"),
        "expected error for (let), got: {result}"
    );
}

#[test]
fn test_let_bindings_only_error() {
    // LET requires exactly 2 arguments: bindings and body.
    // (let ((x 1))) has only 1 argument (bindings list, no body).
    let env = env_with_stdlib();
    let result = eval_line("(let ((x 1)))", &env);
    assert!(
        result.contains("Error") || result.contains("error"),
        "expected error for (let ((x 1))), got: {result}"
    );
}

#[test]
fn test_let_binding_not_a_pair_error() {
    // x is a symbol, not a pair — should error when list_to_vec fails on it.
    let env = env_with_stdlib();
    let result = eval_line("(let (x) x)", &env);
    assert!(
        result.contains("Error") || result.contains("error"),
        "expected error for (let (x) x), got: {result}"
    );
}

// ============================================================================
// LABEL errors
// ============================================================================

#[test]
fn test_label_wrong_arg_count_error() {
    // LABEL requires exactly 2 arguments.
    let env = env_with_stdlib();
    let result = eval_line("(label foo)", &env);
    assert!(
        result.contains("Error") || result.contains("error"),
        "expected error for (label foo), got: {result}"
    );
}

#[test]
fn test_label_non_symbol_name_error() {
    // LABEL name must be a symbol.
    let env = env_with_stdlib();
    let result = eval_line("(label 42 (lambda (x) x))", &env);
    assert!(
        result.contains("Error") || result.contains("error"),
        "expected error for (label 42 ...), got: {result}"
    );
}

// ============================================================================
// COND errors
// ============================================================================

#[test]
fn test_cond_clause_not_a_list_error() {
    // (cond 42) — 42 is not a list, should error
    let env = env_with_stdlib();
    let result = eval_line("(cond 42)", &env);
    assert!(
        result.contains("Error") || result.contains("error"),
        "expected error for (cond 42), got: {result}"
    );
}

// ============================================================================
// PROG / GO errors
// ============================================================================

#[test]
fn test_prog_go_non_symbol_error() {
    // GO argument must be a symbol.
    let env = env_with_stdlib();
    let result = eval_line("(prog () (go 42))", &env);
    assert!(
        result.contains("Error") || result.contains("error"),
        "expected error for (prog () (go 42)), got: {result}"
    );
}

#[test]
fn test_prog_go_missing_label_error() {
    // GO to a label that doesn't exist in PROG.
    let env = env_with_stdlib();
    let result = eval_line("(prog () (go NOSUCHLABEL))", &env);
    assert!(
        result.contains("Error") || result.contains("error"),
        "expected error for (prog () (go NOSUCHLABEL)), got: {result}"
    );
}

// ============================================================================
// LAMBDA parameter errors
// ============================================================================

#[test]
fn test_lambda_rest_with_no_symbol_error() {
    // &rest must be followed by a symbol.
    let env = env_with_stdlib();
    let result = eval_line("((lambda (&rest) x))", &env);
    assert!(
        result.contains("Error") || result.contains("error"),
        "expected error for (lambda (&rest) x), got: {result}"
    );
}

#[test]
fn test_lambda_rest_with_two_symbols_error() {
    // Only one symbol can follow &rest.
    let env = env_with_stdlib();
    let result = eval_line("((lambda (&rest a b) x))", &env);
    assert!(
        result.contains("Error") || result.contains("error"),
        "expected error for (lambda (&rest a b) x), got: {result}"
    );
}

#[test]
fn test_lambda_params_not_symbols_error() {
    // Lambda parameters must be symbols.
    let env = env_with_stdlib();
    let result = eval_line("((lambda (1 2) a))", &env);
    assert!(
        result.contains("Error") || result.contains("error"),
        "expected error for (lambda (1 2) a), got: {result}"
    );
}
