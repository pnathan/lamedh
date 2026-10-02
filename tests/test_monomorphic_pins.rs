//! Issue #512: an un-annotated function whose checker scheme is monomorphic
//! and compileable is compiled under that scheme as PINS. Codegen resolves
//! operand types eagerly, so without pins `fib`'s `(+ (fib ..) (fib ..))` saw
//! the recursive call's still-unsolved return type and reported "`+`: cannot
//! infer operand type" although the checker had inferred `(-> (INT64) INT64)`.
//! The portable gate (`lib/46-hm-check.lisp`) seeds the same pins, so the two
//! hosts keep agreeing. Also: checking-only builtins (`CAR`, `NULL`,
//! `VARIANT-CASE`, ...) are reported as unsupported in compiled code, not as
//! unknown functions.

mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

const FIB_BODY: &str = "(if (< n 2) n (+ (fib (- n 1)) (fib (- n 2))))";

#[test]
fn defun_star_fib_compiles_under_its_inferred_scheme() {
    let e = env_with_stdlib();
    eval_line(&format!("(defun* fib (n) {FIB_BODY})"), &e);
    assert_eq!(
        eval_line("(explain-compile 'fib)", &e),
        "((TIER . COMPILED) (SIGNATURE -> (INT64) INT64))"
    );
    assert_eq!(eval_line("(fib 25)", &e), "75025");
    assert_eq!(eval_line("(why-not-typed 'fib)", &e), "()");
}

#[test]
fn plain_defun_fib_compiles_too() {
    let e = env_with_stdlib();
    eval_line(&format!("(defun fib (n) {FIB_BODY})"), &e);
    assert_eq!(
        eval_line("(explain-compile 'fib)", &e),
        "((TIER . COMPILED) (SIGNATURE -> (INT64) INT64))"
    );
    assert_eq!(eval_line("(fib 20)", &e), "6765");
}

#[test]
fn partially_annotated_defun_star_fills_its_holes_from_the_scheme() {
    let e = env_with_stdlib();
    eval_line(
        "(defun* fibh ((n int64)) \
           (if (< n 2) n (+ (fibh (- n 1)) (fibh (- n 2)))))",
        &e,
    );
    assert_eq!(
        eval_line("(explain-compile 'fibh)", &e),
        "((TIER . COMPILED) (SIGNATURE -> (INT64) INT64))"
    );
    assert_eq!(eval_line("(fibh 10)", &e), "55");
}

#[test]
fn explain_compile_dry_run_pins_like_the_install_path() {
    let e = env_with_stdlib();
    // Bound by DEF, so nothing auto-compiled it: explain must reach the same
    // verdict the install path would, and install nothing.
    eval_line(&format!("(def fib (lambda (n) {FIB_BODY}))"), &e);
    let out = eval_line("(explain-compile 'fib)", &e);
    assert!(out.contains("(SCHEME -> (INT64) INT64)"), "got: {out}");
    assert!(out.contains("natively compileable"), "got: {out}");
    assert_eq!(eval_line("(explain-compile 'fib)", &e), out);
    assert_eq!(eval_line("(fib 10)", &e), "55");
}

#[test]
fn polymorphic_schemes_are_not_pinned() {
    let e = env_with_stdlib();
    eval_line("(defun sq (x) (* x x))", &e);
    let out = eval_line("(explain-compile 'sq)", &e);
    assert!(out.contains("(TIER . CHECKED)"), "got: {out}");
    assert!(out.contains("`*`: cannot infer operand type"), "got: {out}");
}

#[test]
fn portable_gate_agrees_on_fib() {
    let e = env_with_stdlib();
    eval_line(&format!("(def fib (lambda (n) {FIB_BODY}))"), &e);
    assert_eq!(
        eval_line("(hm-compile-verdict 'fib)", &e),
        "(COMPILEABLE (-> (INT64) INT64))"
    );
    assert_eq!(
        eval_line("(island-member-names (typed-island '(fib)))", &e),
        "(FIB)"
    );
    assert_eq!(
        eval_line("(island-signature (typed-island '(fib)) 'fib)", &e),
        "(-> (INT64) INT64)"
    );
}

#[test]
fn checking_only_builtins_are_named_as_unsupported_not_unknown() {
    let e = env_with_stdlib();
    eval_line("(defun* hd (xs) (car xs))", &e);
    assert_eq!(
        eval_line("(why-not-typed 'hd)", &e),
        "\"builtin `CAR` is not supported in compiled code\""
    );
    eval_line("(defun* nonempty (xs) (if (null xs) 0 1))", &e);
    assert_eq!(
        eval_line("(why-not-typed 'nonempty)", &e),
        "\"builtin `NULL` is not supported in compiled code\""
    );
    eval_line("(defvariant Sh512 (Circ512 r) (Sq512 s))", &e);
    eval_line(
        "(defun* area512 (x) (variant-case x ((Circ512 r) r) ((Sq512 s) s)))",
        &e,
    );
    assert_eq!(
        eval_line("(why-not-typed 'area512)", &e),
        "\"builtin `VARIANT-CASE` is not supported in compiled code\""
    );
    // The portable gate uses the same words.
    assert_eq!(
        eval_line("(cadr (hm-compile-verdict 'nonempty))", &e),
        "\"builtin `NULL` is not supported in compiled code\""
    );
    // A genuinely unknown callee keeps its wording.
    eval_line("(defun* usenope (x) (nope512 x))", &e);
    assert_eq!(
        eval_line("(why-not-typed 'usenope)", &e),
        "\"call to unknown function `NOPE512`\""
    );
}
