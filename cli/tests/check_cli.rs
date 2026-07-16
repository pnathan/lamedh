//! Integration smoke tests for `lamedh --check` at the binary level.
//!
//! These drive the compiled `lamedh` binary end-to-end (argument parsing,
//! output formatting, exit codes). The exhaustive lint behaviour is covered by
//! the library-level tests in the `lamedh` crate (`tests/test_check.rs`); here
//! we only confirm the CLI wiring.

use std::io::Write;
use std::process::Command;

/// Path to the freshly built `lamedh` binary under test.
fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_lamedh")
}

/// Write `contents` to a uniquely named temp file and return its path.
fn temp_lisp(name: &str, contents: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "lamedh_check_{}_{}_{name}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let mut f = std::fs::File::create(&path).unwrap();
    f.write_all(contents.as_bytes()).unwrap();
    path
}

#[test]
fn clean_file_exits_zero() {
    let file = temp_lisp("clean.lisp", "(defun sq (x) (* x x))\n(sq 4)\n");
    let out = Command::new(bin())
        .args(["--check", file.to_str().unwrap()])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&file);
    assert!(
        out.status.success(),
        "expected exit 0, got {:?}",
        out.status
    );
    assert!(out.stdout.is_empty(), "expected no output");
}

#[test]
fn typo_exits_one_with_suggestion() {
    let file = temp_lisp("typo.lisp", "(defun f (xs) (revrse xs))\n");
    let out = Command::new(bin())
        .args(["--check", file.to_str().unwrap()])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&file);
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("unbound function REVRSE"), "{stdout}");
    assert!(stdout.contains("did you mean"), "{stdout}");
}

#[test]
fn sexpr_format_is_selectable() {
    let file = temp_lisp("typo2.lisp", "(defun f (xs) (revrse xs))\n");
    let out = Command::new(bin())
        .args(["--check", "--error-format=sexpr", file.to_str().unwrap()])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&file);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("(kind . unbound-function)"), "{stdout}");
    assert!(stdout.contains("(symbol . \"REVRSE\")"), "{stdout}");
}

#[test]
fn parse_error_exits_two() {
    let file = temp_lisp("broken.lisp", "(defun oops (x\n");
    let out = Command::new(bin())
        .args(["--check", file.to_str().unwrap()])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&file);
    assert_eq!(out.status.code(), Some(2));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("error: parse error"), "{stdout}");
}

#[test]
fn check_without_files_errors() {
    let out = Command::new(bin()).arg("--check").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("requires one or more files"), "{stderr}");
}
