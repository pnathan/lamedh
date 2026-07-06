//! Integration tests for lib/22-guard.lisp (issue #284): WITH-FUEL and
//! WITH-CAPABILITIES — composable dynamic-extent attenuation of execution
//! budget and capability authority, pure Lisp ("Phase 1").

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line, with_large_stack};

fn env() -> Shared<Environment> {
    Environment::with_stdlib()
}

// ---------------------------------------------------------------- fuel ----

#[test]
fn fuel_exhaustion_is_a_catchable_error_and_env_survives() {
    with_large_stack(|| {
        let e = env();
        let out = eval_line(
            "(handler-case
               (with-fuel 50
                 (defun guard-spin (n) (if (< n 1) 'done (guard-spin (- n 1))))
                 (guard-spin 1000000))
               (error (er) (error-message er)))",
            &e,
        );
        assert!(
            out.contains("fuel exhausted"),
            "expected a fuel-exhausted error, got: {out}"
        );
        // The interpreter is healthy afterwards.
        assert_eq!(eval_line("(+ 1 2)", &e), "3");
    });
}

#[test]
fn completed_fenced_work_matches_bare_result() {
    with_large_stack(|| {
        let e = env();
        let fenced = eval_line(
            "(with-fuel 100000 (mapcar (lambda (v) (* v v)) (list 1 2 3 4)))",
            &e,
        );
        let bare = eval_line("(mapcar (lambda (v) (* v v)) (list 1 2 3 4))", &e);
        assert_eq!(fenced, bare);
        assert_eq!(fenced, "(1 4 9 16)");
    });
}

#[test]
fn tco_preserved_under_instrumentation() {
    with_large_stack(|| {
        let e = env();
        // A million-iteration tail loop must complete inside a large budget —
        // the inserted ticks must not break tail position.
        let out = eval_line(
            "(with-fuel 5000000
               (defun guard-count (n acc) (if (< n 1) acc (guard-count (- n 1) (+ acc 1))))
               (guard-count 1000000 0))",
            &e,
        );
        assert_eq!(out, "1000000");
    });
}

#[test]
fn while_prog_dotimes_backedges_are_metered() {
    with_large_stack(|| {
        let e = env();
        for (name, form) in [
            ("while", "(with-fuel 40 (while t nil))"),
            ("prog/go", "(with-fuel 40 (prog () lp (go lp)))"),
            ("dotimes", "(with-fuel 40 (dotimes (i 1000000) nil))"),
        ] {
            let out = eval_line(&format!("(handler-case {form} (error (er) 'caught))"), &e);
            assert_eq!(out, "CAUGHT", "{name} loop must exhaust, got: {out}");
        }
    });
}

#[test]
fn fuel_remaining_introspection() {
    with_large_stack(|| {
        let e = env();
        // Outside any fence: NIL.
        assert_eq!(eval_line("(fuel-remaining)", &e), "()");
        // Straight-line code charges nothing at the top level.
        assert_eq!(eval_line("(with-fuel 100 (fuel-remaining))", &e), "100");
    });
}

#[test]
fn nested_fuel_clamps_and_charges_the_outer_fence() {
    with_large_stack(|| {
        let e = env();
        // Inner request larger than the outer remainder is clamped.
        assert_eq!(
            eval_line("(with-fuel 100 (with-fuel 1000 (fuel-remaining)))", &e),
            "100"
        );
        // Work inside the inner fence depletes the outer fence too.
        let out = eval_line(
            "(with-fuel 1000
               (defun guard-n (n) (if (< n 1) 'd (guard-n (- n 1))))
               (with-fuel 500 (guard-n 100))
               (fuel-remaining))",
            &e,
        );
        let remaining: i64 = out.parse().expect("fuel-remaining should be a number");
        assert!(
            remaining < 900,
            "inner work must charge the outer fence, got remaining {remaining}"
        );
    });
}

#[test]
fn eval_escape_hatch_is_metered() {
    with_large_stack(|| {
        let e = env();
        let out = eval_line(
            "(handler-case
               (with-fuel 30 (eval (quote (prog () lp (go lp)))))
               (error (er) 'caught))",
            &e,
        );
        assert_eq!(out, "CAUGHT");
    });
}

#[test]
fn rebinding_the_chaining_name_does_not_evade_charging() {
    with_large_stack(|| {
        let e = env();
        let out = eval_line(
            "(handler-case
               (with-fuel 50
                 (setq $guard-tick (lambda () nil))
                 (defun guard-e (n) (if (< n 1) 'evaded (guard-e (- n 1))))
                 (guard-e 1000000))
               (error (er) 'still-charged))",
            &e,
        );
        assert_eq!(out, "STILL-CHARGED");
    });
}

#[test]
fn escaped_closures_run_against_a_retired_budget() {
    with_large_stack(|| {
        let e = env();
        // A closure created under a fence and called after the fence exits
        // must not raise stale fuel errors.
        eval_line(
            "(setq guard-escapee (with-fuel 10000 (lambda (v) (* v 2))))",
            &e,
        );
        assert_eq!(eval_line("(funcall guard-escapee 21)", &e), "42");
    });
}

#[test]
fn no_compile_inside_fuel_fence() {
    with_large_stack(|| {
        let e = env();
        assert_eq!(
            eval_line(
                "(with-fuel 1000 (defun guard-nc (x) (* x 2)) (jit-optimize guard-nc))",
                &e
            ),
            "COMPILE-DISABLED-BY-GUARD"
        );
        let out = eval_line(
            "(handler-case
               (with-fuel 1000 (defun-typed (guard-t int64) ((x int64)) x))
               (error (er) 'blocked))",
            &e,
        );
        assert_eq!(out, "BLOCKED");
    });
}

// -------------------------------------------------------- capabilities ----

#[test]
fn capabilities_attenuate_and_deny_with_a_catchable_error() {
    with_large_stack(|| {
        let e = env();
        e.enable_feature("READ-FS");
        e.enable_feature("CREATE-FS");
        assert_eq!(
            eval_line(
                "(with-capabilities '(READ-FS) (capabilities-effective))",
                &e
            ),
            "(READ-FS)"
        );
        let out = eval_line(
            "(with-capabilities '(READ-FS)
               (handler-case (write-file \"/tmp/guard-x\" \"hi\")
                 (error (er) (error-message er))))",
            &e,
        );
        assert!(
            out.contains("capability denied") && out.contains("CREATE-FS"),
            "expected a capability-denied error naming CREATE-FS, got: {out}"
        );
    });
}

#[test]
fn capabilities_cannot_exceed_host_grants() {
    with_large_stack(|| {
        let e = env(); // nothing granted
        assert_eq!(
            eval_line(
                "(with-capabilities '(READ-FS CREATE-FS SHELL) (capabilities-effective))",
                &e
            ),
            "()"
        );
    });
}

#[test]
fn nested_capabilities_intersect_and_narrowing_is_permanent() {
    with_large_stack(|| {
        let e = env();
        e.enable_feature("READ-FS");
        e.enable_feature("SHELL");
        assert_eq!(
            eval_line(
                "(with-capabilities '(READ-FS SHELL)
                   (with-capabilities '(SHELL CREATE-FS) (capabilities-effective)))",
                &e
            ),
            "(SHELL)"
        );
        // Re-requesting a dropped capability inside does not restore it.
        assert_eq!(
            eval_line(
                "(with-capabilities '(SHELL)
                   (with-capabilities '(READ-FS SHELL) (capabilities-effective)))",
                &e
            ),
            "(SHELL)"
        );
    });
}

#[test]
fn allowed_operations_pass_through_the_fence() {
    with_large_stack(|| {
        let e = env();
        e.enable_feature("READ-FS");
        let out = eval_line(
            "(with-capabilities '(READ-FS) (file-exists-p \"/etc/hostname\"))",
            &e,
        );
        assert!(
            out == "T" || out == "()",
            "expected a normal boolean answer, got: {out}"
        );
    });
}

// --------------------------------------------------------- composition ----

#[test]
fn both_nesting_orders_behave_identically() {
    with_large_stack(|| {
        let e = env();
        e.enable_feature("READ-FS");
        let a = eval_line(
            "(with-fuel 1000 (with-capabilities '()
               (handler-case (read-file \"/etc/hostname\") (error (er) 'denied))))",
            &e,
        );
        let b = eval_line(
            "(with-capabilities '() (with-fuel 1000
               (handler-case (read-file \"/etc/hostname\") (error (er) 'denied))))",
            &e,
        );
        assert_eq!(a, "DENIED");
        assert_eq!(b, "DENIED");
    });
}

#[test]
fn sandboxed_combinator_composes_both_fences() {
    with_large_stack(|| {
        let e = env();
        e.enable_feature("READ-FS");
        assert_eq!(
            eval_line(
                "(sandboxed (:fuel 1000 :capabilities (READ-FS))
                   (list (fuel-remaining) (capabilities-effective)))",
                &e
            ),
            "(1000 (READ-FS))"
        );
        let out = eval_line(
            "(handler-case
               (sandboxed (:fuel 40 :capabilities ()) (while t nil))
               (error (er) 'caught))",
            &e,
        );
        assert_eq!(out, "CAUGHT");
    });
}

#[test]
fn self_escalation_is_impossible_from_guarded_code() {
    with_large_stack(|| {
        let e = env();
        // enable-feature is not exposed to Lisp at all; the sandbox contract
        // (test_sandboxing.rs) holds inside fences too.
        let out = eval_line(
            "(handler-case
               (with-capabilities '() (enable-feature \"SHELL\"))
               (error (er) 'no-escalation))",
            &e,
        );
        assert_eq!(out, "NO-ESCALATION");
        assert!(!e.feature_enabled("SHELL"));
    });
}
