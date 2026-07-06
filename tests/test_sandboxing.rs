mod test_helpers;
use lamedh::{environment::Environment, eval_line};
use test_helpers::env_with_stdlib;

// ---- READ-FS (load-file) capability tests ----

#[test]
fn test_load_file_feature_off_by_default() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(feature-enabled-p \"READ-FS\")", &env), "()");
}

#[test]
fn test_load_file_gated_off_errors_cleanly() {
    let env = env_with_stdlib();
    let out = eval_line("(load-file \"no-such-file.lisp\")", &env);
    assert!(
        out.contains("READ-FS capability"),
        "expected READ-FS capability error, got: {out}"
    );
    assert!(
        out.contains("not enabled"),
        "expected 'not enabled' in error, got: {out}"
    );
}

#[test]
fn test_load_file_passes_feature_check_when_enabled() {
    // After enabling READ-FS the capability check passes; we get a file-not-found
    // error rather than a capability error.
    let env = env_with_stdlib();
    env.enable_feature("READ-FS");
    let out = eval_line(
        "(load-file \"/nonexistent/path/that/does/not/exist.lisp\")",
        &env,
    );
    assert!(
        !out.contains("READ-FS capability"),
        "should not get capability error after enabling, got: {out}"
    );
}

#[test]
fn test_lisp_cannot_self_escalate_file_io() {
    let env = env_with_stdlib();
    let out = eval_line("(enable-feature \"READ-FS\")", &env);
    assert_ne!(out, "T", "Lisp must not be able to grant READ-FS");
    assert!(
        !env.feature_enabled("READ-FS"),
        "READ-FS should remain disabled"
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
fn test_new_sandboxed_has_read_fs_disabled() {
    let env = Environment::new_sandboxed();
    assert!(!env.feature_enabled("READ-FS"));
    assert!(!env.feature_enabled("CREATE-FS"));
    assert!(!env.feature_enabled("TEMP-FS"));
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
    assert_eq!(
        out, "()",
        "sandboxed env should start with no features enabled, got: {out}"
    );
}

#[test]
fn test_new_sandboxed_load_file_blocked() {
    let env = Environment::new_sandboxed();
    let out = eval_line("(load-file \"anything.lisp\")", &env);
    assert!(
        out.contains("READ-FS capability"),
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

// ---- RENAME-FILE capability tests (issue #273) ----

#[test]
fn test_rename_file_requires_read_fs_too() {
    // CREATE-FS alone must not be enough: rename observes the source path
    // (existence probing via error messages), so it also needs READ-FS.
    let env = env_with_stdlib();
    env.enable_feature("CREATE-FS");
    let out = eval_line("(rename-file \"/tmp/a\" \"/tmp/b\")", &env);
    assert!(
        out.contains("READ-FS capability"),
        "expected READ-FS capability error with only CREATE-FS granted, got: {out}"
    );
}

#[test]
fn test_rename_file_requires_create_fs_too() {
    let env = env_with_stdlib();
    env.enable_feature("READ-FS");
    let out = eval_line("(rename-file \"/tmp/a\" \"/tmp/b\")", &env);
    assert!(
        out.contains("CREATE-FS capability"),
        "expected CREATE-FS capability error with only READ-FS granted, got: {out}"
    );
}

#[test]
fn test_rename_file_passes_capability_checks_with_both() {
    // With both capabilities the checks pass; a nonexistent source then
    // yields an ordinary I/O error, not a capability error.
    let env = env_with_stdlib();
    env.enable_feature("READ-FS");
    env.enable_feature("CREATE-FS");
    let out = eval_line(
        "(rename-file \"/nonexistent/source/xyzzy\" \"/nonexistent/dest/xyzzy\")",
        &env,
    );
    assert!(
        !out.contains("capability"),
        "should not get a capability error with both granted, got: {out}"
    );
}
