//! Integration smoke tests for `lamedh --test` at the binary level. These
//! drive the compiled `lamedh` binary end-to-end (argument parsing, output
//! formatting, exit codes). The runner's Lisp-layer behaviour
//! (`run-all-tests-detailed`) is covered by the library-level tests in
//! `src/test_runner.rs`; here we only confirm the CLI wiring.

use std::io::Write;
use std::process::Command;

fn bin() -> &'static str {
    env!("CARGO_BIN_EXE_lamedh")
}

fn temp_lisp(name: &str, contents: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "lamedh_test_{}_{}_{name}",
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
fn all_passing_exits_zero_with_summary() {
    let file = temp_lisp(
        "pass.lisp",
        "(deftest a (assert-equal (+ 1 1) 2))\n(deftest b (assert-true t))\n",
    );
    let out = Command::new(bin())
        .args(["--test", file.to_str().unwrap()])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&file);
    assert!(
        out.status.success(),
        "expected exit 0, got {:?}",
        out.status
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("test result: 2 passed; 0 failed"),
        "{stdout}"
    );
    assert!(!stdout.contains("FAIL"), "{stdout}");
}

#[test]
fn failing_test_exits_one_and_names_it() {
    let file = temp_lisp(
        "fail.lisp",
        "(deftest good (assert-true t))\n(deftest bad (assert-equal 1 2))\n",
    );
    let out = Command::new(bin())
        .args(["--test", file.to_str().unwrap()])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&file);
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("BAD"), "{stdout}");
    assert!(
        stdout.contains("test result: 1 passed; 1 failed"),
        "{stdout}"
    );
}

#[test]
fn sexpr_format_is_selectable() {
    let file = temp_lisp("fail2.lisp", "(deftest bad (assert-equal 1 2))\n");
    let out = Command::new(bin())
        .args(["--test", "--error-format=sexpr", file.to_str().unwrap()])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&file);
    assert_eq!(out.status.code(), Some(1));
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("(test . \"BAD\")"), "{stdout}");
    assert!(stdout.contains("(status . fail)"), "{stdout}");
    assert!(
        stdout.contains("test result: 0 passed; 1 failed"),
        "{stdout}"
    );
}

#[test]
fn parse_error_exits_two() {
    let file = temp_lisp("broken.lisp", "(deftest oops (x\n");
    let out = Command::new(bin())
        .args(["--test", file.to_str().unwrap()])
        .output()
        .unwrap();
    let _ = std::fs::remove_file(&file);
    assert_eq!(out.status.code(), Some(2));
}

#[test]
fn test_without_files_errors() {
    let out = Command::new(bin()).arg("--test").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("requires one or more files"), "{stderr}");
}

#[test]
fn directory_loads_sorted_lisp_files() {
    let dir = std::env::temp_dir().join(format!(
        "lamedh_test_dir_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("a.lisp"), "(deftest t1 (assert-true t))\n").unwrap();
    std::fs::write(dir.join("b.lisp"), "(deftest t2 (assert-true t))\n").unwrap();

    let out = Command::new(bin())
        .args(["--test", dir.to_str().unwrap()])
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        out.status.success(),
        "expected exit 0, got {:?}",
        out.status
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(
        stdout.contains("test result: 2 passed; 0 failed"),
        "{stdout}"
    );
}
