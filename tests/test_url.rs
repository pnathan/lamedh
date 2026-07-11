//! URL parse/build, percent-encoding, and query-string parse/build (issue
//! #257, epic #253): lib/34-url.lisp's URL module.
//!
//! Coverage: full URL parse/build round-trip (scheme, userinfo, host, port,
//! path, query, fragment; IPv6 literal host), path-segment vs.
//! query-component percent-encoding using different safe-character sets,
//! decode round-trip including non-ASCII UTF-8, query-string parse/build
//! preserving repeated keys and order, malformed-escape and invalid-UTF-8
//! errors plus the lossy path, and a large-flat-input stack-safety check.

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line};

fn env_with_url() -> Shared<Environment> {
    let env = Environment::with_stdlib();
    assert_eq!(eval_line("(import url)", &env), "URL");
    env
}

// ── percent-encoding: path segment vs. query component ──────────────────

#[test]
fn path_segment_and_query_component_use_different_safe_sets() {
    let env = env_with_url();
    // "/" is never safe in a path segment; "&"/"="/"+" are never safe in a
    // query component. Sub-delims like "!" stay literal in a path segment
    // but are percent-encoded in a query component.
    assert_eq!(
        eval_line("(encode-path-segment \"a/b\")", &env),
        "\"a%2Fb\""
    );
    assert_eq!(eval_line("(encode-path-segment \"a!b\")", &env), "\"a!b\"");
    assert_eq!(
        eval_line("(encode-query-component \"a&b=c+d\")", &env),
        "\"a%26b%3Dc%2Bd\""
    );
    assert_eq!(
        eval_line("(encode-query-component \"a!b\")", &env),
        "\"a%21b\""
    );
}

#[test]
fn decode_is_the_context_free_inverse_of_both_encoders() {
    let env = env_with_url();
    for s in ["a/b?c=d", "a b&c", "héllo/wörld", "🎉"] {
        assert_eq!(
            eval_line(
                &format!("(equal (decode (encode-path-segment {s:?})) {s:?})"),
                &env
            ),
            "T",
            "path decode roundtrip failed for {s:?}"
        );
        assert_eq!(
            eval_line(
                &format!("(equal (decode (encode-query-component {s:?})) {s:?})"),
                &env
            ),
            "T",
            "query decode roundtrip failed for {s:?}"
        );
    }
    // DECODE-PATH-SEGMENT / DECODE-QUERY-COMPONENT are the same operation.
    assert_eq!(
        eval_line(
            "(equal (decode-path-segment \"a%2Fb\") (decode-query-component \"a%2Fb\"))",
            &env
        ),
        "T"
    );
}

#[test]
fn malformed_percent_escapes_are_errors() {
    let env = env_with_url();
    let out = eval_line("(decode \"%zz\")", &env);
    assert!(
        out.starts_with("Error:") && out.contains("hex digit"),
        "got: {out}"
    );
    let out = eval_line("(decode \"%4\")", &env);
    assert!(
        out.starts_with("Error:") && out.contains("truncated"),
        "got: {out}"
    );
    let out = eval_line("(decode \"%\")", &env);
    assert!(
        out.starts_with("Error:") && out.contains("truncated"),
        "got: {out}"
    );
}

#[test]
fn invalid_utf8_after_decoding_is_strict_by_default_with_a_lossy_escape_hatch() {
    let env = env_with_url();
    // %FF is never a valid standalone UTF-8 byte.
    let out = eval_line("(decode \"%FF\")", &env);
    assert!(out.starts_with("Error:"), "got: {out}");
    let out = eval_line("(decode \"%FF\" :lossy t)", &env);
    assert_eq!(out, "\"\u{fffd}\"");
}

// ── full URL parse/build ──────────────────────────────────────────────────

#[test]
fn parse_extracts_every_component() {
    let env = env_with_url();
    let url = "https://user:pw@example.com:8080/a/b?x=1&y=2#frag";
    assert_eq!(
        eval_line(&format!("(scheme (parse {url:?}))"), &env),
        "\"https\""
    );
    assert_eq!(
        eval_line(&format!("(userinfo (parse {url:?}))"), &env),
        "\"user:pw\""
    );
    assert_eq!(
        eval_line(&format!("(host (parse {url:?}))"), &env),
        "\"example.com\""
    );
    assert_eq!(eval_line(&format!("(port (parse {url:?}))"), &env), "8080");
    assert_eq!(
        eval_line(&format!("(path (parse {url:?}))"), &env),
        "\"/a/b\""
    );
    assert_eq!(
        eval_line(&format!("(query (parse {url:?}))"), &env),
        "\"x=1&y=2\""
    );
    assert_eq!(
        eval_line(&format!("(fragment (parse {url:?}))"), &env),
        "\"frag\""
    );
}

#[test]
fn parse_build_round_trips() {
    let env = env_with_url();
    for url in [
        "https://example.com/a/b?x=1#f",
        "http://example.com",
        "https://user@example.com/path",
        "/relative/path?q=1",
        "mailto:foo@example.com",
    ] {
        assert_eq!(
            eval_line(&format!("(build (parse {url:?}))"), &env),
            format!("{url:?}"),
            "round trip failed for {url}"
        );
    }
}

#[test]
fn ipv6_literal_host_and_port() {
    let env = env_with_url();
    let url = "http://[::1]:8080/path";
    assert_eq!(
        eval_line(&format!("(host (parse {url:?}))"), &env),
        "\"[::1]\""
    );
    assert_eq!(eval_line(&format!("(port (parse {url:?}))"), &env), "8080");
    assert_eq!(
        eval_line(&format!("(build (parse {url:?}))"), &env),
        format!("{url:?}")
    );
}

#[test]
fn missing_components_are_nil_except_path() {
    let env = env_with_url();
    assert_eq!(eval_line("(scheme (parse \"/just/a/path\"))", &env), "()");
    assert_eq!(eval_line("(host (parse \"/just/a/path\"))", &env), "()");
    assert_eq!(eval_line("(port (parse \"/just/a/path\"))", &env), "()");
    assert_eq!(
        eval_line("(path (parse \"/just/a/path\"))", &env),
        "\"/just/a/path\""
    );
    assert_eq!(eval_line("(query (parse \"/just/a/path\"))", &env), "()");
    assert_eq!(eval_line("(fragment (parse \"/just/a/path\"))", &env), "()");
}

#[test]
fn invalid_port_is_an_error() {
    let env = env_with_url();
    let out = eval_line("(parse \"http://example.com:notaport/\")", &env);
    assert!(
        out.starts_with("Error:") && out.contains("invalid port"),
        "got: {out}"
    );
}

// ── query-string parse/build: repeated keys, ordering ────────────────────

#[test]
fn parse_query_preserves_repeated_keys_and_order() {
    let env = env_with_url();
    assert_eq!(
        eval_line("(parse-query \"a=1&b=2&a=3\")", &env),
        "((\"a\" . \"1\") (\"b\" . \"2\") (\"a\" . \"3\"))"
    );
}

#[test]
fn build_query_is_the_inverse_of_parse_query() {
    let env = env_with_url();
    assert_eq!(
        eval_line(
            "(build-query (list (cons \"a\" \"1\") (cons \"b\" \"hello world\") (cons \"a\" \"3\")))",
            &env
        ),
        "\"a=1&b=hello%20world&a=3\""
    );
    assert_eq!(
        eval_line(
            "(equal (parse-query (build-query (list (cons \"a\" \"1\") (cons \"b\" \"2\") (cons \"a\" \"3\")))) (list (cons \"a\" \"1\") (cons \"b\" \"2\") (cons \"a\" \"3\")))",
            &env
        ),
        "T"
    );
}

// ── stack safety ──────────────────────────────────────────────────────────

#[test]
fn large_input_round_trips_without_overflowing_the_evaluator_stack() {
    lamedh::with_large_stack(|| {
        let env = env_with_url();
        let s = "a b&c=".repeat(1000); // 6000 chars
        let out = eval_line(
            &format!("(equal (decode (encode-query-component {s:?})) {s:?})"),
            &env,
        );
        assert_eq!(out, "T");
    });
}
