//! Hexadecimal encode/decode (issue #257, epic #253): lib/33-hex.lisp's
//! HEX module.
//!
//! Coverage: known vectors, predictable-case ENCODE output vs.
//! case-insensitive DECODE input, all 256 byte values round-trip,
//! malformed-input errors (odd length, invalid digit), and a large-flat-
//! input stack-safety check run under `with_large_stack`.

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line};

fn env_with_hex() -> Shared<Environment> {
    let env = Environment::with_stdlib();
    assert_eq!(eval_line("(import hex)", &env), "HEX");
    assert_eq!(eval_line("(import text)", &env), "TEXT");
    env
}

fn bytes_expr(bytes: &[u8]) -> String {
    let items: Vec<String> = bytes.iter().map(|b| b.to_string()).collect();
    format!("(list->array (list {}))", items.join(" "))
}

#[test]
fn known_vectors() {
    let env = env_with_hex();
    assert_eq!(
        eval_line("(encode (text:string->utf8 \"AB\"))", &env),
        "\"4142\""
    );
    assert_eq!(
        eval_line("(equal (text:utf8->string (decode \"4142\")) \"AB\")", &env),
        "T"
    );
}

#[test]
fn encode_case_is_predictable_lower_and_upper() {
    let env = env_with_hex();
    let bytes = bytes_expr(&[0xAB, 0xCD, 0xEF]);
    assert_eq!(eval_line(&format!("(encode {bytes})"), &env), "\"abcdef\"");
    assert_eq!(
        eval_line(&format!("(encode {bytes} :case ':upper)"), &env),
        "\"ABCDEF\""
    );
}

#[test]
fn decode_is_case_insensitive() {
    let env = env_with_hex();
    let lower = eval_line("(array->list (decode \"abcdef\"))", &env);
    let upper = eval_line("(array->list (decode \"ABCDEF\"))", &env);
    let mixed = eval_line("(array->list (decode \"aBcDeF\"))", &env);
    assert_eq!(lower, upper);
    assert_eq!(lower, mixed);
}

#[test]
fn all_256_byte_values_round_trip() {
    let env = env_with_hex();
    let all: Vec<u8> = (0..=255u16).map(|b| b as u8).collect();
    let bytes = bytes_expr(&all);
    for case in [":lower", ":upper"] {
        let out = eval_line(
            &format!(
                "(equal (mapcar #'char->code (array->list (decode (encode {bytes} :case {case})))) (mapcar #'char->code (array->list {bytes})))"
            ),
            &env,
        );
        assert_eq!(out, "T", "case={case}");
    }
}

#[test]
fn odd_length_input_is_an_error() {
    let env = env_with_hex();
    let out = eval_line("(decode \"abc\")", &env);
    assert!(
        out.starts_with("Error:") && out.contains("odd input length"),
        "got: {out}"
    );
}

#[test]
fn invalid_digit_is_an_error_naming_position() {
    let env = env_with_hex();
    let out = eval_line("(decode \"zz\")", &env);
    assert!(
        out.starts_with("Error:") && out.contains("invalid hex digit"),
        "got: {out}"
    );
}

#[test]
fn unknown_case_keyword_is_an_error() {
    let env = env_with_hex();
    let out = eval_line("(encode (list->array ()) :case ':bogus)", &env);
    assert!(out.starts_with("Error:"), "got: {out}");
}

#[test]
fn large_input_round_trips_without_overflowing_the_evaluator_stack() {
    lamedh::with_large_stack(|| {
        let env = env_with_hex();
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
