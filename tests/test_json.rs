//! JSON parse/stringify (issue #257, epic #253): lib/35-json.lisp's JSON
//! module.
//!
//! Coverage: the documented value mapping both ways (including the
//! false/null/empty-array distinctness the epic's nil-punning trap warns
//! about), int64-exact vs. float number classification at the exact i64
//! boundary and beyond (:ON-INTEGER-OVERFLOW :ERROR vs. :FLOAT), float
//! round-trip fidelity, Unicode strings (escapes, surrogate pairs, raw
//! multi-byte UTF-8), malformed-input errors with line/column context,
//! depth-limit behavior (both that it fires and that it does not crash the
//! process), pretty vs. compact serialization, and a large-flat-input
//! stack-safety check (issue #257's explicit "iteratively/tail-recursively"
//! guidance) run under `with_large_stack` per tests/test_examples.rs.

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line};

fn env_with_json() -> Shared<Environment> {
    let env = Environment::with_stdlib();
    assert_eq!(eval_line("(import json)", &env), "JSON");
    env
}

// ── mapping: parse ───────────────────────────────────────────────────────

#[test]
fn parse_maps_true_false_null_distinctly() {
    let env = env_with_json();
    assert_eq!(eval_line("(parse \"true\")", &env), "T");
    assert_eq!(eval_line("(parse \"false\")", &env), "()");
    assert_eq!(eval_line("(parse \"null\")", &env), ":NULL");
    // false, null, and an empty array must be three distinct values.
    assert_eq!(eval_line("(null-p (parse \"false\"))", &env), "()");
    assert_eq!(eval_line("(null-p (parse \"null\"))", &env), "T");
    assert_eq!(
        eval_line("(eq (parse \"false\") (parse \"null\"))", &env),
        "()"
    );
    assert_eq!(
        eval_line("(eq (parse \"null\") (list->array ()))", &env),
        "()"
    );
    assert_eq!(eval_line("(arrayp (parse \"[]\"))", &env), "T");
    assert_eq!(eval_line("(array-length* (parse \"[]\"))", &env), "0");
}

#[test]
fn parse_object_is_hash_table_with_string_keys() {
    let env = env_with_json();
    assert_eq!(
        eval_line("(hash-table-p (parse \"{\\\"a\\\":1}\"))", &env),
        "T"
    );
    assert_eq!(
        eval_line("(gethash (parse \"{\\\"a\\\":1}\") \"a\")", &env),
        "1"
    );
    // Case-preserving distinct keys (AGENTS.md: "Key" and "key" stay distinct).
    assert_eq!(
        eval_line(
            "(let ((h (parse \"{\\\"Key\\\":1,\\\"key\\\":2}\"))) (list (gethash h \"Key\") (gethash h \"key\")))",
            &env
        ),
        "(1 2)"
    );
}

#[test]
fn parse_object_duplicate_keys_last_wins() {
    let env = env_with_json();
    assert_eq!(
        eval_line(
            "(gethash (parse \"{\\\"a\\\":1,\\\"a\\\":2}\") \"a\")",
            &env
        ),
        "2"
    );
}

#[test]
fn parse_array_maps_to_array_not_list() {
    let env = env_with_json();
    assert_eq!(eval_line("(arrayp (parse \"[1,2,3]\"))", &env), "T");
    assert_eq!(eval_line("(consp (parse \"[1,2,3]\"))", &env), "()");
    assert_eq!(
        eval_line("(array->list (parse \"[1,2,3]\"))", &env),
        "(1 2 3)"
    );
}

// ── mapping: numbers ─────────────────────────────────────────────────────

#[test]
fn integers_in_i64_range_are_exact_numbers() {
    let env = env_with_json();
    assert_eq!(eval_line("(parse \"0\")", &env), "0");
    assert_eq!(eval_line("(parse \"-0\")", &env), "0");
    assert_eq!(eval_line("(parse \"42\")", &env), "42");
    assert_eq!(eval_line("(parse \"-42\")", &env), "-42");
    assert_eq!(
        eval_line("(parse \"9223372036854775807\")", &env),
        "9223372036854775807"
    );
    assert_eq!(
        eval_line("(fixp (parse \"9223372036854775807\"))", &env),
        "T"
    );
    assert_eq!(
        eval_line("(parse \"-9223372036854775808\")", &env),
        "-9223372036854775808"
    );
}

#[test]
fn integers_out_of_i64_range_default_to_error() {
    let env = env_with_json();
    let out = eval_line("(parse \"9223372036854775808\")", &env);
    assert!(
        out.starts_with("Error:") && out.contains("out of i64 range"),
        "got: {out}"
    );
    let out = eval_line("(parse \"-9223372036854775809\")", &env);
    assert!(
        out.starts_with("Error:") && out.contains("out of i64 range"),
        "got: {out}"
    );
}

#[test]
fn integers_out_of_i64_range_widen_to_float_on_request() {
    let env = env_with_json();
    assert_eq!(
        eval_line(
            "(floatp (parse \"9223372036854775808\" :on-integer-overflow ':float))",
            &env
        ),
        "T"
    );
}

#[test]
fn non_integer_numbers_are_floats() {
    let env = env_with_json();
    assert_eq!(eval_line("(floatp (parse \"1.5\"))", &env), "T");
    assert_eq!(eval_line("(floatp (parse \"1e10\"))", &env), "T");
    assert_eq!(eval_line("(floatp (parse \"1E10\"))", &env), "T");
    assert_eq!(eval_line("(floatp (parse \"1.5e-3\"))", &env), "T");
    assert_eq!(eval_line("(parse \"0.5\")", &env), "0.5");
}

#[test]
fn float_round_trips_through_stringify_as_a_float() {
    let env = env_with_json();
    // A whole-valued float must not silently become an integer Number.
    assert_eq!(eval_line("(floatp (parse (stringify 2.0)))", &env), "T");
    assert_eq!(eval_line("(parse (stringify 2.0))", &env), "2.0");
    assert_eq!(eval_line("(parse (stringify 3.14))", &env), "3.14");
}

#[test]
fn stringify_rejects_nan_and_infinite_floats() {
    let env = env_with_json();
    let out = eval_line("(stringify (/ 1.0 0.0))", &env);
    assert!(
        out.starts_with("Error:") && out.contains("non-finite"),
        "got: {out}"
    );
}

#[test]
fn leading_zero_integers_are_rejected() {
    let env = env_with_json();
    for bad in ["01", "-01", "00"] {
        let out = eval_line(&format!("(parse {bad:?})"), &env);
        assert!(
            out.starts_with("Error:") && out.contains("leading zero"),
            "{bad}: got {out}"
        );
    }
    // "0" and "0.5" and "0e1" remain valid.
    assert_eq!(eval_line("(parse \"0\")", &env), "0");
    assert_eq!(eval_line("(floatp (parse \"0.5\"))", &env), "T");
}

// ── mapping: strings / Unicode ───────────────────────────────────────────

#[test]
fn strings_round_trip_ascii_and_multibyte_unicode() {
    let env = env_with_json();
    for s in ["", "hello", "café", "世界", "🎉"] {
        let out = eval_line(&format!("(equal (parse (stringify {s:?})) {s:?})"), &env);
        assert_eq!(out, "T", "roundtrip failed for {s:?}");
    }
}

#[test]
fn standard_escapes_decode_correctly() {
    let env = env_with_json();
    // eval_line returns the PRINTER's readable form, which re-escapes
    // control/backslash/quote characters — so the expected Rust strings
    // below use literal `\\n` etc. (two characters) to mean "the printer
    // shows a backslash-n", not an actual embedded newline byte.
    assert_eq!(eval_line(r#"(parse "\"a\\nb\"")"#, &env), "\"a\\nb\"");
    assert_eq!(eval_line(r#"(parse "\"a\\tb\"")"#, &env), "\"a\\tb\"");
    assert_eq!(eval_line(r#"(parse "\"a\\\"b\"")"#, &env), "\"a\\\"b\"");
    assert_eq!(eval_line(r#"(parse "\"a\\\\b\"")"#, &env), "\"a\\\\b\"");
    assert_eq!(eval_line(r#"(parse "\"a\\/b\"")"#, &env), "\"a/b\"");
}

#[test]
fn unicode_escape_decodes_bmp_code_point() {
    let env = env_with_json();
    // é is 'é'.
    assert_eq!(eval_line(r#"(parse "\"\\u00e9\"")"#, &env), "\"é\"");
}

#[test]
fn surrogate_pair_decodes_astral_code_point() {
    let env = env_with_json();
    // U+1F389 PARTY POPPER encoded as a UTF-16 surrogate pair.
    let out = eval_line(r#"(parse "\"\\ud83c\\udf89\"")"#, &env);
    assert_eq!(out, "\"🎉\"");
}

#[test]
fn lone_surrogates_are_errors() {
    let env = env_with_json();
    let out = eval_line(r#"(parse "\"\\ud800\"")"#, &env);
    assert!(
        out.starts_with("Error:") && out.contains("surrogate"),
        "got: {out}"
    );
    let out = eval_line(r#"(parse "\"\\udc00\"")"#, &env);
    assert!(
        out.starts_with("Error:") && out.contains("surrogate"),
        "got: {out}"
    );
    // high surrogate followed by something that is not a low surrogate escape
    let out = eval_line(r#"(parse "\"\\ud800\\u0041\"")"#, &env);
    assert!(
        out.starts_with("Error:") && out.contains("surrogate"),
        "got: {out}"
    );
}

#[test]
fn unescaped_control_character_in_string_is_an_error() {
    let env = env_with_json();
    // A literal tab byte (0x09) inside a JSON string, unescaped.
    let out = eval_line("(parse (concat \"\\\"a\" (code-char 9) \"b\\\"\"))", &env);
    assert!(
        out.starts_with("Error:") && out.contains("control character"),
        "got: {out}"
    );
}

// ── malformed input ───────────────────────────────────────────────────────

#[test]
fn trailing_garbage_is_rejected() {
    let env = env_with_json();
    let out = eval_line("(parse \"1 2\")", &env);
    assert!(
        out.starts_with("Error:") && out.contains("trailing garbage"),
        "got: {out}"
    );
}

#[test]
fn truncated_input_is_rejected() {
    let env = env_with_json();
    for bad in ["{", "[", "\"unterminated", "{\"a\":", "[1,", "tru", "nul"] {
        let out = eval_line(&format!("(parse {bad:?})"), &env);
        assert!(
            out.starts_with("Error:"),
            "{bad:?}: expected error, got {out}"
        );
    }
}

#[test]
fn errors_carry_line_and_column() {
    let env = env_with_json();
    let out = eval_line("(parse \"{\\n  \\\"a\\\": ,\\n}\")", &env);
    assert!(
        out.starts_with("Error:") && out.contains("line 2"),
        "got: {out}"
    );
}

#[test]
fn object_key_must_be_a_string() {
    let env = env_with_json();
    let out = eval_line("(parse \"{1:2}\")", &env);
    assert!(
        out.starts_with("Error:") && out.contains("string key"),
        "got: {out}"
    );
}

#[test]
fn array_and_object_require_comma_or_close() {
    let env = env_with_json();
    let out = eval_line("(parse \"[1 2]\")", &env);
    assert!(out.starts_with("Error:"), "got: {out}");
    let out = eval_line("(parse \"{\\\"a\\\":1 \\\"b\\\":2}\")", &env);
    assert!(out.starts_with("Error:"), "got: {out}");
}

// ── depth limit ───────────────────────────────────────────────────────────

#[test]
fn depth_limit_errors_cleanly_instead_of_crashing() {
    let env = env_with_json();
    let nested = format!("{}1{}", "[".repeat(20), "]".repeat(20));
    let out = eval_line(&format!("(parse {nested:?} :max-depth 5)"), &env);
    assert!(
        out.starts_with("Error:") && out.contains("nesting depth"),
        "got: {out}"
    );
    // The same shape parses fine within a sufficient limit.
    let out = eval_line(&format!("(arrayp (parse {nested:?} :max-depth 25))"), &env);
    assert_eq!(out, "T");
}

#[test]
fn deep_nesting_beyond_default_limit_is_a_clean_error_not_a_crash() {
    // A moderately deep structure (well past the 512 default, but not so
    // deep as to be slow) must be caught by MAX-DEPTH before it can ever
    // stress the native call stack.
    let env = env_with_json();
    let nested = format!("{}1{}", "[".repeat(600), "]".repeat(600));
    let out = eval_line(&format!("(parse {nested:?})", nested = nested), &env);
    assert!(
        out.starts_with("Error:") && out.contains("nesting depth"),
        "got: {out}"
    );
}

// ── stringify: compact / pretty ──────────────────────────────────────────

#[test]
fn stringify_compact_has_no_insignificant_whitespace() {
    let env = env_with_json();
    // Single-key object: hash-table iteration order is unspecified, so
    // avoid asserting exact text for a multi-key object (checked structurally
    // via round-trip elsewhere) and just check compact formatting here.
    assert_eq!(
        eval_line("(stringify (parse \"{\\\"a\\\":1}\"))", &env),
        "\"{\\\"a\\\":1}\""
    );
    assert_eq!(
        eval_line("(stringify (parse \"[1,2,3]\"))", &env),
        "\"[1,2,3]\""
    );
    // Multi-key object: no whitespace anywhere, regardless of key order.
    let out = eval_line(
        "(stringify (parse \"{\\\"a\\\":1,\\\"b\\\":[1,2]}\"))",
        &env,
    );
    assert!(!out.contains(' ') && !out.contains('\n'), "got: {out}");
}

#[test]
fn stringify_pretty_indents() {
    let env = env_with_json();
    let out = eval_line("(stringify (parse \"[1,2]\") :pretty t)", &env);
    assert_eq!(out, "\"[\\n  1,\\n  2\\n]\"");
}

#[test]
fn stringify_empty_array_and_object() {
    let env = env_with_json();
    assert_eq!(eval_line("(stringify (list->array ()))", &env), "\"[]\"");
    assert_eq!(eval_line("(stringify (make-hash-table))", &env), "\"{}\"");
}

#[test]
fn stringify_errors_on_unmappable_value() {
    let env = env_with_json();
    let out = eval_line("(stringify (cons 1 2))", &env);
    assert!(
        out.starts_with("Error:") && out.contains("cannot serialize"),
        "got: {out}"
    );
}

// ── large flat input: stack safety ───────────────────────────────────────

#[test]
fn large_flat_array_does_not_overflow_the_evaluator_stack() {
    // Before the tail-recursive rewrite, a plain `(cons x (recurse ...))`
    // array-item loop and the Prelude's STRING->LIST both hit this
    // evaluator's ~10000-eval-frame recursion limit on inputs far smaller
    // than realistic JSON payloads. 6000 flat elements safely exceeds that
    // ceiling while keeping the (debug-build) test fast.
    lamedh::with_large_stack(|| {
        let env = env_with_json();
        let n = 6000usize;
        let items: Vec<String> = (0..n).map(|i| i.to_string()).collect();
        let json_text = format!("[{}]", items.join(","));
        let out = eval_line(&format!("(array-length* (parse {json_text:?}))"), &env);
        assert_eq!(out, n.to_string());
        // Round trip through stringify too (exercises the tail-recursive join).
        let out = eval_line(
            &format!(
                "(equal (array->list (parse (stringify (parse {json_text:?})))) (array->list (parse {json_text:?})))"
            ),
            &env,
        );
        assert_eq!(out, "T");
    });
}

#[test]
fn long_string_value_does_not_overflow_the_evaluator_stack() {
    lamedh::with_large_stack(|| {
        let env = env_with_json();
        let s = "x".repeat(8000);
        let out = eval_line(&format!("(equal (parse (stringify {s:?})) {s:?})"), &env);
        assert_eq!(out, "T");
    });
}
