// Tests for Tail-Call Optimization (TCO) — issue #62.
//
// With proper TCO, tail-recursive functions must handle arbitrarily large
// inputs without hitting the recursion depth limit. The key acceptance test
// is (loopy 1000000) returning DONE.

mod test_helpers;
use lamedh::{eval_line, with_large_stack};
use test_helpers::env_with_stdlib;

/// The primary acceptance test: one million tail-recursive calls must succeed.
#[test]
fn test_tco_self_recursion_million() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        eval_line(
            "(defun loopy (n) (if (= n 0) (quote done) (loopy (- n 1))))",
            &env,
        );
        let result = eval_line("(loopy 1000000)", &env);
        assert_eq!(result, "DONE", "expected DONE from (loopy 1000000), got: {result}");
    });
}

/// IF tail branches must be TCO'd — both the true and false branches.
#[test]
fn test_tco_if_tail() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        // true branch
        eval_line(
            "(defun count-down (n) (if (= n 0) (quote done) (count-down (- n 1))))",
            &env,
        );
        assert_eq!(eval_line("(count-down 500000)", &env), "DONE");

        // false branch (starts false, flips to true at 0)
        eval_line(
            "(defun count-up (n max) (if (= n max) (quote reached) (count-up (+ n 1) max)))",
            &env,
        );
        assert_eq!(eval_line("(count-up 0 500000)", &env), "REACHED");
    });
}

/// COND tail clause must be TCO'd.
#[test]
fn test_tco_cond_tail() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        eval_line(
            "(defun cond-loop (n)
               (cond
                 ((= n 0) (quote done))
                 (t (cond-loop (- n 1)))))",
            &env,
        );
        assert_eq!(eval_line("(cond-loop 500000)", &env), "DONE");
    });
}

/// PROGN's last form is a tail call.
#[test]
fn test_tco_progn_tail() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        eval_line(
            "(defun progn-loop (n)
               (progn
                 (if (= n 0) (quote done) (progn-loop (- n 1)))))",
            &env,
        );
        assert_eq!(eval_line("(progn-loop 500000)", &env), "DONE");
    });
}

/// LET body is a tail call.
#[test]
fn test_tco_let_tail() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        eval_line(
            "(defun let-loop (n)
               (let ((m (- n 1)))
                 (if (= n 0) (quote done) (let-loop m))))",
            &env,
        );
        assert_eq!(eval_line("(let-loop 500000)", &env), "DONE");
    });
}

/// Two mutually-tail-calling functions (ping-pong) are effectively TCO'd
/// because each individual call is in tail position and each `defun` body
/// is a lambda tail call.
#[test]
fn test_tco_even_odd() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        eval_line("(defun my-even (n) (if (= n 0) t (my-odd (- n 1))))", &env);
        eval_line("(defun my-odd (n) (if (= n 0) nil (my-even (- n 1))))", &env);
        // 200000 calls alternating between my-even and my-odd
        assert_eq!(eval_line("(my-even 200000)", &env), "T");
        assert_eq!(eval_line("(my-odd 200001)", &env), "T");
    });
}

/// Non-tail-recursive code (naive fibonacci) should still get the depth-limit
/// error rather than a stack overflow, confirming the guard still works for
/// genuine non-tail recursion.
#[test]
fn test_non_tail_recursion_still_guarded() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        eval_line(
            "(defun fib (n) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2)))))",
            &env,
        );
        // fib(100000) would need exponentially deep call stacks; must error cleanly.
        let out = eval_line("(fib 100000)", &env);
        assert!(
            out.contains("recursion limit"),
            "expected recursion limit error for non-tail fib, got: {out}"
        );
    });
}
