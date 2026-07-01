//! Tests for the M2 increments of the compile→execute IR (issue #200):
//! compiled `SETQ`, `UNWIND-PROTECT`, `WHILE`, `FOR`, `&rest` tail calls, and
//! a tail call from compiled code into a macro.
//!
//! These forms are only compiled when they appear in a lambda body (compiled
//! at `defun`/`lambda` creation time), so every test here calls through a
//! `defun` rather than evaluating the form at top level.

mod test_helpers;
use lamedh::{eval_line, with_large_stack};
use test_helpers::env_with_stdlib;

#[test]
fn compiled_setq_and_while_sum_to_n() {
    let env = env_with_stdlib();
    eval_line(
        "(defun sum-to (n) (let ((acc 0) (i 0)) (while (<= i n) (setq acc (+ acc i)) (setq i (+ i 1))) acc))",
        &env,
    );
    assert_eq!(eval_line("(sum-to 100)", &env), "5050");
}

#[test]
fn compiled_for_sums_inclusive_range() {
    let env = env_with_stdlib();
    eval_line(
        "(defun for-sum (n) (let ((acc 0)) (for (i 1 n) (setq acc (+ acc i))) acc))",
        &env,
    );
    assert_eq!(eval_line("(for-sum 5)", &env), "15");
}

#[test]
fn compiled_for_respects_step_and_direction() {
    let env = env_with_stdlib();
    eval_line(
        "(defun countdown-sum (n) (let ((acc 0)) (for (i n 0 -1) (setq acc (+ acc i))) acc))",
        &env,
    );
    // 5+4+3+2+1+0 = 15
    assert_eq!(eval_line("(countdown-sum 5)", &env), "15");
}

#[test]
fn compiled_unwind_protect_runs_cleanup_on_error() {
    let env = env_with_stdlib();
    eval_line("(defvar *log* nil)", &env);
    eval_line(
        "(defun risky () (unwind-protect (error \"boom\") (setq *log* (cons 'cleaned *log*))))",
        &env,
    );
    // errorset traps the ordinary error; the cleanup must still have run.
    eval_line("(errorset '(risky))", &env);
    assert_eq!(eval_line("*log*", &env), "(CLEANED)");
}

#[test]
fn compiled_unwind_protect_runs_cleanup_on_success() {
    let env = env_with_stdlib();
    eval_line("(defvar *log2* nil)", &env);
    eval_line(
        "(defun fine () (unwind-protect 42 (setq *log2* (cons 'cleaned *log2*))))",
        &env,
    );
    assert_eq!(eval_line("(fine)", &env), "42");
    assert_eq!(eval_line("*log2*", &env), "(CLEANED)");
}

#[test]
fn compiled_unwind_protect_runs_cleanup_on_nonlocal_exit() {
    let env = env_with_stdlib();
    eval_line("(defvar *log3* nil)", &env);
    // The `catch` lives at the call site (top level), not inside the defun,
    // so the defun's own compiled body is exactly the `unwind-protect` —
    // exercising `Code::UnwindProtect` against a THROW rather than an error.
    eval_line(
        "(defun risky-throw () (unwind-protect (throw 'tag 'thrown) (setq *log3* (cons 'cleaned *log3*))))",
        &env,
    );
    assert_eq!(eval_line("(catch 'tag (risky-throw))", &env), "THROWN");
    assert_eq!(eval_line("*log3*", &env), "(CLEANED)");
}

#[test]
fn compiled_rest_arity_underflow_errors_cleanly() {
    let env = env_with_stdlib();
    eval_line("(defun needs-two (a b &rest more) (list a b more))", &env);
    // Too few args for the compiled `&rest` inline-TCO path must fall through
    // to `apply()`'s ordinary arity error, not panic on the arg-drain.
    assert_eq!(eval_line("(errorset '(needs-two 1))", &env), "()");
}

#[test]
fn compiled_for_rejects_zero_step() {
    let env = env_with_stdlib();
    eval_line("(defun bad-step () (for (i 1 5 0) i))", &env);
    assert_eq!(eval_line("(errorset '(bad-step))", &env), "()");
}

#[test]
fn compiled_for_rejects_non_integer_bound() {
    let env = env_with_stdlib();
    eval_line("(defun bad-bound () (for (i 1.5 5) i))", &env);
    assert_eq!(eval_line("(errorset '(bad-bound))", &env), "()");
}

#[test]
fn compiled_setq_odd_arity_errors() {
    // Odd argument count falls back to `Code::Interp` at compile time; the
    // tree-walker must still report the error (not panic or silently ignore
    // the dangling variable).
    let env = env_with_stdlib();
    eval_line("(defun bad-setq () (setq x))", &env);
    assert_eq!(eval_line("(errorset '(bad-setq))", &env), "()");
}

/// `SETQ` on a name unbound anywhere in the chain creates it in the *current*
/// environment (matching the tree-walker) — here that's the call's own local
/// frame, so the binding is visible within the call (return value 99) but
/// does not leak to the global environment once the call returns. This is
/// ordinary Lisp scoping, not specific to compilation; verified identically
/// on the tree-walker path via `cargo run -- -s`.
#[test]
fn compiled_setq_creates_variable_in_current_frame() {
    let env = env_with_stdlib();
    eval_line(
        "(defun make-fresh-var () (setq m2-fresh-test-var 99))",
        &env,
    );
    assert_eq!(eval_line("(make-fresh-var)", &env), "99");
    assert_eq!(eval_line("(errorset 'm2-fresh-test-var)", &env), "()");
}

/// A tail-recursive `&rest` function must run in O(1) native stack — before
/// the M2 `&rest` inline-TCO fix, a compiled variadic lambda always fell
/// through to `apply()` (a plain, non-tail Rust call) even in tail position.
#[test]
fn compiled_rest_param_tail_call_is_tco() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        eval_line(
            "(defun count-down (n &rest ignored) (if (= n 0) (quote done) (count-down (- n 1))))",
            &env,
        );
        let result = eval_line("(count-down 1000000)", &env);
        assert_eq!(
            result, "DONE",
            "expected DONE from a TCO'd &rest tail call, got: {result}"
        );
    });
}

/// A tail call from compiled code into a macro must hand off to the
/// tree-walker on the *same* trampoline rather than recursing — before this
/// M2 increment the compiled `Call` site always ran `eval(original, env)` as
/// a plain nested call for any macro/fexpr/vau callee, which is not TCO'd.
#[test]
fn compiled_tail_call_through_macro_is_tco() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        eval_line("(defmacro identity-form (x) x)", &env);
        eval_line(
            "(defun loopy2 (n) (if (= n 0) (quote done) (identity-form (loopy2 (- n 1)))))",
            &env,
        );
        let result = eval_line("(loopy2 1000000)", &env);
        assert_eq!(
            result, "DONE",
            "expected DONE from a TCO'd tail call through a macro, got: {result}"
        );
    });
}
