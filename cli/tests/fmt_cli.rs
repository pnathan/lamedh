//! Integration smoke tests for `lamedh --fmt` / `--fmt-check` at the binary
//! level. These drive the compiled `lamedh` binary end-to-end (argument
//! parsing, in-place rewriting, exit codes). Formatting-rule behaviour is
//! covered by the library-level tests in `tests/test_fmt.rs`; here we only
//! confirm the CLI wiring.

use std::io::Write;
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_lamedh")
}

/// Write `contents` to a uniquely named temp file and return its path.
fn temp_lisp(name: &str, contents: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "lamedh_fmt_{}_{}_{name}",
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
fn fmt_check_clean_file_exits_zero_and_prints_nothing() {
    let file = temp_lisp("clean.lisp", "(defun sq (x)\n  (* x x))\n");
    let out = Command::new(bin())
        .args(["--fmt-check", file.to_str().unwrap()])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&file);
    assert!(
        out.status.success(),
        "expected exit 0, got {:?}",
        out.status
    );
    assert!(
        out.stdout.is_empty(),
        "expected no output, got {:?}",
        out.stdout
    );
}

#[test]
fn fmt_check_messy_file_exits_one_and_names_it() {
    let file = temp_lisp("messy.lisp", "(defun sq (x)\n(* x x))\n");
    let path = file.to_str().unwrap();
    let out = Command::new(bin())
        .args(["--fmt-check", path])
        .output()
        .unwrap();
    let before = std::fs::read_to_string(&file).unwrap();
    let _ = std::fs::remove_file(&file);
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains(path), "{stdout}");
    // --fmt-check must never rewrite the file.
    assert_eq!(before, "(defun sq (x)\n(* x x))\n");
}

#[test]
fn fmt_rewrites_in_place_and_second_check_is_clean() {
    let file = temp_lisp("rewrite.lisp", "(defun sq (x)\n(* x x))\n");
    let path = file.to_str().unwrap();

    let out = Command::new(bin()).args(["--fmt", path]).output().unwrap();
    assert!(
        out.status.success(),
        "expected exit 0, got {:?}",
        out.status
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains(path), "{stdout}");

    let rewritten = std::fs::read_to_string(&file).unwrap();
    assert_eq!(rewritten, "(defun sq (x)\n  (* x x))\n");

    let out2 = Command::new(bin())
        .args(["--fmt-check", path])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&file);
    assert!(
        out2.status.success(),
        "expected exit 0 after rewrite, got {:?}",
        out2.status
    );
    assert!(out2.stdout.is_empty());
}

#[test]
fn fmt_check_parse_error_exits_two() {
    let file = temp_lisp("broken.lisp", "(defun oops (x\n");
    let out = Command::new(bin())
        .args(["--fmt-check", file.to_str().unwrap()])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&file);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn fmt_without_files_errors() {
    let out = Command::new(bin()).arg("--fmt").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("requires one or more files"), "{stderr}");
}
