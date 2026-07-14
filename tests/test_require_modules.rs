//! REQUIRE / PROVIDE module registry (issue #256, epic #253).
//!
//! Covers: load-once (duplicate require is a no-op), dependency cycles,
//! missing PROVIDE, load failure, unknown module, registered-source
//! loading, capability gating of the disk resolution tier, explicit
//! reload, the embedder API (`register_module`/`require_module`/
//! `loaded_modules`), and that `Environment::with_stdlib()` still loads
//! everything it always has.

use lamedh::environment::Environment;
use lamedh::{eval_line, eval_str};

fn line(env: &lamedh::Shared<Environment>, src: &str) -> String {
    eval_line(src, env)
}

// ---------------------------------------------------------------------
// with_prelude(): lighter than with_stdlib(), require-able optional libs
// ---------------------------------------------------------------------

#[test]
fn with_prelude_has_no_optional_vocabulary_until_required() {
    let env = Environment::with_prelude();
    // Prelude vocabulary works.
    assert_eq!(line(&env, "(+ 1 2 3)"), "6");
    line(&env, "(setf x 5)");
    assert_eq!(line(&env, "x"), "5");
    // Optional-library vocabulary is absent.
    assert!(!env.is_bound("SHELL:SHELL-OK-P"));
    assert!(!env.is_bound("DEFTEST"));
    assert!(!env.is_bound("DEFMODULE"));
    assert_eq!(line(&env, "(loaded-modules)"), "()");
}

#[test]
fn require_pulls_in_an_optional_embedded_module() {
    let env = Environment::with_prelude();
    // shell is a fully-qualified module now (#56): its vocabulary is SHELL:*.
    assert!(!env.is_bound("SHELL:SHELL-OK-P"));
    assert_eq!(line(&env, "(require 'shell)"), "SHELL");
    assert!(env.is_bound("SHELL:SHELL-OK-P"));
    assert_eq!(line(&env, "(module-state 'shell)"), "REQUIRE-LOADED");
    assert_eq!(line(&env, "(member 'SHELL (loaded-modules))"), "(SHELL)");
}

#[test]
fn embedded_optional_modules_load_without_read_fs() {
    // Sandboxed environments (READ-FS off) must still be able to pull in
    // embedded optional libraries -- only the disk resolution tier needs
    // the capability.
    let env = Environment::with_prelude();
    assert!(!env.feature_enabled("READ-FS"));
    assert_eq!(line(&env, "(require 'testing)"), "TESTING");
    assert!(env.is_bound("DEFTEST"));
}

#[test]
fn require_accepts_a_string_name_too() {
    let env = Environment::with_prelude();
    assert_eq!(line(&env, "(require \"lisp15\")"), "LISP15");
}

// ---------------------------------------------------------------------
// Load-once / no-op semantics
// ---------------------------------------------------------------------

#[test]
fn duplicate_require_is_a_no_op() {
    let env = Environment::with_prelude();
    eval_str("(require 'guard)", &env).unwrap();
    // Redefine a function GUARD provides, then require it again: if this
    // were NOT a no-op, the redefinition would be clobbered by re-eval.
    eval_str("(defun with-fuel-marker () 'still-here)", &env).unwrap();
    assert_eq!(line(&env, "(require 'guard)"), "GUARD");
    assert_eq!(line(&env, "(with-fuel-marker)"), "STILL-HERE");
}

// ---------------------------------------------------------------------
// Cycles, missing PROVIDE, load failure, unknown module
// ---------------------------------------------------------------------

#[test]
fn dependency_cycle_names_the_chain() {
    let env = Environment::with_prelude();
    env.register_module("A", "(require 'b) (provide 'a)");
    env.register_module("B", "(require 'c) (provide 'b)");
    env.register_module("C", "(require 'a) (provide 'c)");
    let err = eval_str("(require 'a)", &env).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("dependency cycle"), "message was: {msg}");
    assert!(msg.contains("A"), "message was: {msg}");
    assert!(msg.contains("B"), "message was: {msg}");
    assert!(msg.contains("C"), "message was: {msg}");
}

#[test]
fn missing_provide_fails_the_load_and_leaves_it_unloaded() {
    let env = Environment::with_prelude();
    env.register_module("SLOPPY", "(defun sloppy-fn () 42)");
    let err = eval_str("(require 'sloppy)", &env).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("provide"), "message was: {msg}");
    assert_eq!(line(&env, "(module-state 'sloppy)"), "REQUIRE-UNLOADED");
    // Partial definitions are NOT rolled back -- documented, not pretended.
    assert!(env.is_bound("SLOPPY-FN"));
    // A second attempt retries from scratch rather than staying "stuck".
    let err2 = eval_str("(require 'sloppy)", &env).unwrap_err();
    assert!(format!("{err2}").contains("provide"));
}

#[test]
fn load_failure_is_not_marked_loaded_and_records_the_error() {
    let env = Environment::with_prelude();
    env.register_module("BROKEN", "(error \"boom\") (provide 'broken)");
    let err = eval_str("(require 'broken)", &env).unwrap_err();
    assert!(format!("{err}").contains("boom"));
    assert_eq!(line(&env, "(module-state 'broken)"), "REQUIRE-UNLOADED");
    let info_err = line(&env, "(cdr (assoc 'error (module-info 'broken)))");
    assert!(
        info_err.contains("boom"),
        "module-info error was: {info_err}"
    );
}

#[test]
fn unknown_module_is_a_clear_error() {
    let env = Environment::with_prelude();
    let err = eval_str("(require 'this-does-not-exist-anywhere)", &env).unwrap_err();
    assert!(format!("{err}").contains("unknown module"));
}

// ---------------------------------------------------------------------
// Registered-source loading (embedder API) and resolution order
// ---------------------------------------------------------------------

#[test]
fn registered_source_takes_priority_over_embedded() {
    let env = Environment::with_prelude();
    // SHELL is an embedded module; register a host override under the same
    // name and confirm the host's source wins.
    env.register_module(
        "SHELL",
        "(defun sh (cmd) 'host-shell-stub) (provide 'shell)",
    );
    assert_eq!(line(&env, "(require 'shell)"), "SHELL");
    assert_eq!(line(&env, "(sh \"ignored\")"), "HOST-SHELL-STUB");
    // The real SHELL-OK-P helper from the embedded file was never evaluated.
    assert!(!env.is_bound("SHELL-OK-P"));
}

#[test]
fn require_module_embedder_api_from_rust() {
    let env = Environment::with_prelude();
    let result = lamedh::require_module("guard", &env).expect("require_module should succeed");
    assert_eq!(lamedh::printer::print(&result), "GUARD");
    assert!(env.is_bound("FUEL-REMAINING"));
}

#[test]
fn loaded_modules_embedder_api_from_rust() {
    let env = Environment::with_prelude();
    assert!(lamedh::loaded_modules(&env).is_empty());
    lamedh::require_module("lisp15", &env).unwrap();
    let loaded = lamedh::loaded_modules(&env);
    assert!(loaded.contains(&"LISP15".to_string()), "{loaded:?}");
}

// ---------------------------------------------------------------------
// Disk resolution tier: capability-gated, host-configured search paths
// ---------------------------------------------------------------------

#[test]
fn disk_module_requires_read_fs() {
    let dir = std::env::temp_dir().join(format!("lamedh-require-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("diskmod.lisp"),
        "(def disk-mod-marker 'loaded) (provide 'diskmod)",
    )
    .unwrap();

    // Without READ-FS: disk resolution must not silently succeed.
    let env = Environment::with_prelude();
    env.add_module_search_path(dir.clone());
    let err = eval_str("(require 'diskmod)", &env).unwrap_err();
    let msg = format!("{err}");
    assert!(
        msg.contains("READ-FS") || msg.contains("capability"),
        "expected a capability error, got: {msg}"
    );
    assert!(!env.is_bound("DISK-MOD-MARKER"));

    // With READ-FS granted: it resolves and loads.
    let env2 = Environment::with_prelude();
    env2.enable_feature("READ-FS");
    env2.add_module_search_path(dir.clone());
    assert_eq!(eval_line("(require 'diskmod)", &env2), "DISKMOD");
    assert!(env2.is_bound("DISK-MOD-MARKER"));
    let source = eval_line("(cdr (assoc 'source (module-info 'diskmod)))", &env2);
    assert!(source.contains("disk:"), "source was: {source}");

    std::fs::remove_dir_all(&dir).ok();
}

// ---------------------------------------------------------------------
// Explicit reload (development operation)
// ---------------------------------------------------------------------

#[test]
fn require_reload_forces_re_evaluation() {
    let env = Environment::with_prelude();
    env.register_module("COUNTER", "(defun counter-val () 1) (provide 'counter)");
    eval_str("(require 'counter)", &env).unwrap();
    assert_eq!(line(&env, "(counter-val)"), "1");

    // Ordinary require does NOT pick up a changed registration.
    env.register_module("COUNTER", "(defun counter-val () 2) (provide 'counter)");
    eval_str("(require 'counter)", &env).unwrap();
    assert_eq!(line(&env, "(counter-val)"), "1");

    // require-reload does.
    eval_str("(require-reload 'counter)", &env).unwrap();
    assert_eq!(line(&env, "(counter-val)"), "2");
}

// ---------------------------------------------------------------------
// with_stdlib(): unchanged eager load; require becomes a documented no-op
// ---------------------------------------------------------------------

#[test]
fn with_stdlib_still_loads_every_optional_module_and_marks_it_loaded() {
    let env = Environment::with_stdlib();
    // Spot-check vocabulary from several optional files, exactly as before.
    // shell is a fully-qualified module now (#56): SHELL:SH, not flat SH.
    assert!(env.is_bound("SHELL:SH"));
    assert!(env.is_bound("DEFTEST"));
    assert!(env.is_bound("DEFMODULE"));
    assert!(env.is_bound("HELP"));
    assert_eq!(line(&env, "(module-state 'condensation)"), "REQUIRE-LOADED");
    assert_eq!(line(&env, "(module-state 'text)"), "REQUIRE-LOADED");
    assert_eq!(line(&env, "(module-state 'help-data)"), "REQUIRE-LOADED");
    // A later require is a documented no-op, not a redundant re-evaluation.
    assert_eq!(line(&env, "(require 'shell)"), "SHELL");
    assert_eq!(line(&env, "(module-state 'ports)"), "REQUIRE-LOADED");
    // Every OPTIONAL_MODULES row (src/lib.rs) — 19 pre-existing, the five
    // #257 codec modules (base64, hex, url, json, mime), the three #258
    // networking modules (net, tcp, udp), the #259 http module, the
    // #260 os/os-linux modules, and the #365 tls module.
    assert_eq!(line(&env, "(length (loaded-modules))"), "31");
}

#[test]
fn with_stdlib_text_module_is_usable() {
    let env = Environment::with_stdlib();
    assert_eq!(
        line(&env, "(text:utf8->string (text:string->utf8 \"hi\"))"),
        "\"hi\""
    );
}
