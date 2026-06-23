mod test_helpers;
use lamedh::{environment::Environment, eval_line};
use test_helpers::env_with_stdlib;

// ---- FILE-IO (load-file) capability tests ----

#[test]
fn test_load_file_feature_off_by_default() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(feature-enabled-p \"FILE-IO\")", &env), "()");
}

#[test]
fn test_load_file_gated_off_errors_cleanly() {
    let env = env_with_stdlib();
    let out = eval_line("(load-file \"no-such-file.lisp\")", &env);
    assert!(
        out.contains("FILE-IO capability"),
        "expected FILE-IO capability error, got: {out}"
    );
    assert!(
        out.contains("not enabled"),
        "expected 'not enabled' in error, got: {out}"
    );
}

#[test]
fn test_load_file_error_mentions_enable_feature() {
    let env = env_with_stdlib();
    let out = eval_line("(load-file \"no-such-file.lisp\")", &env);
    assert!(
        out.contains("enable-feature"),
        "error should mention enable-feature, got: {out}"
    );
}

#[test]
fn test_load_file_passes_feature_check_when_enabled() {
    // After enabling FILE-IO the feature check passes; we get a file-not-found
    // error rather than a capability error.
    let env = env_with_stdlib();
    eval_line("(enable-feature \"FILE-IO\")", &env);
    let out = eval_line("(load-file \"/nonexistent/path/that/does/not/exist.lisp\")", &env);
    assert!(
        !out.contains("FILE-IO capability"),
        "should not get capability error after enabling, got: {out}"
    );
}

#[test]
fn test_disable_file_io_re_gates() {
    let env = env_with_stdlib();
    eval_line("(enable-feature \"FILE-IO\")", &env);
    eval_line("(disable-feature \"FILE-IO\")", &env);
    let out = eval_line("(load-file \"no-such-file.lisp\")", &env);
    assert!(
        out.contains("FILE-IO capability"),
        "expected capability error after re-gating, got: {out}"
    );
}

// ---- IO (read) capability tests ----

#[test]
fn test_read_feature_off_by_default() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(feature-enabled-p \"IO\")", &env), "()");
}

#[test]
fn test_read_gated_off_errors_cleanly() {
    let env = env_with_stdlib();
    let out = eval_line("(read)", &env);
    assert!(
        out.contains("IO capability"),
        "expected IO capability error, got: {out}"
    );
    assert!(
        out.contains("not enabled"),
        "expected 'not enabled' in error, got: {out}"
    );
}

#[test]
fn test_read_error_mentions_enable_feature() {
    let env = env_with_stdlib();
    let out = eval_line("(read)", &env);
    assert!(
        out.contains("enable-feature"),
        "error should mention enable-feature, got: {out}"
    );
}

// ---- new_sandboxed() constructor tests ----

#[test]
fn test_new_sandboxed_has_shell_disabled() {
    let env = Environment::new_sandboxed();
    assert!(
        !env.feature_enabled("SHELL"),
        "SHELL should be disabled in sandboxed env"
    );
}

#[test]
fn test_new_sandboxed_has_file_io_disabled() {
    let env = Environment::new_sandboxed();
    assert!(
        !env.feature_enabled("FILE-IO"),
        "FILE-IO should be disabled in sandboxed env"
    );
}

#[test]
fn test_new_sandboxed_has_io_disabled() {
    let env = Environment::new_sandboxed();
    assert!(
        !env.feature_enabled("IO"),
        "IO should be disabled in sandboxed env"
    );
}

#[test]
fn test_new_sandboxed_features_list_is_empty() {
    let env = Environment::new_sandboxed();
    // No features enabled; the Lisp (features) builtin should return ()
    let out = eval_line("(features)", &env);
    assert_eq!(out, "()", "sandboxed env should start with no features enabled, got: {out}");
}

#[test]
fn test_new_sandboxed_load_file_blocked() {
    let env = Environment::new_sandboxed();
    let out = eval_line("(load-file \"anything.lisp\")", &env);
    assert!(
        out.contains("FILE-IO capability"),
        "load-file should be blocked in sandboxed env, got: {out}"
    );
}

#[test]
fn test_new_sandboxed_shell_blocked() {
    let env = Environment::new_sandboxed();
    let out = eval_line("(shell \"echo hi\")", &env);
    assert!(
        out.contains("not enabled"),
        "shell should be blocked in sandboxed env, got: {out}"
    );
}

#[test]
fn test_new_sandboxed_read_blocked() {
    let env = Environment::new_sandboxed();
    let out = eval_line("(read)", &env);
    assert!(
        out.contains("IO capability"),
        "read should be blocked in sandboxed env, got: {out}"
    );
}
