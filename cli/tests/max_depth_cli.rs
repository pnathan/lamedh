//! Integration tests for `--max-depth N` (issue #520): the CLI knob the
//! recursion-limit error names, and the ceiling for `set-eval-depth-limit!`.

use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_lamedh")
}

const DEEP: &str = "(let ((n 0)) (dolist (x (iota 15000)) (setq n (+ n 1))) n)";

#[test]
fn default_limit_error_names_the_flag() {
    let out = Command::new(bin()).args(["-s", DEEP]).output().unwrap();
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("recursion limit exceeded (10000 eval frames)")
            && stderr.contains("`lamedh --max-depth N`"),
        "got: {stderr}"
    );
}

#[test]
fn max_depth_raises_the_limit_and_the_lisp_ceiling() {
    let out = Command::new(bin())
        .args([
            "--max-depth",
            "20000",
            "-s",
            DEEP,
            "-s",
            "(list (set-eval-depth-limit! 100) (set-eval-depth-limit! 20000))",
        ])
        .output()
        .unwrap();
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        out.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    assert_eq!(stdout.trim(), "15000\n(20000 100)");
}

#[test]
fn max_depth_zero_is_rejected() {
    let out = Command::new(bin())
        .args(["--max-depth", "0", "-s", "1"])
        .output()
        .unwrap();
    assert!(!out.status.success());
}
