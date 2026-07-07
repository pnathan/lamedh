//! Integration tests for the restart system (lib/16-conditions.lisp):
//! RESTART-CASE / INVOKE-RESTART / FIND-RESTART / COMPUTE-RESTARTS /
//! HANDLER-BIND and the standard invokers — the recovery half of the
//! condition system, pure Lisp on CATCH/THROW + dynamic variables.

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line, with_large_stack};

fn env() -> Shared<Environment> {
    Environment::with_stdlib()
}

#[test]
fn restart_case_passes_through_normal_values() {
    let e = env();
    assert_eq!(
        eval_line("(restart-case (+ 1 2) (use-value (v) v))", &e),
        "3"
    );
}

#[test]
fn canonical_use_value_recovery() {
    let e = env();
    // The canonical shape: handler chooses the recovery, establisher
    // performs it.
    let out = eval_line(
        "(restart-case
           (handler-bind ((error (lambda (er) (invoke-restart 'use-value 42))))
             (error \"boom\"))
           (use-value (v) v))",
        &e,
    );
    assert_eq!(out, "42");
}

#[test]
fn condition_data_flows_into_the_restart() {
    let e = env();
    let out = eval_line(
        "(restart-case
           (handler-bind ((error (lambda (er) (use-value (error-message er)))))
             (error \"parse failed\"))
           (use-value (v) (list 'recovered v)))",
        &e,
    );
    assert_eq!(out, "(RECOVERED \"parse failed\")");
}

#[test]
fn restart_arguments_are_applied_to_the_clause() {
    let e = env();
    let out = eval_line(
        "(restart-case
           (handler-bind ((error (lambda (er) (invoke-restart 'replace 20 22))))
             (error \"x\"))
           (replace (a b) (+ a b)))",
        &e,
    );
    assert_eq!(out, "42");
}

#[test]
fn retry_restart_reruns_until_success() {
    let e = env();
    eval_line("(setq attempts 0)", &e);
    let out = eval_line(
        "(with-retry-restart
           (setq attempts (+ attempts 1))
           (handler-bind ((error (lambda (er) (if (< attempts 3) (retry) nil))))
             (if (< attempts 3) (error \"flaky\") (list 'ok attempts))))",
        &e,
    );
    assert_eq!(out, "(OK 3)");
}

#[test]
fn inner_restart_shadows_outer() {
    let e = env();
    let out = eval_line(
        "(restart-case
           (restart-case
             (handler-bind ((error (lambda (er) (use-value 'x))))
               (error \"e\"))
             (use-value (v) 'inner))
           (use-value (v) 'outer))",
        &e,
    );
    assert_eq!(out, "INNER");
    // But an outer-only name reaches the outer establisher from inside.
    let out = eval_line(
        "(restart-case
           (restart-case
             (handler-bind ((error (lambda (er) (invoke-restart 'outer-only 7))))
               (error \"e\"))
             (use-value (v) 'inner))
           (outer-only (v) (list 'outer v)))",
        &e,
    );
    assert_eq!(out, "(OUTER 7)");
}

#[test]
fn restarts_expire_with_their_extent() {
    let e = env();
    let out = eval_line(
        "(handler-case
           (progn (restart-case 1 (use-value (v) v))
                  (invoke-restart 'use-value 9))
           (error (er) 'extent-expired))",
        &e,
    );
    assert_eq!(out, "EXTENT-EXPIRED");
    assert_eq!(eval_line("(compute-restarts)", &e), "()");
}

#[test]
fn introspection_lists_live_restarts_innermost_first() {
    let e = env();
    let out = eval_line(
        "(restart-case
           (restart-case
             (mapcar (lambda (r) (restart-name r)) (compute-restarts))
             (retry () nil))
           (use-value (v) v))",
        &e,
    );
    assert_eq!(out, "(RETRY USE-VALUE)");
    let out = eval_line(
        "(restart-case (restart-name (find-restart 'use-value)) (use-value (v) v))",
        &e,
    );
    assert_eq!(out, "USE-VALUE");
}

#[test]
fn declining_handlers_fall_through_outward() {
    let e = env();
    let out = eval_line(
        "(handler-case
           (handler-bind ((error (lambda (er) nil)))   ; declines
             (error \"still propagates\"))
           (error (er) (list 'outer-saw (error-message er))))",
        &e,
    );
    assert_eq!(out, "(OUTER-SAW \"still propagates\")");
}

#[test]
fn unwind_protect_cleanups_run_on_restart_transfer() {
    let e = env();
    eval_line("(setq cleaned nil)", &e);
    let out = eval_line(
        "(restart-case
           (handler-bind ((error (lambda (er) (use-value 'recovered))))
             (unwind-protect (error \"boom\") (setq cleaned t)))
           (use-value (v) v))",
        &e,
    );
    assert_eq!(out, "RECOVERED");
    assert_eq!(eval_line("cleaned", &e), "T");
}

#[test]
fn restarts_compose_with_guard_fences() {
    with_large_stack(|| {
        let e = env();
        // A fuel-exhaustion condition is an ordinary error: a handler can
        // choose a restart for it like any other — bounded work with a
        // programmatic fallback, the whole thesis in one form.
        eval_line(
            "(defun rs-spin (n) (if (< n 1) 'done (rs-spin (- n 1))))",
            &e,
        );
        let out = eval_line(
            "(restart-case
               (handler-bind ((error (lambda (er) (use-value 'budget-fallback))))
                 (with-fuel 50 (rs-spin 100000000)))
               (use-value (v) v))",
            &e,
        );
        assert_eq!(out, "BUDGET-FALLBACK");
    });
}

#[test]
fn invoking_a_missing_restart_errors_cleanly() {
    let e = env();
    let out = eval_line(
        "(handler-case (invoke-restart 'nope) (error (er) (error-message er)))",
        &e,
    );
    assert!(
        out.contains("no live restart"),
        "expected a clear missing-restart error, got: {out}"
    );
}
