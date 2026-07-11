//! String <-> UTF-8 Array<Char> boundary (issue #254, epic #253): the
//! kernel primitives STRING->UTF8*/UTF8->STRING*/UTF8->STRING-LOSSY* wrapped
//! by lib/30-text.lisp's TEXT module.
//!
//! Coverage: exact byte-for-byte agreement with Rust's own UTF-8 encoding,
//! round-trips across ASCII / multibyte / non-BMP / empty strings, strict
//! vs. lossy invalid-UTF-8 handling, and — the #137 typed/JIT membrane
//! acceptance criterion — that an Array<Char> the evaluator builds carries
//! the exact same byte values a typed `(array char)` FETCH sees, across
//! every typed tier (compiled, deopt-interpreter, traced).

use lamedh::environment::Environment;
use lamedh::eval_line;
use lamedh::jit::{Jit, Value};
use lamedh::reader::read;

/// Build a fresh `Jit` with `defs` (each a `defun-typed` source string).
fn jit_with(defs: &[&str]) -> Jit {
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    for src in defs {
        let form = read(src, &env).unwrap_or_else(|e| panic!("read failed for `{src}`: {e}"));
        j.define(&form)
            .unwrap_or_else(|e| panic!("define failed for `{src}`: {e}"));
    }
    j
}

#[test]
fn string_to_utf8_matches_rust_encoding() {
    let env = Environment::with_stdlib();
    let bytes = "café".as_bytes();
    assert_eq!(
        eval_line("(array-length* (string->utf8* \"café\"))", &env),
        bytes.len().to_string()
    );
    for (i, b) in bytes.iter().enumerate() {
        let out = eval_line(
            &format!("(char-code (fetch (string->utf8* \"café\") {i}))"),
            &env,
        );
        assert_eq!(out, b.to_string(), "byte {i} of \"café\" mismatch");
    }
}

#[test]
fn utf8_roundtrip_ascii_multibyte_nonbmp_empty() {
    let env = Environment::with_stdlib();
    // ASCII, 2-byte (é), 3-byte (世界), 4-byte non-BMP (🎉), and empty.
    for s in ["", "hello", "café", "世界", "🎉"] {
        let out = eval_line(
            &format!("(equal (utf8->string* (string->utf8* {s:?})) {s:?})"),
            &env,
        );
        assert_eq!(out, "T", "roundtrip failed for {s:?}");
    }
}

#[test]
fn utf8_to_string_strict_signals_error_with_offset() {
    let env = Environment::with_stdlib();
    // A lone UTF-8 continuation byte (0x80) is never valid on its own.
    let out = eval_line("(utf8->string* (list->array (list (make-char 128))))", &env);
    assert!(
        out.starts_with("Error: UTF8->STRING: invalid UTF-8 at byte offset 0"),
        "got: {out}"
    );
}

#[test]
fn utf8_to_string_lossy_substitutes_replacement_char() {
    let env = Environment::with_stdlib();
    let out = eval_line(
        "(utf8->string-lossy* (list->array (list (make-char 104) (make-char 128) (make-char 105))))",
        &env,
    );
    assert_eq!(out, "\"h\u{FFFD}i\"");
}

#[test]
fn text_module_wraps_the_kernel_primitives() {
    let env = Environment::with_stdlib();
    eval_line("(import text)", &env);
    assert_eq!(
        eval_line("(utf8->string (string->utf8 \"héllo\"))", &env),
        "\"héllo\""
    );
    assert_eq!(
        eval_line(
            "(equal (text:string->utf8 \"hi\") (text:string->utf8 \"hi\"))",
            &env
        ),
        // Arrays compare by identity, not contents (mutable containers).
        "()"
    );
    assert_eq!(
        eval_line(
            "(equal (array->list (text:string->utf8 \"hi\")) (array->list (string->utf8* \"hi\")))",
            &env
        ),
        "T"
    );
}

#[test]
fn evaluator_bytes_agree_with_typed_jit_array_char_membrane() {
    // Ground truth: the exact UTF-8 bytes Rust computes for "café" — the
    // bytes STRING->UTF8* must produce (checked below on the evaluator
    // side), and the bytes an (array char) built from them must FETCH
    // identically on the typed side.
    let bytes: Vec<u8> = "café".bytes().collect();

    // Evaluator side.
    let env = Environment::with_stdlib();
    for (i, b) in bytes.iter().enumerate() {
        let out = eval_line(
            &format!("(char-code (fetch (string->utf8* \"café\") {i}))"),
            &env,
        );
        assert_eq!(out, b.to_string());
    }

    // Typed/JIT side: the SAME bytes, as an (array char), FETCHed through
    // every typed tier. This is the #137 array-char membrane (already
    // covered generically by test_jit_array_parity.rs/test_jit_char_parity.rs);
    // #254 introduces no new representation, so it must agree exactly.
    let j = jit_with(&["(defun-typed (nth-byte char) ((a (array char)) (i int64)) (fetch a i))"]);
    let arr = Value::Array(bytes.iter().map(|b| Value::Char(*b)).collect());
    for (i, b) in bytes.iter().enumerate() {
        let idx = Value::Int(i as i64);
        j.compile_all();
        let compiled = j.call("nth-byte", &[arr.clone(), idx.clone()]);
        j.deoptimize_all();
        let interpreted = j.call("nth-byte", &[arr.clone(), idx.clone()]);
        let traced = j
            .trace_call("nth-byte", &[arr.clone(), idx])
            .map(|(v, _log)| v);
        for (label, r) in [
            ("compiled", compiled),
            ("deopt-interpreter", interpreted),
            ("traced", traced),
        ] {
            assert_eq!(r, Ok(Value::Char(*b)), "{label} tier: byte {i} mismatch");
        }
    }
}
