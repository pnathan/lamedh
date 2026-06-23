// Regression test for issue #61: deep / infinite recursion must produce a
// recoverable error, NOT a native stack overflow that aborts the process.
//
// Runs on a large stack (via with_large_stack) so the depth guard fires before
// the stack is exhausted, exactly as the CLI runs.
//
// Note: With TCO (issue #62), tail-recursive calls no longer consume depth
// frames. The depth guard only fires for genuinely non-tail-recursive code
// (e.g. naive fibonacci where both branches recurse before combining results).

mod test_helpers;
use lamedh::{eval_line, set_eval_depth_limit, with_large_stack};
use test_helpers::env_with_stdlib;

#[test]
fn deep_recursion_returns_error_not_abort() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        // Naive fibonacci is genuinely non-tail-recursive: each call makes two
        // recursive calls that cannot be TCO'd. Deep fib still hits the limit.
        eval_line(
            "(defun fib-deep (n) (if (< n 2) n (+ (fib-deep (- n 1)) (fib-deep (- n 2)))))",
            &env,
        );
        // fib(100000) would require impossibly deep recursion; must error cleanly.
        let out = eval_line("(fib-deep 100000)", &env);
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
        set_eval_depth_limit(50);
        // Naive fibonacci has non-tail recursive calls, so it will still hit the
        // depth limit even with TCO. fib(200) requires ~200 levels of recursion.
        eval_line(
            "(defun fib-cfg (n) (if (< n 2) n (+ (fib-cfg (- n 1)) (fib-cfg (- n 2)))))",
            &env,
        );
        // 200 requires far more than 50 nested non-tail frames -> clean error.
        let out = eval_line("(fib-cfg 200)", &env);
        assert!(out.contains("recursion limit"), "got: {out}");
    });
}
