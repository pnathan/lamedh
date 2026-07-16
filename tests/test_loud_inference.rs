//! Loud type inference: `defun*` silently falls back to a plain lambda when
//! HM inference fails, which makes "did it compile, and if not why" invisible
//! from Lisp. These three introspection builtins make it observable:
//!
//! - `(signature 'fn)` — the inferred type signature as a readable sexpr, or
//!   `NIL` for an untyped function.
//! - `(compiled-p 'fn)` — the execution tier that will actually run:
//!   `NATIVE` (Cranelift), `CLOSURE` (portable closure edition only), or
//!   `NIL` (plain interpreted).
//! - `(why-not-typed 'fn)` — for a `defun*` that fell back, the recorded
//!   inference-failure reason; `NIL` if typed or never a `defun*` candidate.

mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

/// The printer renders `NIL` as either token depending on call site; accept
/// both rather than pinning one (see other suites, e.g. test_net.rs).
fn is_nil(s: &str) -> bool {
    s == "NIL" || s == "()"
}

#[test]
fn signature_of_a_typed_defun_star() {
    let e = env_with_stdlib();
    eval_line("(defun* add (x int64) (y int64) (+ x y))", &e);
    assert_eq!(eval_line("(signature 'add)", &e), "(INT64 INT64 -> INT64)");
}

#[test]
fn signature_of_a_fully_inferred_defun_star() {
    let e = env_with_stdlib();
    // No type hints at all: x's type is pinned by the `1` literal.
    eval_line("(defun* inc (x) (+ x 1))", &e);
    assert_eq!(eval_line("(signature 'inc)", &e), "(INT64 -> INT64)");
}

#[test]
fn signature_is_nil_for_untyped_functions() {
    let e = env_with_stdlib();
    // Never a defun* candidate at all.
    eval_line("(defun plain (x) x)", &e);
    assert!(is_nil(&eval_line("(signature 'plain)", &e)));
    // A defun* that fell back to a plain lambda.
    eval_line("(defun* mk (a b) (cons a b))", &e);
    assert!(is_nil(&eval_line("(signature 'mk)", &e)));
}

#[test]
fn compiled_p_is_native_under_the_jit_feature() {
    let e = env_with_stdlib();
    eval_line("(defun* add (x int64) (y int64) (+ x y))", &e);
    #[cfg(feature = "jit")]
    assert_eq!(eval_line("(compiled-p 'add)", &e), "NATIVE");
    #[cfg(not(feature = "jit"))]
    assert_eq!(eval_line("(compiled-p 'add)", &e), "CLOSURE");
}

#[test]
fn compiled_p_is_nil_for_untyped_and_unknown_functions() {
    let e = env_with_stdlib();
    eval_line("(defun sq (x) (* x x))", &e); // checked, not compileable
    assert!(is_nil(&eval_line("(compiled-p 'sq)", &e)));
    eval_line("(defun* mk (a b) (cons a b))", &e); // defun* fallback
    assert!(is_nil(&eval_line("(compiled-p 'mk)", &e)));
    // Not a function at all.
    assert!(is_nil(&eval_line("(compiled-p 'totally-unbound-name)", &e)));
}

#[test]
fn why_not_typed_reports_the_inference_failure_reason() {
    let e = env_with_stdlib();
    // `cons` is outside the typed core: a concrete, actionable reason, not a
    // generic "inference failed".
    eval_line("(defun* mk (a b) (cons a b))", &e);
    let out = eval_line("(why-not-typed 'mk)", &e);
    assert!(!is_nil(&out), "expected a reason, got: {out}");
    assert!(
        out.to_uppercase().contains("CONS"),
        "reason should name the untypeable operation: got {out}"
    );
    // The function still runs correctly despite the fallback.
    assert_eq!(eval_line("(mk 1 2)", &e), "(1 . 2)");
}

#[test]
fn why_not_typed_is_nil_for_typed_functions() {
    let e = env_with_stdlib();
    eval_line("(defun* add (x int64) (y int64) (+ x y))", &e);
    assert!(is_nil(&eval_line("(why-not-typed 'add)", &e)));
}

#[test]
fn why_not_typed_is_nil_for_plain_defun_functions() {
    let e = env_with_stdlib();
    // A `defun` (never `defun*`) that also fails to compile natively still
    // reports NIL here — the reason lives in `explain-compile`, not
    // `why-not-typed`, which is specifically the `defun*` fallback ledger.
    eval_line("(defun sq (x) (* x x))", &e);
    assert!(is_nil(&eval_line("(why-not-typed 'sq)", &e)));
    // Never defined at all.
    assert!(is_nil(&eval_line(
        "(why-not-typed 'totally-unbound-name)",
        &e
    )));
}

#[test]
fn redefinition_updates_all_three_surfaces() {
    let e = env_with_stdlib();
    eval_line("(defun* g (x) (+ x 1))", &e);
    assert_eq!(eval_line("(g 5)", &e), "6");
    assert_eq!(eval_line("(signature 'g)", &e), "(INT64 -> INT64)");
    assert!(is_nil(&eval_line("(why-not-typed 'g)", &e)));
    #[cfg(feature = "jit")]
    assert_eq!(eval_line("(compiled-p 'g)", &e), "NATIVE");

    // Redefine as an untyped function (cons is outside the typed core).
    eval_line("(defun* g (x) (cons x x))", &e);
    assert_eq!(eval_line("(g 5)", &e), "(5 . 5)");
    assert!(is_nil(&eval_line("(signature 'g)", &e)));
    assert!(is_nil(&eval_line("(compiled-p 'g)", &e)));
    let reason = eval_line("(why-not-typed 'g)", &e);
    assert!(!is_nil(&reason), "expected a reason, got: {reason}");

    // Redefine back to typed: why-not-typed clears, signature returns.
    eval_line("(defun* g (x) (+ x 2))", &e);
    assert_eq!(eval_line("(g 5)", &e), "7");
    assert_eq!(eval_line("(signature 'g)", &e), "(INT64 -> INT64)");
    assert!(is_nil(&eval_line("(why-not-typed 'g)", &e)));
}

#[test]
fn signature_and_compiled_p_respect_the_live_binding_after_overwrite() {
    // Regression, mirroring the existing check-type staleness test: if a
    // typed defun* is later shadowed by a plain (non-defun*) DEF/lambda
    // binding, the typed registry entry is stale and must not be reported.
    let e = env_with_stdlib();
    eval_line("(defun* h (x int64) (+ x 1))", &e);
    assert_eq!(eval_line("(signature 'h)", &e), "(INT64 -> INT64)");
    eval_line("(def h (lambda (x) (cons x x)))", &e);
    assert!(is_nil(&eval_line("(signature 'h)", &e)));
    assert!(is_nil(&eval_line("(compiled-p 'h)", &e)));
}
