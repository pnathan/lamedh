//! Unicode-aware case conversion and character classes (issue #519).
//!
//! The helpers in lib/14-strings.lisp were ASCII range checks; they now sit on
//! the STRING-UPCASE*/STRING-DOWNCASE* and CHAR-*-P* kernel primitives. The
//! full Lisp-level coverage (ASCII sweep included) is in
//! tests/lisp/51-string-completions.lisp; these pin the issue's repro table.

mod test_helpers;
use lamedh::eval_line;
use test_helpers::env_with_stdlib;

#[test]
fn issue_519_repro_table() {
    let env = env_with_stdlib();
    assert_eq!(eval_line(r#"(char-downcase "É")"#, &env), r#""é""#);
    assert_eq!(eval_line(r#"(alphanumeric-p "é")"#, &env), "T");
    assert_eq!(
        eval_line(r#"(string-upcase "straße")"#, &env),
        r#""STRASSE""#
    );
}

#[test]
fn case_mapping_greek_and_cjk() {
    let env = env_with_stdlib();
    assert_eq!(eval_line(r#"(string-upcase "αβγ")"#, &env), r#""ΑΒΓ""#);
    assert_eq!(eval_line(r#"(string-downcase "ΟΔΟΣ")"#, &env), r#""οδος""#);
    assert_eq!(
        eval_line(r#"(string-upcase "漢字 ok")"#, &env),
        r#""漢字 OK""#
    );
    assert_eq!(eval_line(r#"(alpha-p "漢")"#, &env), "T");
    assert_eq!(eval_line(r#"(char-upper-p "漢")"#, &env), "()");
    // CHAR-UPCASE keeps its one-character contract: sharp s is unchanged.
    assert_eq!(eval_line(r#"(char-upcase "ß")"#, &env), r#""ß""#);
}

#[test]
fn reader_symbol_folding_unchanged() {
    // The reader folds symbol case itself (src/reader.rs), independent of the
    // Lisp-layer helpers, and accepts only ASCII symbol constituents; both
    // are unchanged. STRING-UPCASE still names the reader's symbol.
    let env = env_with_stdlib();
    assert_eq!(eval_line("'foo-bar", &env), "FOO-BAR");
    assert_eq!(
        eval_line(r#"(eq 'foo-bar (intern (string-upcase "foo-bar")))"#, &env),
        "T"
    );
    assert!(eval_line("'café", &env).contains("parse error"));
}
