//! Tests for the COLLAPSE-FRAMES optimizer pass (issue #128).
//!
//! The pass implements three rewrites:
//!   1. Identity let elimination: (let ((x e)) x) -> e
//!   2. Single-use open-position inline: (let ((x e)) body) -> body[x<-e]
//!      when x has exactly one reference that is not inside a closure body.
//!   3. Frame merge: (let outer (let inner body)) -> (let merged body)
//!      when binding names are disjoint and no inner init uses an outer var.

mod test_helpers;
use lamedh::eval_line;
use test_helpers::env_with_stdlib;

// ─── Rule 1: Identity let elimination ────────────────────────────────────────

/// (let ((x 42)) x) collapses to 42.
#[test]
fn collapse_identity_let_literal() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(optimize-form '(let ((x 42)) x))", &env), "42");
}

/// (let ((x (+ 1 2))) x) collapses to (+ 1 2), then the builtin folder folds
/// to 3.
#[test]
fn collapse_identity_let_expression() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("(optimize-form '(let ((x (+ 1 2))) x))", &env),
        "3"
    );
}

/// (let ((x (if t 7 0))) x) — non-pure init, body is just x; always safe.
#[test]
fn collapse_identity_let_non_pure_init() {
    let env = env_with_stdlib();
    // After collapse: (if t 7 0), then constant-folder simplifies to 7.
    assert_eq!(
        eval_line("(optimize-form '(let ((x (if t 7 0))) x))", &env),
        "7"
    );
}

// ─── Rule 2: Single-use open-position inline ─────────────────────────────────

/// A single-use binding with an atom init is already handled by opt-pass,
/// but confirm collapse-frames doesn't break it.
#[test]
fn collapse_single_use_atom_stays_inlined() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("(optimize-form '(let ((x 7)) (* x 2)))", &env),
        "14"
    );
}

/// (let ((x (+ a b))) (* x 2)) — non-atom pure init, single open use.
/// The collapse pass inlines x -> (+ a b) so the frame is eliminated.
#[test]
fn collapse_single_use_pure_non_atom_init() {
    let env = env_with_stdlib();
    // We verify via $opt (which evaluates with real values).
    assert_eq!(
        eval_line(
            "(let ((a 3) (b 4)) ($opt (let ((x (+ a b))) (* x 2))))",
            &env
        ),
        "14"
    );
}

/// (let ((x e)) (f x y)) with x used once — inline.
#[test]
fn collapse_single_use_application_arg() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("($opt (let ((n (+ 2 3))) (+ n 1)))", &env), "6");
}

/// Multiple uses: do NOT inline (that would duplicate evaluation).
#[test]
fn collapse_no_inline_multi_use() {
    let env = env_with_stdlib();
    // x used twice — must keep the binding.
    // Value should still be correct.
    assert_eq!(eval_line("($opt (let ((x (+ 1 2))) (+ x x)))", &env), "6");
}

/// Mutated variable: do NOT inline even when count is 1.
#[test]
fn collapse_no_inline_mutated_var() {
    let env = env_with_stdlib();
    // x is setq'd inside the body — must keep the binding.
    assert_eq!(
        eval_line("($opt (let ((x 10)) (progn (setq x (+ x 5)) x)))", &env),
        "15"
    );
}

/// A closure-captured variable must NOT be inlined (rule 2's open-ref guard).
/// The init is a side-effectful form; if inlined into the lambda body it would
/// re-execute on every call instead of once at bind time.
#[test]
fn collapse_no_inline_into_closure() {
    let env = env_with_stdlib();
    // counter starts at 0.  The let init increments it; the lambda just
    // returns x.  Without inlining: counter = 1 after both calls.
    // With incorrect inlining into the lambda: counter = 2 (re-evaluated).
    assert_eq!(
        eval_line(
            "(progn
               (def *cf-counter* 0)
               (let ((f (let ((x (progn
                                   (setq *cf-counter* (+ *cf-counter* 1))
                                   42)))
                          (lambda () x))))
                 (list (funcall f) (funcall f) *cf-counter*)))",
            &env
        ),
        "(42 42 1)"
    );
}

// ─── Rule 3: Nested let frame merge ──────────────────────────────────────────

/// Independent nested lets with disjoint names merge into one frame.
#[test]
fn collapse_merges_independent_lets() {
    let env = env_with_stdlib();
    // (let ((a 1) (b 2)) (let ((c 3) (d 4)) (+ a b c d))) → 10
    assert_eq!(
        eval_line(
            "($opt (let ((a 1) (b 2)) (let ((c 3) (d 4)) (+ a b c d))))",
            &env
        ),
        "10"
    );
}

/// Name conflict: inner binding shadows outer variable — must keep nested.
#[test]
fn collapse_bails_on_name_conflict() {
    let env = env_with_stdlib();
    // (let ((x 1)) (let ((x 2)) x)) — inner x shadows outer.
    // Merging would be wrong; correct value is 2 (inner x wins).
    assert_eq!(eval_line("($opt (let ((x 1)) (let ((x 2)) x)))", &env), "2");
}

/// Inner init depends on an outer-bound variable: must keep nested.
/// Merging would evaluate the inner init in the pre-outer scope where the
/// outer variable is not yet bound.
#[test]
fn collapse_bails_when_inner_init_uses_outer_var() {
    let env = env_with_stdlib();
    // (let ((a 5)) (let ((b (+ a 1))) (+ a b)))
    // inner init (+ a 1) uses outer var a — cannot merge.
    // Correct value: a=5, b=6, result=11.
    assert_eq!(
        eval_line("($opt (let ((a 5)) (let ((b (+ a 1))) (+ a b))))", &env),
        "11"
    );
}

// ─── End-to-end correctness ───────────────────────────────────────────────────

/// Collapsed form still evaluates to the expected value.
#[test]
fn collapse_frames_end_to_end_identity() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("($opt (let ((x (+ 3 4))) x))", &env), "7");
}

/// Multi-level nested lets all collapse correctly.
#[test]
fn collapse_frames_multi_level_nesting() {
    let env = env_with_stdlib();
    // Three independent lets — the outer two can merge; the innermost
    // may further merge or not depending on dependency analysis.
    assert_eq!(
        eval_line(
            "($opt (let ((a 1))
                     (let ((b 2))
                       (let ((c 3))
                         (+ a b c)))))",
            &env
        ),
        "6"
    );
}

/// optimize-form on a quoted form returns the structurally optimized form.
/// A plain identity let should collapse all the way to the literal.
#[test]
fn collapse_optimize_form_returns_collapsed_quoted() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("(optimize-form '(let ((result (* 6 7))) result))", &env),
        "42"
    );
}
