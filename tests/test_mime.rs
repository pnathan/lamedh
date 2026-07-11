//! Case-insensitive multi-value headers and Content-Type parse/build (issue
//! #257, epic #253): lib/36-mime.lisp's MIME module.
//!
//! Coverage: case-insensitive header lookup while preserving original
//! value/casing, the multi-value accessor never collapsing repeated headers
//! (the ticket's explicit Set-Cookie requirement), Content-Type parse/build
//! round-trip with bare-token and quoted-string parameter values (including
//! backslash-escaped quotes), and malformed-input errors.

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line};

fn env_with_mime() -> Shared<Environment> {
    let env = Environment::with_stdlib();
    assert_eq!(eval_line("(import mime)", &env), "MIME");
    env
}

// ── headers ────────────────────────────────────────────────────────────

#[test]
fn header_name_equality_is_case_insensitive() {
    let env = env_with_mime();
    assert_eq!(
        eval_line("(header-name= \"Content-Type\" \"content-type\")", &env),
        "T"
    );
    assert_eq!(
        eval_line("(header-name= \"Content-Type\" \"X-Other\")", &env),
        "()"
    );
}

#[test]
fn headers_get_is_case_insensitive_and_preserves_value() {
    let env = env_with_mime();
    let hs = "(list (cons \"Content-Type\" \"text/html\"))";
    assert_eq!(
        eval_line(&format!("(headers-get {hs} \"content-type\")"), &env),
        "\"text/html\""
    );
    assert_eq!(
        eval_line(&format!("(headers-get {hs} \"CONTENT-TYPE\")"), &env),
        "\"text/html\""
    );
    assert_eq!(
        eval_line(&format!("(headers-get {hs} \"missing\")"), &env),
        "()"
    );
}

#[test]
fn multi_value_headers_are_never_collapsed() {
    let env = env_with_mime();
    let hs = "(headers-add (headers-add (list) \"Set-Cookie\" \"a=1\") \"Set-Cookie\" \"b=2\")";
    assert_eq!(
        eval_line(&format!("(headers-get-all {hs} \"set-cookie\")"), &env),
        "(\"a=1\" \"b=2\")"
    );
    // HEADERS-GET only returns the first.
    assert_eq!(
        eval_line(&format!("(headers-get {hs} \"set-cookie\")"), &env),
        "\"a=1\""
    );
    // Original casing is preserved for both entries.
    assert_eq!(
        eval_line(&format!("(headers-names {hs})"), &env),
        "(\"Set-Cookie\")"
    );
}

#[test]
fn headers_set_replaces_every_matching_entry() {
    let env = env_with_mime();
    let hs = "(headers-add (headers-add (list) \"X-A\" \"1\") \"x-a\" \"2\")";
    let replaced = format!("(headers-set {hs} \"X-A\" \"3\")");
    assert_eq!(
        eval_line(&format!("(headers-get-all {replaced} \"x-a\")"), &env),
        "(\"3\")"
    );
}

#[test]
fn headers_remove_drops_every_matching_entry() {
    let env = env_with_mime();
    let hs = "(headers-add (headers-add (list) \"X-A\" \"1\") \"X-B\" \"2\")";
    let removed = format!("(headers-remove {hs} \"x-a\")");
    assert_eq!(
        eval_line(&format!("(headers-get {removed} \"x-a\")"), &env),
        "()"
    );
    assert_eq!(
        eval_line(&format!("(headers-get {removed} \"x-b\")"), &env),
        "\"2\""
    );
}

#[test]
fn headers_names_are_first_seen_casing_deduplicated() {
    let env = env_with_mime();
    let hs = "(headers-add (headers-add (headers-add (list) \"X-A\" \"1\") \"x-a\" \"2\") \"X-B\" \"3\")";
    assert_eq!(
        eval_line(&format!("(headers-names {hs})"), &env),
        "(\"X-A\" \"X-B\")"
    );
}

// ── Content-Type ──────────────────────────────────────────────────────────

#[test]
fn parse_content_type_extracts_type_subtype_and_parameters() {
    let env = env_with_mime();
    assert_eq!(
        eval_line(
            "(cdr (assoc 'type (parse-content-type \"text/html; charset=UTF-8\")))",
            &env
        ),
        "\"text\""
    );
    assert_eq!(
        eval_line(
            "(cdr (assoc 'subtype (parse-content-type \"text/html; charset=UTF-8\")))",
            &env
        ),
        "\"html\""
    );
    assert_eq!(
        eval_line(
            "(content-type-parameter (parse-content-type \"text/html; charset=UTF-8\") \"charset\")",
            &env
        ),
        "\"UTF-8\""
    );
    // Parameter lookup is case-insensitive.
    assert_eq!(
        eval_line(
            "(content-type-parameter (parse-content-type \"text/html; charset=UTF-8\") \"CHARSET\")",
            &env
        ),
        "\"UTF-8\""
    );
}

#[test]
fn parse_content_type_handles_quoted_values_with_escapes() {
    let env = env_with_mime();
    let ct = r#""multipart/form-data; boundary=\"a \\\"b\\\" c\"""#;
    let out = eval_line(
        &format!("(content-type-parameter (parse-content-type {ct}) \"boundary\")"),
        &env,
    );
    assert_eq!(out, "\"a \\\"b\\\" c\"");
}

#[test]
fn build_content_type_round_trips_through_parse() {
    let env = env_with_mime();
    assert_eq!(
        eval_line(
            "(build-content-type \"text\" \"html\" (list (cons \"charset\" \"utf-8\")))",
            &env
        ),
        "\"text/html; charset=utf-8\""
    );
    // A value needing quoting (contains a space) is quoted on build and
    // recovered exactly on parse.
    let built = eval_line(
        "(build-content-type \"multipart\" \"form-data\" (list (cons \"boundary\" \"a b\")))",
        &env,
    );
    assert!(built.contains("\\\"a b\\\""), "got: {built}");
    assert_eq!(
        eval_line(
            "(content-type-parameter (parse-content-type (build-content-type \"multipart\" \"form-data\" (list (cons \"boundary\" \"a b\")))) \"boundary\")",
            &env
        ),
        "\"a b\""
    );
}

#[test]
fn missing_slash_in_media_type_is_an_error() {
    let env = env_with_mime();
    let out = eval_line("(parse-content-type \"texthtml\")", &env);
    assert!(
        out.starts_with("Error:") && out.contains("missing '/'"),
        "got: {out}"
    );
}

#[test]
fn unterminated_quoted_value_is_an_error() {
    let env = env_with_mime();
    let out = eval_line(
        "(parse-content-type \"text/plain; charset=\\\"utf-8\")",
        &env,
    );
    assert!(
        out.starts_with("Error:") && out.contains("unterminated"),
        "got: {out}"
    );
}
