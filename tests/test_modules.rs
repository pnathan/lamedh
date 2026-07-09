//! Modules (0.3): DEFMODULE / WITH-MODULE / IMPORT — MODULE:SYMBOL naming
//! over the flat global namespace, function-to-module association, and
//! module-provided custom capabilities in the attenuable lattice.

mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

fn env() -> lamedh::Shared<lamedh::environment::Environment> {
    let e = env_with_stdlib();
    eval_line(
        "(defmodule geometry (:export area) (:provides FAST-MATH))",
        &e,
    );
    eval_line(
        "(with-module geometry
           (defun helper (x) (* x 3))
           (defun area (r) (helper (* r r))))",
        &e,
    );
    e
}

#[test]
fn definitions_are_stored_qualified_and_cross_reference() {
    let e = env();
    // area calls helper through its qualified name.
    assert_eq!(eval_line("(geometry:area 2)", &e), "12");
    assert_eq!(eval_line("(geometry:helper 5)", &e), "15");
    // The unqualified names are NOT bound by definition alone.
    let out = eval_line("(area 2)", &e);
    assert!(out.contains("Unbound"), "got: {out}");
}

#[test]
fn import_binds_exports_only() {
    let e = env();
    eval_line("(import geometry)", &e);
    assert_eq!(eval_line("(area 3)", &e), "27");
    // helper was not exported.
    let out = eval_line("(helper 3)", &e);
    assert!(out.contains("Unbound"), "got: {out}");
    let out = eval_line("(import nosuchmodule)", &e);
    assert!(out.contains("unknown module"), "got: {out}");
}

#[test]
fn functions_are_associated_with_their_module() {
    let e = env();
    assert_eq!(eval_line("(module-of 'geometry:area)", &e), "GEOMETRY");
    assert_eq!(
        eval_line("(module-functions 'geometry)", &e),
        "(GEOMETRY:HELPER GEOMETRY:AREA)"
    );
    assert_eq!(eval_line("(module-exports 'geometry)", &e), "(AREA)");
}

#[test]
fn later_with_module_bodies_see_earlier_locals() {
    let e = env();
    eval_line(
        "(with-module geometry (defun twice-area (r) (* 2 (area r))))",
        &e,
    );
    assert_eq!(eval_line("(geometry:twice-area 2)", &e), "24");
}

#[test]
fn provided_capabilities_join_the_attenuable_lattice() {
    let e = env();
    // Held by registration at the outermost level...
    assert_eq!(
        eval_line("(member 'FAST-MATH (capabilities-effective))", &e),
        "(FAST-MATH)"
    );
    // ...gates via require-capability inside fences...
    assert_eq!(
        eval_line(
            "(with-capabilities '(FAST-MATH) (require-capability 'FAST-MATH))",
            &e
        ),
        "T"
    );
    // ...and attenuates away like any built-in capability.
    let out = eval_line(
        "(handler-case (with-capabilities '() (require-capability 'FAST-MATH))
           (error (er) (error-message er)))",
        &e,
    );
    assert!(out.contains("capability denied: FAST-MATH"), "got: {out}");
    // Custom capabilities never grant kernel abilities: READ-FS still denied.
    let out = eval_line(
        "(handler-case (read-file \"/etc/hostname\") (error (er) 'denied))",
        &e,
    );
    assert_eq!(out, "DENIED");
}

#[test]
fn quoted_data_is_not_rewritten() {
    let e = env();
    eval_line("(with-module geometry (defun tags () '(area helper)))", &e);
    // The quoted list keeps unqualified symbols.
    assert_eq!(eval_line("(geometry:tags)", &e), "(AREA HELPER)");
}
