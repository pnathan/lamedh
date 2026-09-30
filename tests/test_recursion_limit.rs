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
        // The eval depth limit is the INTERPRETER's guard; since #512 naive fib
        // compiles natively (bounded by the typed call cap instead, see
        // `native_deep_recursion_*` below), so pin it to the interpreter.
        eval_line(
            "(defun fib-cfg (n) (declare (no-compile)) \
               (if (< n 2) n (+ (fib-cfg (- n 1)) (fib-cfg (- n 2)))))",
            &env,
        );
        // 200 requires far more than 50 nested non-tail frames -> clean error.
        let out = eval_line("(fib-cfg 200)", &env);
        assert!(out.contains("recursion limit"), "got: {out}");
    });
}

#[test]
fn limit_message_names_a_user_reachable_knob() {
    // Issue #520: the hint used to name the Rust-only set_eval_depth_limit.
    with_large_stack(|| {
        let env = env_with_stdlib();
        // Plain non-tail recursion deeper than the default limit. (This used
        // to be a long DOLIST, which #504 made constant-stack.)
        eval_line(
            "(defun knob-deep (n) (declare (no-compile)) (if (= n 0) 0 (+ 1 (knob-deep (- n 1)))))",
            &env,
        );
        let out = eval_line("(knob-deep 20000)", &env);
        assert!(
            out.starts_with(
                "Error: recursion limit exceeded (10000 eval frames); \
                 rewrite iteratively or raise it with `lamedh --max-depth N`"
            ),
            "got: {out}"
        );
        assert!(!out.contains("set_eval_depth_limit"), "got: {out}");
        // The runaway frames collapse into one counted entry.
        assert!(out.contains("\n  in: KNOB-DEEP (\u{d7}"), "got: {out}");
        assert!(!out.contains("KNOB-DEEP \u{2190} KNOB-DEEP"), "got: {out}");
    });
}

#[test]
fn lisp_can_lower_and_restore_the_limit() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        assert_eq!(eval_line("(eval-depth-limit)", &env), "10000");
        // Returns the previous limit.
        assert_eq!(eval_line("(set-eval-depth-limit! 50)", &env), "10000");
        assert_eq!(eval_line("(eval-depth-limit)", &env), "50");
        eval_line(
            "(defun nt (n) (if (= n 0) (error \"bottom\") (+ 1 (nt (- n 1)))))",
            &env,
        );
        let out = eval_line("(nt 200)", &env);
        assert!(
            out.contains("recursion limit exceeded (50 eval frames)"),
            "got: {out}"
        );
        // Back up to (but not past) the host's ceiling.
        assert_eq!(eval_line("(set-eval-depth-limit! 10000)", &env), "50");
        assert_eq!(
            eval_line("(nt 200)", &env),
            "Error: bottom\n  in: NT (\u{d7}201)"
        );
    });
}

#[test]
fn lisp_cannot_raise_the_limit_past_the_host_ceiling() {
    // Only the host knows the native stack size; a (possibly sandboxed)
    // program raising the limit could abort the process on stack overflow.
    with_large_stack(|| {
        let env = env_with_stdlib();
        let out = eval_line("(set-eval-depth-limit! 10001)", &env);
        assert!(
            out.contains("limit must be between 1 and 10000"),
            "got: {out}"
        );
        assert_eq!(eval_line("(eval-depth-limit)", &env), "10000");
        for bad in ["0", "-3", "'x", "1.5"] {
            let out = eval_line(&format!("(set-eval-depth-limit! {bad})"), &env);
            assert!(
                out.starts_with("Error: SET-EVAL-DEPTH-LIMIT!"),
                "{bad}: {out}"
            );
        }
        // The host raising the limit raises the ceiling with it.
        set_eval_depth_limit(20_000);
        assert_eq!(eval_line("(set-eval-depth-limit! 100)", &env), "20000");
        assert_eq!(eval_line("(set-eval-depth-limit! 20000)", &env), "100");
    });
}

// Since #512 an un-annotated naive fib compiles natively. Past the typed call
// cap the native edition must stop promptly: before, frames below the cap kept
// calling after the error was pending (exponential work, OOM-killed).

#[test]
fn native_deep_recursion_errors_promptly_for_defun_typed() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        eval_line(
            "(defun-typed (fib-typed int64) ((n int64)) \
               (if (< n 2) n (+ (fib-typed (- n 1)) (fib-typed (- n 2)))))",
            &env,
        );
        let out = eval_line("(fib-typed 100000)", &env);
        assert!(out.contains("recursion limit"), "got: {out}");
        // The function still works afterwards.
        assert_eq!(eval_line("(fib-typed 20)", &env), "6765");
    });
}

#[test]
fn native_deep_recursion_errors_promptly_for_auto_compiled_defun() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        eval_line(
            "(defun fib-auto (n) (if (< n 2) n (+ (fib-auto (- n 1)) (fib-auto (- n 2)))))",
            &env,
        );
        assert_eq!(
            eval_line("(explain-compile 'fib-auto)", &env),
            "((TIER . COMPILED) (SIGNATURE -> (INT64) INT64))"
        );
        let out = eval_line("(fib-auto 100000)", &env);
        assert!(out.contains("recursion limit"), "got: {out}");
        // The failed-native fallback does not leave native disabled.
        assert_eq!(eval_line("(fib-auto 20)", &env), "6765");
    });
}
