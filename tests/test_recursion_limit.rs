// Regression test for issue #61: deep / infinite recursion must produce a
// recoverable error, NOT a native stack overflow that aborts the process.
//
// Runs on a large stack (via with_large_stack) so the depth guard fires before
// the stack is exhausted, exactly as the CLI runs.

mod test_helpers;
use lamedh::{eval_line, set_eval_depth_limit, with_large_stack};
use test_helpers::env_with_stdlib;

#[test]
fn deep_recursion_returns_error_not_abort() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        eval_line(
            "(defun loopy (n) (if (= n 0) (quote done) (loopy (- n 1))))",
            &env,
        );
        // Far beyond any reasonable limit: must come back as an error string,
        // and crucially the process must still be alive to make this assertion.
        let out = eval_line("(loopy 100000000)", &env);
        assert!(
            out.contains("recursion limit"),
            "expected a recursion-limit error, got: {out}"
        );
    });
}

#[test]
fn shallow_recursion_still_works() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        eval_line(
            "(defun countdown (n) (if (= n 0) (quote done) (countdown (- n 1))))",
            &env,
        );
        assert_eq!(eval_line("(countdown 500)", &env), "DONE");
    });
}

#[test]
fn depth_limit_is_configurable() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        set_eval_depth_limit(200);
        eval_line(
            "(defun countdown (n) (if (= n 0) (quote done) (countdown (- n 1))))",
            &env,
        );
        // 5000 nested calls exceed a 200-frame limit -> clean error.
        let out = eval_line("(countdown 5000)", &env);
        assert!(out.contains("recursion limit"), "got: {out}");
    });
}
