//! Base64 encode/decode (issue #257, epic #253): lib/32-base64.lisp's
//! BASE64 module.
//!
//! Coverage: standard RFC 4648 test vectors, all 256 byte values round-trip
//! (standard/url alphabets, padded/unpadded), malformed-input errors (bad
//! padding count/position, invalid characters, inconsistent unpadded
//! length), and a large-flat-input stack-safety check run under
//! `with_large_stack`.

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line};

fn env_with_base64() -> Shared<Environment> {
    let env = Environment::with_stdlib();
    assert_eq!(eval_line("(import base64)", &env), "BASE64");
    assert_eq!(eval_line("(import text)", &env), "TEXT");
    env
}

fn bytes_expr(bytes: &[u8]) -> String {
    let items: Vec<String> = bytes.iter().map(|b| b.to_string()).collect();
    format!("(list->array (list {}))", items.join(" "))
}

// ── RFC 4648 test vectors ────────────────────────────────────────────────

#[test]
fn rfc4648_test_vectors() {
    let env = env_with_base64();
    let cases = [
        ("", "\"\""),
        ("f", "\"Zg==\""),
        ("fo", "\"Zm8=\""),
        ("foo", "\"Zm9v\""),
        ("foob", "\"Zm9vYg==\""),
        ("fooba", "\"Zm9vYmE=\""),
        ("foobar", "\"Zm9vYmFy\""),
    ];
    for (plain, want) in cases {
        let out = eval_line(&format!("(encode (text:string->utf8 {plain:?}))"), &env);
        assert_eq!(out, want, "encode({plain:?})");
        let dec = eval_line(
            &format!("(equal (text:utf8->string (decode {want})) {plain:?})"),
            &env,
        );
        assert_eq!(dec, "T", "decode({want})");
    }
}

#[test]
fn url_safe_alphabet_uses_dash_underscore() {
    let env = env_with_base64();
    // Bytes chosen so the standard alphabet would emit '+' and '/'.
    let bytes = bytes_expr(&[0xFB, 0xFF, 0xBF]);
    let std_out = eval_line(&format!("(encode {bytes})"), &env);
    let url_out = eval_line(&format!("(encode {bytes} :alphabet ':url)"), &env);
    assert!(
        std_out.contains('+') || std_out.contains('/'),
        "got: {std_out}"
    );
    assert!(
        !url_out.contains('+') && !url_out.contains('/'),
        "got: {url_out}"
    );
    assert_eq!(
        eval_line(
            &format!(
                "(equal (mapcar #'char->code (array->list (decode {url_out} :alphabet ':url))) (mapcar #'char->code (array->list {bytes})))"
            ),
            &env
        ),
        "T"
    );
}

#[test]
fn unpadded_encode_omits_equals_and_decodes_back() {
    let env = env_with_base64();
    let bytes = bytes_expr(b"f");
    let out = eval_line(&format!("(encode {bytes} :pad nil)"), &env);
    assert_eq!(out, "\"Zg\"");
    // DECODE always produces Char elements (matching TEXT:STRING->UTF8's own
    // convention), which the printer shows as character literals.
    assert_eq!(
        eval_line(&format!("(array->list (decode {out} :pad nil))"), &env),
        "('f')"
    );
}

// ── all 256 byte values round-trip ───────────────────────────────────────

#[test]
fn all_256_byte_values_round_trip_every_combination() {
    let env = env_with_base64();
    let all: Vec<u8> = (0..=255u16).map(|b| b as u8).collect();
    let bytes = bytes_expr(&all);
    for (alph, pad) in [
        (":standard", "t"),
        (":standard", "nil"),
        (":url", "t"),
        (":url", "nil"),
    ] {
        let out = eval_line(
            &format!(
                "(equal (mapcar #'char->code (array->list (decode (encode {bytes} :alphabet {alph} :pad {pad}) :alphabet {alph} :pad {pad}))) (mapcar #'char->code (array->list {bytes})))"
            ),
            &env,
        );
        assert_eq!(out, "T", "alphabet={alph} pad={pad}");
    }
}

// ── malformed input ───────────────────────────────────────────────────────

#[test]
fn bad_padding_count_is_an_error() {
    let env = env_with_base64();
    let out = eval_line("(decode \"Zm9v===\")", &env);
    assert!(out.starts_with("Error:"), "got: {out}");
}

#[test]
fn misplaced_padding_is_an_error() {
    let env = env_with_base64();
    let out = eval_line("(decode \"Z=9v\")", &env);
    assert!(
        out.starts_with("Error:") && out.contains("padding"),
        "got: {out}"
    );
}

#[test]
fn invalid_character_is_an_error_naming_position() {
    let env = env_with_base64();
    // Correct length (multiple of 4) so the error is specifically about the
    // invalid character, not the length/padding check.
    let out = eval_line("(decode \"Zm9!\")", &env);
    assert!(
        out.starts_with("Error:") && out.contains("invalid character"),
        "got: {out}"
    );
}

#[test]
fn wrong_length_for_padding_policy_is_an_error() {
    let env = env_with_base64();
    // "Zg" (2 chars) needs padding to "Zg==" for the padded decoder.
    let out = eval_line("(decode \"Zg\")", &env);
    assert!(out.starts_with("Error:"), "got: {out}");
    // A single leftover char is never valid, padded or not.
    let out = eval_line("(decode \"Z\" :pad nil)", &env);
    assert!(out.starts_with("Error:"), "got: {out}");
}

#[test]
fn unexpected_padding_when_pad_nil_is_an_error() {
    let env = env_with_base64();
    let out = eval_line("(decode \"Zg==\" :pad nil)", &env);
    assert!(
        out.starts_with("Error:") && out.contains("padding"),
        "got: {out}"
    );
}

#[test]
fn unknown_alphabet_keyword_is_an_error() {
    let env = env_with_base64();
    let out = eval_line("(encode (list->array ()) :alphabet ':bogus)", &env);
    assert!(out.starts_with("Error:"), "got: {out}");
}

// ── stack safety ──────────────────────────────────────────────────────────

#[test]
fn large_input_round_trips_without_overflowing_the_evaluator_stack() {
    // Before the tail-recursive rewrite, the group-building recursion and
    // STRING->LIST-based decode hit this evaluator's eval-frame recursion
    // limit at a few thousand bytes.
    lamedh::with_large_stack(|| {
        let env = env_with_base64();
        let data: Vec<u8> = (0..4000u32).map(|i| (i % 256) as u8).collect();
        let bytes = bytes_expr(&data);
        let out = eval_line(
            &format!(
                "(equal (mapcar #'char->code (array->list (decode (encode {bytes})))) (mapcar #'char->code (array->list {bytes})))"
            ),
            &env,
        );
        assert_eq!(out, "T");
    });
}
