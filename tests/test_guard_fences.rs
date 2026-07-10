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
        // metering must not break tail position (~a few kernel steps per
        // iteration).
        let out = eval_line(
            "(with-fuel 20000000
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
        // One ruler (0.3): the budget is kernel steps, so even reading the
        // gauge costs a few. Most of a 100-step budget must remain.
        let left: i64 = eval_line("(with-fuel 100 (fuel-remaining))", &e)
            .parse()
            .expect("fuel-remaining should be a number");
        assert!(left > 50 && left <= 100, "got {left}");
    });
}

#[test]
fn nested_fuel_clamps_and_charges_the_outer_fence() {
    with_large_stack(|| {
        let e = env();
        // Inner request larger than the outer remainder is clamped to it
        // (fence setup itself charges the enclosing budget — anything else
        // would be a free-work escape).
        let clamped: i64 = eval_line(
            "(with-fuel 100000 (with-fuel 100000000 (fuel-remaining)))",
            &e,
        )
        .parse()
        .expect("fuel-remaining should be a number");
        assert!(clamped > 50_000 && clamped < 100_000, "got {clamped}");
        // Work inside the inner fence depletes the outer fence too.
        let out = eval_line(
            "(with-fuel 100000
               (defun guard-n (n) (if (< n 1) 'd (guard-n (- n 1))))
               (with-fuel 50000 (guard-n 100))
               (fuel-remaining))",
            &e,
        );
        let remaining: i64 = out.parse().expect("fuel-remaining should be a number");
        assert!(
            remaining < 99000,
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
            "(with-fuel 100000 (with-capabilities '()
               (handler-case (read-file \"/etc/hostname\") (error (er) 'denied))))",
            &e,
        );
        let b = eval_line(
            "(with-capabilities '() (with-fuel 100000
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
        let out = eval_line(
            "(sandboxed (:fuel 1000 :capabilities (READ-FS))
               (list (fuel-remaining) (capabilities-effective)))",
            &e,
        );
        // (steps-left (READ-FS)) with most of the 1000-step budget left.
        assert!(out.ends_with("(READ-FS))"), "got: {out}");
        let left: i64 = out
            .trim_start_matches('(')
            .split(' ')
            .next()
            .unwrap()
            .parse()
            .expect("fuel number");
        assert!(left > 500 && left <= 1000, "got: {out}");
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

// ------------------------------------------------- capability manifests ----

#[test]
fn capabilities_needed_infers_transitively_and_cycle_safely() {
    with_large_stack(|| {
        let e = env();
        eval_line("(defun cn-fetch (p) (read-file p))", &e);
        eval_line(
            "(defun cn-deploy (p q) (cn-fetch p) (rename-file p q) (sh \"true\"))",
            &e,
        );
        eval_line("(defun cn-pure (x) (* x x))", &e);
        // Self-recursive caller: the walk must terminate.
        eval_line(
            "(defun cn-loop (n) (if (< n 1) (cn-deploy \"a\" \"b\") (cn-loop (- n 1))))",
            &e,
        );
        assert_eq!(
            eval_line("(capabilities-needed 'cn-fetch)", &e),
            "(READ-FS)"
        );
        // Transitive closure incl. the Lisp-layer SH -> SHELL builtin edge,
        // and RENAME-FILE's dual requirement (#273).
        let deploy = eval_line("(capabilities-needed 'cn-deploy)", &e);
        for cap in ["SHELL", "READ-FS", "CREATE-FS"] {
            assert!(
                deploy.contains(cap),
                "deploy manifest missing {cap}: {deploy}"
            );
        }
        assert_eq!(eval_line("(capabilities-needed 'cn-pure)", &e), "()");
        assert_eq!(eval_line("(capabilities-needed 'cn-loop)", &e), deploy);
    });
}

#[test]
fn capabilities_needed_form_analyzes_raw_forms() {
    with_large_stack(|| {
        let e = env();
        let out = eval_line(
            "(capabilities-needed-form '(write-file \"x\" (sh \"date\")))",
            &e,
        );
        assert!(
            out.contains("CREATE-FS") && out.contains("SHELL"),
            "got: {out}"
        );
        assert_eq!(eval_line("(capabilities-needed-form '(+ 1 2))", &e), "()");
    });
}

#[test]
fn manifest_drives_a_minimal_fence_end_to_end() {
    with_large_stack(|| {
        let e = env();
        e.enable_feature("READ-FS");
        e.enable_feature("CREATE-FS");
        // Infer the manifest, grant exactly that, and confirm an operation
        // OUTSIDE the manifest is denied inside the fence.
        eval_line("(defun mf-probe (p) (file-exists-p p))", &e);
        let out = eval_line(
            "(with-capabilities (capabilities-needed 'mf-probe)
               (list (mf-probe \"/etc/hostname\")
                     (handler-case (write-file \"/tmp/mf-x\" \"hi\")
                       (error (er) 'denied))))",
            &e,
        );
        assert!(out.ends_with("DENIED)"), "got: {out}");
    });
}

// ------------------------------------------- kernel fuel backstop (#284) ----

#[test]
fn kernel_backstop_catches_pre_fence_closure_loops() {
    with_large_stack(|| {
        let e = env();
        // The flagship Phase-1 leak: a function defined OUTSIDE the fence is
        // not instrumented, so the Lisp-level ticks never fire — the kernel
        // step budget must terminate it anyway, catchably.
        eval_line(
            "(defun kb-spin (n) (if (< n 1) 'leaked (kb-spin (- n 1))))",
            &e,
        );
        let out = eval_line(
            "(handler-case (with-fuel 50 (kb-spin 100000000))
               (error (er) (error-message er)))",
            &e,
        );
        assert!(
            out.contains("fuel exhausted"),
            "kernel backstop must catch the uninstrumented loop, got: {out}"
        );
        // The interpreter (and the disarmed counter) are healthy afterwards.
        assert_eq!(eval_line("(+ 1 2)", &e), "3");
        assert_eq!(eval_line("(kernel-fuel-remaining)", &e), "()");
    });
}

#[test]
fn kernel_backstop_arms_and_restores_around_fences() {
    with_large_stack(|| {
        let e = env();
        // Unarmed outside any fence.
        assert_eq!(eval_line("(kernel-fuel-remaining)", &e), "()");
        // One ruler (0.3): armed at exactly the budget (minus the steps
        // spent reaching the gauge).
        let inside: i64 = eval_line("(with-fuel 100 (kernel-fuel-remaining))", &e)
            .parse()
            .expect("armed count");
        assert!(
            inside > 50 && inside <= 100,
            "expected <=100 steps armed, got {inside}"
        );
        // Disarmed again after the fence exits.
        assert_eq!(eval_line("(kernel-fuel-remaining)", &e), "()");
    });
}

#[test]
fn kernel_backstop_nested_exhaustion_leaves_outer_fence_alive() {
    with_large_stack(|| {
        let e = env();
        eval_line(
            "(defun kb-spin2 (n) (if (< n 1) 'leaked (kb-spin2 (- n 1))))",
            &e,
        );
        let out = eval_line(
            "(with-fuel 1000
               (list (handler-case (with-fuel 50 (kb-spin2 100000000))
                       (error (er) 'inner-caught))
                     (if (kernel-fuel-remaining) 'outer-rearmed 'outer-disarmed)))",
            &e,
        );
        // The inner fence exhausts and is caught; the outer fence re-arms
        // with its remainder (1000 minus the inner spend of 50 and the
        // handling overhead).
        assert_eq!(out, "(INNER-CAUGHT OUTER-REARMED)");
    });
}

#[test]
fn kernel_fuel_setter_denied_inside_fence_usable_outside() {
    with_large_stack(|| {
        let e = env();
        let out = eval_line(
            "(handler-case (with-fuel 100 (kernel-fuel-set! nil))
               (error (er) 'setter-denied))",
            &e,
        );
        assert_eq!(out, "SETTER-DENIED");
        // Host-level use outside any fence: arm, read, disarm.
        assert_eq!(eval_line("(kernel-fuel-set! 5000)", &e), "()");
        let rem: i64 = eval_line("(kernel-fuel-remaining)", &e)
            .parse()
            .expect("count");
        assert!(rem > 4_000 && rem <= 5_000);
        let prev: i64 = eval_line("(kernel-fuel-set! nil)", &e)
            .parse()
            .expect("previous count");
        assert!(prev > 3_000 && prev < 5_000);
        assert_eq!(eval_line("(kernel-fuel-remaining)", &e), "()");
    });
}

// ── #320: dynamic-extent attenuation ─────────────────────────────────────

#[test]
fn attenuation_reaches_helpers_and_eval() {
    with_large_stack(|| {
        let e = env();
        e.enable_feature("READ-FS");
        eval_line(
            "(defun guard-helper-reads () (read-file \"/etc/hostname\"))",
            &e,
        );
        // The old lexical fence only shadowed names IN the body; the dynamic
        // mask fences the whole extent — helpers and eval'd code included.
        assert_eq!(
            eval_line(
                "(handler-case (with-capabilities '() (guard-helper-reads)) (error (er) 'denied))",
                &e
            ),
            "DENIED"
        );
        assert_eq!(
            eval_line(
                "(handler-case (with-capabilities '() (eval '(read-file \"/etc/hostname\"))) (error (er) 'denied))",
                &e
            ),
            "DENIED"
        );
        // Escaped closures run with the CALLER's authority (ambient authority
        // belongs to the execution, not the definition site).
        eval_line(
            "(setq guard-esc (with-capabilities '() (lambda () (guard-helper-reads))))",
            &e,
        );
        assert_eq!(eval_line("(stringp (funcall guard-esc))", &e), "T");
    });
}

#[test]
fn fuel_metering_reaches_helpers_and_natives_fall_back() {
    with_large_stack(|| {
        let e = env();
        // A pre-compiled one-door native: under an armed fence it takes the
        // interpreted fallback, so its internal work charges steps.
        eval_line(
            "(defun guard-native-spin (n acc) (if (< n 1) acc (guard-native-spin (- n 1) (+ acc 1))))",
            &e,
        );
        assert_eq!(eval_line("(guard-native-spin 10 0)", &e), "10"); // warm/compile
        let out = eval_line(
            "(handler-case (with-fuel 50 (guard-native-spin 1000000 0)) (error (er) 'exhausted))",
            &e,
        );
        assert_eq!(out, "EXHAUSTED");
        // Widening from inside is denied; the fence restores on exit.
        let out = eval_line(
            "(handler-case (with-fuel 100 (kernel-fuel-set! 100000)) (error (er) 'widen-denied))",
            &e,
        );
        assert_eq!(out, "WIDEN-DENIED");
        assert_eq!(eval_line("(kernel-fuel-remaining)", &e), "()");
    });
}
