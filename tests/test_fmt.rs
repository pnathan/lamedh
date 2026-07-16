//! Acceptance tests for the canonical formatter (`lamedh --fmt` /
//! `--fmt-check`), backed by [`lamedh::formatter::format_source`].
//!
//! Three properties matter for a formatter that claims to be conservative:
//!
//! 1. **Idempotence** — formatting an already-formatted file is a no-op.
//! 2. **Semantic preservation** — formatting never changes what a file
//!    *means*: reading the original and reading the formatted output must
//!    yield the same forms.
//! 3. **Region preservation** — string/char literal/comment/shebang content
//!    is reproduced exactly, never reflowed or rewritten.

use lamedh::environment::Environment;
use lamedh::formatter::format_source;
use lamedh::printer;
use lamedh::reader;

/// Collect `*.lisp` files under `dir` (recursively), sorted for determinism.
fn lisp_files(dir: &str) -> Vec<String> {
    let mut out = Vec::new();
    collect(std::path::Path::new(dir), &mut out);
    out.sort();
    out
}

fn collect(dir: &std::path::Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect(&path, out);
        } else if path.extension().and_then(|e| e.to_str()) == Some("lisp") {
            out.push(path.to_string_lossy().into_owned());
        }
    }
}

fn root() -> String {
    env!("CARGO_MANIFEST_DIR").to_string()
}

fn lib_corpus() -> Vec<String> {
    lisp_files(&format!("{}/lib", root()))
}

/// Read every top-level form from `src` and render each with [`printer::print`],
/// joined by newlines. Two sources are "semantically equal" (for this test's
/// purposes) iff this rendering agrees.
fn printed_forms(src: &str, env: &lamedh::Shared<Environment>) -> String {
    let forms = reader::read_all(src, env).expect("source must parse");
    forms
        .iter()
        .map(printer::print)
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn idempotent_over_lib_corpus() {
    let corpus = lib_corpus();
    assert!(!corpus.is_empty(), "no lib/*.lisp files found");
    let mut failures = Vec::new();
    for path in &corpus {
        let src = std::fs::read_to_string(path).expect("read lib file");
        let once = format_source(&src);
        let twice = format_source(&once);
        if once != twice {
            failures.push(path.clone());
        }
    }
    assert!(
        failures.is_empty(),
        "format(format(x)) != format(x) for: {failures:?}"
    );
}

#[test]
fn semantic_preservation_over_lib_corpus() {
    lamedh::with_large_stack(|| {
        let corpus = lib_corpus();
        assert!(!corpus.is_empty(), "no lib/*.lisp files found");
        let env = Environment::with_stdlib_fresh();
        let mut failures = Vec::new();
        for path in &corpus {
            let src = std::fs::read_to_string(path).expect("read lib file");
            let formatted = format_source(&src);
            let before = printed_forms(&src, &env);
            let after = printed_forms(&formatted, &env);
            if before != after {
                failures.push(path.clone());
            }
        }
        assert!(
            failures.is_empty(),
            "reading before/after formatting disagreed for: {failures:?}"
        );
    });
}

#[test]
fn comment_and_string_regions_are_byte_identical() {
    // Strings containing parens and semicolons, a char literal for each of
    // '(' and ')' (which must not perturb paren-depth counting), and a
    // shebang line, all mis-indented on purpose.
    let src = "#!/usr/bin/env lamedh\n\
               (defun weird ()\n\
               \"string with (parens) and ; a fake comment inside\"\n\
               \t  (let ((open-paren '(')\n\
               \t\t(close-paren ')'))\n\
               \t\t   (list open-paren close-paren)))\n\
               ; a real trailing comment with (parens) too   \n";
    let formatted = format_source(src);

    // The shebang line is untouched at column 0.
    assert!(formatted.starts_with("#!/usr/bin/env lamedh\n"));

    // The string literal's content survived byte-for-byte, unindented and
    // unreflowed.
    assert!(
        formatted.contains("\"string with (parens) and ; a fake comment inside\""),
        "{formatted}"
    );

    // Both char literals survived, and their parens did not desynchronize
    // paren-depth counting (the file must still parse and read the same).
    assert!(formatted.contains("'('"), "{formatted}");
    assert!(formatted.contains("')'"), "{formatted}");

    // Semantic preservation: reading before and after agrees.
    let env = Environment::new_with_builtins();
    let before = printed_forms(src, &env);
    let after = printed_forms(&formatted, &env);
    assert_eq!(before, after);

    // Idempotent.
    assert_eq!(format_source(&formatted), formatted);
}

#[test]
fn multiline_string_body_is_never_touched() {
    let src =
        "(def x\n  \"line one\n     line two, weirdly indented\n(not a paren)\")\n(def y 2)\n";
    let formatted = format_source(src);
    assert!(
        formatted.contains("\"line one\n     line two, weirdly indented\n(not a paren)\""),
        "{formatted}"
    );
}

#[test]
fn block_comment_body_is_never_touched() {
    let src = "(foo)\n#| a block comment\n   with irregular   spacing\nand (unbalanced parens |#\n(bar)\n";
    let formatted = format_source(src);
    assert!(
        formatted
            .contains("#| a block comment\n   with irregular   spacing\nand (unbalanced parens |#"),
        "{formatted}"
    );
}

#[test]
fn reindents_and_strips_trailing_whitespace() {
    let src = "(defun sq (x)   \n(* x x))\n";
    assert_eq!(format_source(src), "(defun sq (x)\n  (* x x))\n");
}

#[test]
fn collapses_long_blank_runs_and_trims_trailing_blanks() {
    let src = "(a)\n\n\n\n\n(b)\n\n\n\n";
    assert_eq!(format_source(src), "(a)\n\n\n(b)\n");
}
