mod test_helpers;
use lamedh::eval_line;
use test_helpers::env_with_stdlib;

// The SHELL capability is OFF by default and must be granted by the host.
// Lisp code can introspect capabilities but cannot grant them.

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
    env.enable_feature("SHELL");
    assert_eq!(eval_line("(feature-enabled-p \"SHELL\")", &env), "T");
    // (exit-code stdout stderr)
    assert_eq!(eval_line("(shell \"printf hi\")", &env), "(0 \"hi\" \"\")");
}

#[test]
fn test_features_list_after_enable() {
    let env = env_with_stdlib();
    env.enable_feature("SHELL");
    assert_eq!(eval_line("(features)", &env), "(\"SHELL\")");
}

#[test]
fn test_nonzero_exit_code_is_returned() {
    let env = env_with_stdlib();
    env.enable_feature("SHELL");
    assert_eq!(eval_line("(shell \"exit 3\")", &env), "(3 \"\" \"\")");
}

#[test]
fn test_multi_arg_exec_form_no_shell() {
    let env = env_with_stdlib();
    env.enable_feature("SHELL");
    // program + args, no shell interpolation
    assert_eq!(
        eval_line("(shell \"printf\" \"xyz\")", &env),
        "(0 \"xyz\" \"\")"
    );
}

#[test]
fn test_host_disable_re_gates() {
    // The host API can revoke a capability; Lisp code cannot.
    let env = env_with_stdlib();
    env.enable_feature("SHELL");
    env.disable_feature("SHELL");
    let out = eval_line("(shell \"printf hi\")", &env);
    assert!(out.contains("not enabled"), "got: {out}");
}

#[test]
fn test_lisp_cannot_self_escalate() {
    // enable-feature must not be callable from Lisp.
    let env = env_with_stdlib();
    let out = eval_line("(enable-feature \"SHELL\")", &env);
    // Should be an unbound-symbol error, not T.
    assert_ne!(out, "T", "Lisp should not be able to grant capabilities");
    assert!(
        !env.feature_enabled("SHELL"),
        "SHELL should remain disabled"
    );
}

#[test]
fn test_sh_helper_returns_stdout() {
    let env = env_with_stdlib();
    env.enable_feature("SHELL");
    assert_eq!(eval_line("(sh \"printf hello\")", &env), "\"hello\"");
}

#[test]
fn test_sh_helper_errors_on_nonzero() {
    let env = env_with_stdlib();
    env.enable_feature("SHELL");
    let out = eval_line("(sh \"exit 1\")", &env);
    assert!(out.contains("failed"), "got: {out}");
}
