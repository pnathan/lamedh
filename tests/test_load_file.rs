mod test_helpers;
use lamedh::{eval_line, load_file};
use std::fs;
use std::path::PathBuf;
use test_helpers::env_with_stdlib;

fn temp_load_dir(name: &str) -> PathBuf {
    let unique = format!(
        "lamedh-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    );
    std::env::temp_dir().join(unique)
}

#[test]
fn test_load_file() {
    let env = env_with_stdlib();
    // Enable READ-FS so the feature gate passes.
    env.enable_feature("READ-FS");
    let load_output = eval_line("(load-file \"tests/load_file_test_sample.lisp\")", &env);
    assert_eq!(load_output, "T");
    let output = eval_line("(loaded-function)", &env);
    assert_eq!(output, "42");
}

#[test]
fn test_load_file_include_relative_file() {
    let root = temp_load_dir("include-relative");
    let lib = root.join("lib");
    fs::create_dir_all(&lib).unwrap();
    fs::write(
        lib.join("defs.lisp"),
        r#"
        (defun included-value () 99)
        "#,
    )
    .unwrap();
    fs::write(
        root.join("main.lisp"),
        r#"
        (include "lib/defs.lisp")
        (defun main-value () (included-value))
        "#,
    )
    .unwrap();

    let env = env_with_stdlib();
    load_file(root.join("main.lisp").to_str().unwrap(), &env).unwrap();
    assert_eq!(eval_line("(main-value)", &env), "99");

    let _ = fs::remove_dir_all(root);
}

#[test]
fn test_load_file_include_cycle_errors() {
    let root = temp_load_dir("include-cycle");
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("a.lisp"), r#"(include "b.lisp")"#).unwrap();
    fs::write(root.join("b.lisp"), r#"(include "a.lisp")"#).unwrap();

    let env = env_with_stdlib();
    let err = load_file(root.join("a.lisp").to_str().unwrap(), &env).unwrap_err();
    assert!(
        format!("{err}").contains("include cycle detected"),
        "expected include cycle error, got {err}"
    );

    let _ = fs::remove_dir_all(root);
}
