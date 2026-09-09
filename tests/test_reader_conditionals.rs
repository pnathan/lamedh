//! Integration tests for issue #459: `#+feature form` / `#-feature form`
//! reader conditionals.
//!
//! `#+feature form` reads `form` normally when `feature` is present in the
//! reading environment's reader-feature set, and reads it as if it were
//! whitespace otherwise -- `form` is still parsed structurally (so a
//! malformed skipped form is still a parse error, and surrounding list
//! parsing is unaffected) but contributes no value. `#-feature form` is the
//! complement. Feature expressions combine atomic feature names with
//! `(AND ...)`, `(OR ...)`, `(NOT e)`.
//!
//! The Rust reference seeds every environment with the reader features
//! `RUST` (host identity), `READER-FLOATS`, `READER-RADIX`, and
//! `READER-BLOCK-COMMENTS` (capabilities matching KERNEL.md Part XII items
//! this reference already implements) -- see `DEFAULT_READER_FEATURES` in
//! `src/environment.rs`. `NONEXISTENT-FEATURE` below stands in for any name
//! absent from that set.

use lamedh::Shared;
use lamedh::environment::Environment;
use lamedh::printer::print;
use lamedh::reader::{read, read_all};

fn env() -> Shared<Environment> {
    Environment::new_with_builtins()
}

fn read_str(src: &str) -> String {
    let e = env();
    print(&read(src, &e).unwrap())
}

fn read_all_strs(src: &str) -> Vec<String> {
    let e = env();
    read_all(src, &e)
        .unwrap()
        .into_iter()
        .map(|v| print(&v))
        .collect()
}

// ---------------------------------------------------------------------------
// Basic present/absent, `#+` and `#-`
// ---------------------------------------------------------------------------

#[test]
fn plus_present_feature_reads_form() {
    assert_eq!(read_str("#+rust 42"), "42");
}

#[test]
fn plus_absent_feature_skips_form_in_list() {
    // The guarded form contributes no value: surrounding list elements are
    // unaffected, and there is no NIL or other placeholder left behind.
    assert_eq!(
        read_all_strs("(a #+nonexistent-feature 1 b)"),
        vec!["(A B)"]
    );
}

#[test]
fn minus_present_feature_skips_form() {
    assert_eq!(read_all_strs("(a #-rust 1 b)"), vec!["(A B)"]);
}

#[test]
fn minus_absent_feature_reads_form() {
    assert_eq!(
        read_all_strs("(a #-nonexistent-feature 1 b)"),
        vec!["(A 1 B)"]
    );
}

#[test]
fn feature_name_is_case_insensitive_and_colon_optional() {
    assert_eq!(read_str("#+RUST 1"), "1");
    assert_eq!(read_str("#+:rust 1"), "1");
    assert_eq!(read_str("#+:RuSt 1"), "1");
}

// ---------------------------------------------------------------------------
// A skipped form is still parsed structurally, not merely scanned for a
// closing delimiter: nested parens/quotes inside it are respected, and a
// malformed skipped form is still a parse error.
// ---------------------------------------------------------------------------

#[test]
fn skipped_form_is_read_structurally_not_textually() {
    // If the reader merely scanned for a matching paren depth this would
    // still pass, but if it merely skipped to the next whitespace it would
    // wrongly stop after `(a`.
    assert_eq!(
        read_all_strs("(x #+nonexistent-feature (a (b c) 'd) y)"),
        vec!["(X Y)"]
    );
}

#[test]
fn malformed_skipped_form_is_still_a_parse_error() {
    let e = env();
    assert!(read_all("(x #+nonexistent-feature (a b y)", &e).is_err());
}

// ---------------------------------------------------------------------------
// Whole-buffer conditioned out: no value at all, not an error, and not NIL.
// ---------------------------------------------------------------------------

#[test]
fn entirely_conditioned_out_buffer_reads_no_forms() {
    let e = env();
    assert_eq!(
        read_all("#+nonexistent-feature (only form)", &e).unwrap(),
        vec![]
    );
}

#[test]
fn conditioned_out_form_amid_others_reads_the_rest() {
    let e = env();
    let forms = read_all("1 #+nonexistent-feature 2 3", &e).unwrap();
    let rendered: Vec<String> = forms.iter().map(|v| print(&v)).collect();
    assert_eq!(rendered, vec!["1", "3"]);
}

// ---------------------------------------------------------------------------
// `#+`/`#-` always promises a following form, whether that form ends up
// kept or discarded -- so a directive with nothing after it is a hard parse
// error in both cases, not just when the feature is absent. (An earlier
// version of this reader only enforced this for the discard branch, so
// `#+rust` at true EOF silently meant "no more forms" while the identical
// shape with an absent feature errored -- fixed for consistency.)
// ---------------------------------------------------------------------------

#[test]
fn bare_conditional_at_true_eof_is_an_error_regardless_of_feature_truth() {
    let e = env();
    assert!(read_all("#+rust", &e).is_err());
    assert!(read_all("#+nonexistent-feature", &e).is_err());
    assert!(read_all("#-rust", &e).is_err());
    assert!(read_all("#-nonexistent-feature", &e).is_err());
}

#[test]
fn conditional_missing_its_form_inside_a_list_is_an_error() {
    let e = env();
    // `#+rust` is true, so a form is expected before `)`, but there isn't
    // one -- this must fail, not silently produce `(A)`.
    assert!(read_all("(a #+rust)", &e).is_err());
    // Same for the discard branch: `#+nonexistent-feature` needs a form to
    // discard before `)`.
    assert!(read_all("(a #+nonexistent-feature)", &e).is_err());
}

// ---------------------------------------------------------------------------
// `(AND ...)` / `(OR ...)` / `(NOT ...)` feature-expression combinations.
// ---------------------------------------------------------------------------

#[test]
fn and_combination() {
    assert_eq!(read_str("#+(and rust reader-floats) 1"), "1");
    assert_eq!(
        read_all_strs("(a #+(and rust nonexistent-feature) 1 b)"),
        vec!["(A B)"]
    );
}

#[test]
fn or_combination() {
    assert_eq!(read_str("#+(or nonexistent-feature rust) 1"), "1");
    assert_eq!(
        read_all_strs("(a #+(or nonexistent-1 nonexistent-2) 1 b)"),
        vec!["(A B)"]
    );
}

#[test]
fn not_combination() {
    assert_eq!(read_str("#+(not nonexistent-feature) 1"), "1");
    assert_eq!(read_all_strs("(a #+(not rust) 1 b)"), vec!["(A B)"]);
}

#[test]
fn nested_combinations() {
    // (or (and rust (not nonexistent-feature)) nonexistent-feature)  =>  T
    assert_eq!(
        read_str("#+(or (and rust (not nonexistent-feature)) nonexistent-feature) 99"),
        "99"
    );
}

// ---------------------------------------------------------------------------
// Nested / chained `#+`/`#-` directives.
// ---------------------------------------------------------------------------

#[test]
fn chained_directives_all_true_reads_form() {
    assert_eq!(read_str("#+rust #-nonexistent-feature 7"), "7");
}

#[test]
fn chained_directives_first_false_skips_the_rest_as_one_unit() {
    // #+nonexistent-feature's guarded form is the ENTIRE `#+rust 7`, so this
    // reads no value at all, not `7`.
    assert_eq!(
        read_all_strs("(a #+nonexistent-feature #+rust 7 b)"),
        vec!["(A B)"]
    );
}

#[test]
fn directive_guarding_another_directive_that_keeps() {
    // #-nonexistent-feature is true (feature absent), so its guarded form
    // `#+rust 7` is read normally, which itself keeps and reads `7`.
    assert_eq!(read_str("#-nonexistent-feature #+rust 7"), "7");
}

// ---------------------------------------------------------------------------
// Malformed feature expressions are hard parse errors, not silently false.
// ---------------------------------------------------------------------------

#[test]
fn unknown_combinator_is_an_error() {
    let e = env();
    assert!(read("#+(xor a b) 1", &e).is_err());
}

#[test]
fn non_symbol_feature_atom_is_an_error() {
    let e = env();
    assert!(read("#+\"rust\" 1", &e).is_err());
    assert!(read("#+42 1", &e).is_err());
}

#[test]
fn not_with_wrong_arity_is_an_error() {
    let e = env();
    assert!(read("#+(not a b) 1", &e).is_err());
    assert!(read("#+(not) 1", &e).is_err());
}

// ---------------------------------------------------------------------------
// Interaction with existing shebang-stripping and comment handling.
// ---------------------------------------------------------------------------

#[test]
fn conditional_after_shebang_and_comments() {
    let e = env();
    let src = "#!/usr/bin/env lamedh\n; a comment\n#+rust 5 ; trailing comment\n";
    assert_eq!(print(&read(src, &e).unwrap()), "5");
}

#[test]
fn conditional_after_block_comment() {
    assert_eq!(read_str("#| a block comment |# #+rust 1"), "1");
}

// ---------------------------------------------------------------------------
// Host/embedder-set reader features (Environment API).
// ---------------------------------------------------------------------------

#[test]
fn host_can_add_and_remove_reader_features() {
    let e = env();
    assert!(!e.reader_feature_enabled("sbcl"));
    e.enable_reader_feature("sbcl");
    assert!(e.reader_feature_enabled("SBCL"));
    assert_eq!(print(&read("#+sbcl 1", &e).unwrap()), "1");

    e.disable_reader_feature("SBCL");
    assert!(!e.reader_feature_enabled("sbcl"));
}

#[test]
fn default_reader_features_include_host_identity_and_capabilities() {
    let e = env();
    let features = e.reader_features_list();
    for expected in [
        "RUST",
        "READER-FLOATS",
        "READER-RADIX",
        "READER-BLOCK-COMMENTS",
    ] {
        assert!(
            features.iter().any(|f| f == expected),
            "expected default reader feature {expected} in {features:?}"
        );
    }
}
