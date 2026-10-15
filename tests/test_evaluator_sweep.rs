//! Regression tests from the evaluator / special-form bug sweep.
//!
//! Each section pins one defect against the contract in `KERNEL.md`.

mod test_helpers;
use lamedh::eval_line;
use test_helpers::env_with_stdlib;

// ---------------------------------------------------------------------------
// QUASIQUOTE: `,@` splice semantics (stack-depth regression lives in
// `src/evaluator/quasiquote.rs`, which can reach the private helper)
// ---------------------------------------------------------------------------

#[test]
fn unquote_splicing_keeps_order_tail_and_errors() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("`(1 ,@(list 2 3) 4)", &env), "(1 2 3 4)");
    assert_eq!(eval_line("`(1 ,@(list 2 3) . 4)", &env), "(1 2 3 . 4)");
    assert_eq!(eval_line("`(,@nil)", &env), "()");
    assert_eq!(eval_line("`(,@nil . 5)", &env), "5");
    // Spliced cells are copied; the original list is untouched and unshared.
    assert_eq!(
        eval_line(
            "(let ((l (list 1 2))) (list (eq (cdr `(0 ,@l)) l) l))",
            &env
        ),
        "(() (1 2))"
    );
    // Non-list and improper splices are errors.
    assert!(eval_line("`(1 ,@5)", &env).contains("expected a list argument"));
    assert!(eval_line("`(1 ,@'(2 . 3))", &env).contains("expected a list argument"));
}
