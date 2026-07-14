//! Regression coverage for issue #359: the pre-ports whole-file builtin
//! `read-file-section` decoded bytes with `from_utf8_lossy`, silently
//! substituting U+FFFD — exactly the implicit lossy text coercion epic #253's
//! design rules out (text must cross the byte boundary explicitly).
//!
//! After the fix `read-file-section` decodes STRICTLY (a structured error on
//! invalid UTF-8), with two explicit opt-ins that mirror the text substrate:
//! `read-file-section-lossy` (U+FFFD, like `text:utf8->string-lossy`) and
//! `read-file-section-bytes` (the raw `Array<Char>` byte buffer, to cross the
//! boundary yourself). `read-file-byte` already returned a raw integer and is
//! unchanged.

mod test_helpers;

use lamedh::eval_line;
use std::fs;
use std::path::{Path, PathBuf};
use test_helpers::env_with_stdlib;

/// A unique temp path per (process, thread, name) so parallel test threads
/// never collide.
fn temp_file(name: &str, bytes: &[u8]) -> PathBuf {
    let unique = format!(
        "lamedh-rfs-{name}-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    );
    let path = std::env::temp_dir().join(unique);
    fs::write(&path, bytes).unwrap();
    path
}

fn p(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "\\\\")
}

// 'A' 'B' then two bytes that are not valid UTF-8, then 'C' 'D'.
const INVALID: &[u8] = b"AB\xff\xfeCD";

#[test]
fn strict_decode_on_valid_utf8() {
    let env = env_with_stdlib();
    env.enable_feature("READ-FS");
    let f = temp_file("valid", b"hello world");
    assert_eq!(
        eval_line(&format!("(read-file-section \"{}\" 0 5)", p(&f)), &env),
        "\"hello\""
    );
    // Offset + length past EOF is truncated, not an error.
    assert_eq!(
        eval_line(&format!("(read-file-section \"{}\" 6 100)", p(&f)), &env),
        "\"world\""
    );
}

#[test]
fn strict_decode_errors_on_invalid_utf8() {
    let env = env_with_stdlib();
    env.enable_feature("READ-FS");
    let f = temp_file("strict-bad", INVALID);
    let out = eval_line(&format!("(read-file-section \"{}\" 0 6)", p(&f)), &env);
    assert!(
        out.contains("invalid UTF-8") && out.contains("byte offset 2"),
        "expected a strict invalid-UTF-8 error naming the offset, got: {out}"
    );
    // No silent U+FFFD anywhere in the strict path.
    assert!(
        !out.contains('\u{FFFD}'),
        "strict decode must not substitute U+FFFD, got: {out}"
    );
}

#[test]
fn lossy_variant_substitutes_replacement_char() {
    let env = env_with_stdlib();
    env.enable_feature("READ-FS");
    let f = temp_file("lossy", INVALID);
    // The two invalid bytes each become one U+FFFD.
    assert_eq!(
        eval_line(
            &format!("(read-file-section-lossy \"{}\" 0 6)", p(&f)),
            &env
        ),
        "\"AB\u{FFFD}\u{FFFD}CD\""
    );
}

#[test]
fn bytes_variant_returns_raw_array_of_chars() {
    let env = env_with_stdlib();
    env.enable_feature("READ-FS");
    let f = temp_file("bytes", INVALID);
    // Raw bytes, no decoding: an Array<Char> of the six byte values.
    assert_eq!(
        eval_line(
            &format!(
                "(mapcar #'char-code (array->list (read-file-section-bytes \"{}\" 0 6)))",
                p(&f)
            ),
            &env
        ),
        "(65 66 255 254 67 68)"
    );
}

#[test]
fn bytes_variant_composes_with_text_substrate() {
    let env = env_with_stdlib();
    env.enable_feature("READ-FS");
    eval_line("(require 'text)", &env);
    let good = temp_file("compose-good", b"hello");
    let bad = temp_file("compose-bad", INVALID);
    // Strict decode of clean bytes through the explicit text boundary.
    assert_eq!(
        eval_line(
            &format!(
                "(text:utf8->string (read-file-section-bytes \"{}\" 0 5))",
                p(&good)
            ),
            &env
        ),
        "\"hello\""
    );
    // Lossy decode of dirty bytes through the explicit text boundary matches
    // the read-file-section-lossy convenience.
    assert_eq!(
        eval_line(
            &format!(
                "(text:utf8->string-lossy (read-file-section-bytes \"{}\" 0 6))",
                p(&bad)
            ),
            &env
        ),
        "\"AB\u{FFFD}\u{FFFD}CD\""
    );
}

#[test]
fn read_file_byte_is_byte_clean() {
    // read-file-byte already returned a raw integer (never a decoded string),
    // so it was never affected by #359 — pin that it keeps returning the exact
    // byte value even for a non-ASCII byte, and NIL at EOF.
    let env = env_with_stdlib();
    env.enable_feature("READ-FS");
    let f = temp_file("byte", INVALID);
    assert_eq!(
        eval_line(&format!("(read-file-byte \"{}\" 2)", p(&f)), &env),
        "255"
    );
    assert_eq!(
        eval_line(&format!("(read-file-byte \"{}\" 6)", p(&f)), &env),
        "()"
    );
}

#[test]
fn all_variants_are_read_fs_gated() {
    // Every variant, including the new ones, refuses to run without READ-FS.
    let f = temp_file("gate", b"hi");
    for form in [
        format!("(read-file-section \"{}\" 0 2)", p(&f)),
        format!("(read-file-section-lossy \"{}\" 0 2)", p(&f)),
        format!("(read-file-section-bytes \"{}\" 0 2)", p(&f)),
    ] {
        let env = env_with_stdlib();
        let out = eval_line(&form, &env);
        assert!(
            out.contains("READ-FS"),
            "expected a READ-FS capability error for `{form}`, got: {out}"
        );
    }
}
