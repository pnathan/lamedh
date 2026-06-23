mod test_helpers;
use lamedh::eval_line;
use test_helpers::env_with_stdlib;

// The SHELL capability is OFF by default and must be opted into. We use
// `printf` in commands for deterministic (newline-free) stdout.

#[test]
fn test_shell_feature_off_by_default() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(feature-enabled-p \"SHELL\")", &env), "()");
    assert_eq!(eval_line("(features)", &env), "()");
}

#[test]
fn test_shell_gated_off_errors_cleanly() {
    let env = env_with_stdlib();
    let out = eval_line("(shell \"printf hi\")", &env);
    assert!(out.contains("not enabled"), "got: {out}");
}

#[test]
fn test_enable_then_run() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(enable-feature \"SHELL\")", &env), "T");
    assert_eq!(eval_line("(feature-enabled-p \"SHELL\")", &env), "T");
    // (exit-code stdout stderr)
    assert_eq!(eval_line("(shell \"printf hi\")", &env), "(0 \"hi\" \"\")");
}

#[test]
fn test_features_list_after_enable() {
    let env = env_with_stdlib();
    eval_line("(enable-feature \"SHELL\")", &env);
    assert_eq!(eval_line("(features)", &env), "(\"SHELL\")");
}

#[test]
fn test_enable_via_symbol_is_case_normalized() {
    let env = env_with_stdlib();
    // symbol 'shell is read as SHELL; string lookup must agree
    assert_eq!(eval_line("(enable-feature 'shell)", &env), "T");
    assert_eq!(eval_line("(feature-enabled-p \"SHELL\")", &env), "T");
}

#[test]
fn test_nonzero_exit_code_is_returned() {
    let env = env_with_stdlib();
    eval_line("(enable-feature \"SHELL\")", &env);
    assert_eq!(eval_line("(shell \"exit 3\")", &env), "(3 \"\" \"\")");
}

#[test]
fn test_multi_arg_exec_form_no_shell() {
    let env = env_with_stdlib();
    eval_line("(enable-feature \"SHELL\")", &env);
    // program + args, no shell interpolation
    assert_eq!(
        eval_line("(shell \"printf\" \"xyz\")", &env),
        "(0 \"xyz\" \"\")"
    );
}

#[test]
fn test_disable_feature_re_gates() {
    let env = env_with_stdlib();
    eval_line("(enable-feature \"SHELL\")", &env);
    assert_eq!(eval_line("(disable-feature \"SHELL\")", &env), "T");
    let out = eval_line("(shell \"printf hi\")", &env);
    assert!(out.contains("not enabled"), "got: {out}");
}

#[test]
fn test_sh_helper_returns_stdout() {
    let env = env_with_stdlib();
    eval_line("(enable-feature \"SHELL\")", &env);
    assert_eq!(eval_line("(sh \"printf hello\")", &env), "\"hello\"");
}

#[test]
fn test_sh_helper_errors_on_nonzero() {
    let env = env_with_stdlib();
    eval_line("(enable-feature \"SHELL\")", &env);
    let out = eval_line("(sh \"exit 1\")", &env);
    assert!(out.contains("failed"), "got: {out}");
}
