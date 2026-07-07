#![cfg(feature = "concurrency")]
//! Capability processes (issue #140): SPAWN is the guard fences worn as a
//! share-nothing thread. A child is a fresh interpreter whose authority is
//! the intersection of the requested capabilities with the parent's, under
//! an optional kernel-fuel budget; the body crosses as serialized data.

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line, with_large_stack};

fn env() -> Shared<Environment> {
    Environment::with_stdlib()
}

#[test]
fn await_returns_the_childs_value() {
    with_large_stack(|| {
        let e = env();
        assert_eq!(eval_line("(await (spawn () (+ 40 2)))", &e), "42");
    });
}

#[test]
fn parameterized_fan_out_joins_all_workers() {
    with_large_stack(|| {
        let e = env();
        eval_line(
            "(setq ws (mapcar (lambda (n) (spawn* () (list '* n n))) (list 1 2 3 4 5)))",
            &e,
        );
        assert_eq!(eval_line("(mapcar await ws)", &e), "(1 4 9 16 25)");
    });
}

#[test]
fn child_errors_are_data_and_reraisable() {
    with_large_stack(|| {
        let e = env();
        // Raw outcome is a datum.
        let out = eval_line("(spawn-value (spawn () (car 5)))", &e);
        assert!(out.starts_with("(:ERROR"), "got: {out}");
        assert_eq!(
            eval_line("(spawn-error-p (spawn-value (spawn () (car 5))))", &e),
            "T"
        );
        // await re-signals in the parent, catchably.
        let caught = eval_line(
            "(handler-case (await (spawn () (car 5))) (error (er) 'caught))",
            &e,
        );
        assert_eq!(caught, "CAUGHT");
    });
}

#[test]
fn capabilities_attenuate_never_amplify() {
    with_large_stack(|| {
        let e = env();
        e.enable_feature("READ-FS");
        // Requesting SHELL (parent lacks it) yields an empty child set.
        assert_eq!(
            eval_line(
                "(spawn-value (spawn (:capabilities (SHELL)) (capabilities-effective)))",
                &e
            ),
            "(:OK ())"
        );
        // Requesting a held capability grants exactly it.
        assert_eq!(
            eval_line(
                "(spawn-value (spawn (:capabilities (READ-FS)) (capabilities-effective)))",
                &e
            ),
            "(:OK (READ-FS))"
        );
    });
}

#[test]
fn a_child_cannot_exceed_an_attenuated_parent() {
    with_large_stack(|| {
        let e = env();
        e.enable_feature("READ-FS");
        e.enable_feature("SHELL");
        // Inside a narrowed fence, a child requesting SHELL still can't have
        // it — the fence's effective set is the ceiling.
        let out = eval_line(
            "(with-capabilities '(READ-FS)
               (spawn-value (spawn (:capabilities (READ-FS SHELL)) (capabilities-effective))))",
            &e,
        );
        assert_eq!(out, "(:OK (READ-FS))");
    });
}

#[test]
fn fuel_bounds_a_runaway_child() {
    with_large_stack(|| {
        let e = env();
        let out = eval_line(
            "(handler-case
               (await (spawn (:fuel 40)
                        (progn (defun sp (n) (if (< n 1) 'd (sp (- n 1)))) (sp 100000000))))
               (error (er) 'child-fuel-caught))",
            &e,
        );
        assert_eq!(out, "CHILD-FUEL-CAUGHT");
    });
}

#[test]
fn children_are_share_nothing() {
    with_large_stack(|| {
        let e = env();
        // A def in the parent is invisible to the child (fresh environment),
        // and a def in the child does not leak back.
        eval_line("(setq parent-only 99)", &e);
        let out = eval_line("(spawn-value (spawn () (boundp 'parent-only)))", &e);
        assert_eq!(out, "(:OK ())", "child must not see the parent's binding");
        eval_line("(await (spawn () (setq child-only 7)))", &e);
        assert_eq!(
            eval_line("(boundp 'child-only)", &e),
            "()",
            "child's binding must not leak to the parent"
        );
    });
}
