//! Integration tests for `--fuel N`: a per-execution-unit step budget that
//! turns runaway code into a clean `fuel exhausted` error with a nonzero exit
//! in batch modes, while leaving well-behaved programs untouched.

use std::io::Write;
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_lamedh")
}

fn temp_lisp(name: &str, contents: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "lamedh_fuel_{}_{}_{name}",
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
fn script_infinite_loop_exits_nonzero_with_fuel_error() {
    let file = temp_lisp("spin.lisp", "(defun spin (n) (spin (+ n 1)))\n(spin 0)\n");
    let out = Command::new(bin())
        .args(["--fuel", "300000", file.to_str().unwrap()])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&file);
    assert_eq!(out.status.code(), Some(1), "expected exit 1");
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        stderr.contains("fuel"),
        "expected fuel error, got: {stderr}"
    );
}

#[test]
fn well_behaved_script_under_generous_fuel_runs_clean() {
    let file = temp_lisp("ok.lisp", "(print (+ 1 2 3))\n");
    let out = Command::new(bin())
        .args(["--fuel", "100000000", file.to_str().unwrap()])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&file);
    assert!(
        out.status.success(),
        "expected exit 0, got {:?}",
        out.status
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains('6'), "{stdout}");
}

#[test]
fn s_expression_infinite_loop_exits_nonzero() {
    let out = Command::new(bin())
        .args([
            "--fuel",
            "300000",
            "-s",
            "(defun loop (n) (loop (+ n 1))) (loop 0)",
        ])
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("fuel"), "{stderr}");
}

#[test]
fn s_expression_well_behaved_under_generous_fuel() {
    let out = Command::new(bin())
        .args(["--fuel", "100000000", "-s", "(+ 1 2 3)"])
        .output()
        .unwrap();
    assert!(out.status.success(), "{:?}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains('6'), "{stdout}");
}

#[test]
fn without_fuel_flag_a_bounded_program_still_runs() {
    // No --fuel: the budget is unarmed, so ordinary code is unaffected.
    let out = Command::new(bin())
        .args(["-s", "(+ 40 2)"])
        .output()
        .unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("42"), "{stdout}");
}
