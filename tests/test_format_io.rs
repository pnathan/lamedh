//! Host-visible I/O & formatting coverage for issue #150: FORMAT's port
//! destination, READ-LINE against real stdin capability gating,
//! WITH-OUTPUT-TO-STRING's port lifecycle, and the READ-SEXPR-FILE /
//! WRITE-SEXPR-FILE round-trip (needs READ-FS/CREATE-FS, which the Lisp
//! xUnit suite's environment does not grant -- see
//! tests/lisp/96-format-and-io.lisp for everything that does not need a
//! capability).

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line};

fn env_with_stdlib() -> Shared<Environment> {
    Environment::with_stdlib()
}

fn temp_path(name: &str) -> String {
    let mut p = std::env::temp_dir();
    p.push(format!(
        "lamedh-format-io-test-{}-{}-{name}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    p.to_string_lossy().into_owned()
}

// ── FORMAT: port destination ────────────────────────────────────────────

#[test]
fn format_port_destination_writes_exact_utf8_bytes() {
    let env = env_with_stdlib();
    let out = eval_line(
        "(let ((p (ports:open-output-bytes)))
           (format p \"hi ~a\" 42)
           (text:utf8->string-lossy (ports:output-contents p)))",
        &env,
    );
    assert_eq!(out, "\"hi 42\"");
}

#[test]
fn format_port_destination_returns_nil() {
    let env = env_with_stdlib();
    let out = eval_line("(format (ports:open-output-bytes) \"x\")", &env);
    assert_eq!(out, "()");
}

#[test]
fn format_invalid_destination_errors() {
    let env = env_with_stdlib();
    let out = eval_line("(format 'bogus \"x\")", &env);
    assert!(
        out.contains("FORMAT") && out.contains("destination"),
        "got: {out}"
    );
}

#[test]
fn format_stdout_destination_needs_no_capability() {
    // PRINC already writes to stdout unconditionally (matching the ports
    // module's STDOUT), so FORMAT with DEST=T should too.
    let env = env_with_stdlib();
    let out = eval_line("(format t \"\")", &env);
    assert_eq!(out, "()");
}

// ── FORMAT: unknown directive is now an error, not pass-through ────────────

#[test]
fn format_unknown_directive_errors_not_passthrough() {
    let env = env_with_stdlib();
    let out = eval_line("(format nil \"~z\")", &env);
    assert!(
        out.contains("FORMAT") || out.contains("Error"),
        "got: {out}"
    );
    assert_ne!(out, "\"~z\"", "must not silently pass through");
}

// ── FORMAT: long control string stays stack-safe (#361 trap) ───────────────

#[test]
fn format_long_control_string_does_not_overflow() {
    // Comfortably past the 10,000-eval-frame recursion limit (#361's trap):
    // if FORMAT-BUILD's walk over the control string were not tail
    // recursive, this would blow the interpreter's stack instead of just
    // taking a moment.
    let env = env_with_stdlib();
    let n = 12_000;
    let long_literal_body = "x".repeat(n);
    let src = format!(
        "(string-length* (format nil (concat {:?} \"~a\") 1))",
        long_literal_body
    );
    let out = eval_line(&src, &env);
    // The literal body (N chars) plus PRINC-TO-STRING of 1 (one char).
    assert_eq!(out, (n + 1).to_string());
}

#[test]
fn format_large_iteration_list_does_not_overflow() {
    // Same concern as above, for the ~{...~} iteration helper
    // ($FORMAT-ITERATE): a large iteration list must not grow the stack.
    let env = env_with_stdlib();
    let out = eval_line(
        "(progn
           (defun $range (k acc) (if (< k 0) acc ($range (- k 1) (cons k acc))))
           (let ((lst ($range 11999 ())))
             (list (length lst) (stringp (format nil \"~{~d,~}\" lst)))))",
        &env,
    );
    assert_eq!(out, "(12000 T)");
}

// ── READ-LINE ────────────────────────────────────────────────────────────

#[test]
fn read_line_default_requires_io_capability() {
    let env = env_with_stdlib();
    let out = eval_line("(read-line)", &env);
    assert!(
        out.contains("IO capability") && out.contains("not enabled"),
        "got: {out}"
    );
}

#[test]
fn read_line_explicit_port_needs_no_extra_capability() {
    let env = env_with_stdlib();
    let out = eval_line(
        "(read-line (ports:open-input-bytes (text:string->utf8 \"hi\")))",
        &env,
    );
    assert_eq!(out, "\"hi\"");
}

// ── WITH-OUTPUT-TO-STRING ───────────────────────────────────────────────

#[test]
fn with_output_to_string_captures_writes_and_closes_port() {
    let env = env_with_stdlib();
    let out = eval_line(
        "(let ((p nil))
           (list (with-output-to-string (s)
                   (setq p s)
                   (ports:write-string! s \"abc\"))
                 (ports:open-p p)))",
        &env,
    );
    assert_eq!(out, "(\"abc\" ())");
}

#[test]
fn with_output_to_string_closes_port_even_on_error() {
    let env = env_with_stdlib();
    let out = eval_line(
        "(let ((p nil))
           (list (errorset '(with-output-to-string (s)
                               (setq p s)
                               (error \"boom\"))
                            nil)
                 (ports:open-p p)))",
        &env,
    );
    assert_eq!(out, "(() ())");
}

// ── s-expression file round-trip ────────────────────────────────────────

#[test]
fn sexpr_file_roundtrip_requires_capabilities() {
    let env = env_with_stdlib();
    let path = temp_path("roundtrip");
    let out = eval_line(&format!("(write-sexpr-file {path:?} (list 1 2 3))"), &env);
    assert!(out.contains("CREATE-FS capability"), "got: {out}");
}

#[test]
fn sexpr_file_roundtrip_writes_and_reads_multiple_forms() {
    let env = env_with_stdlib();
    env.enable_feature("READ-FS");
    env.enable_feature("CREATE-FS");
    let path = temp_path("roundtrip-ok");
    let write_out = eval_line(
        &format!("(write-sexpr-file {path:?} (list (list 1 2 3) \"hi\" 'foo 4.5))"),
        &env,
    );
    assert_eq!(write_out, "T");
    let read_out = eval_line(&format!("(read-sexpr-file {path:?})"), &env);
    assert_eq!(read_out, "((1 2 3) \"hi\" FOO 4.5)");
    let _ = std::fs::remove_file(&path);
}
