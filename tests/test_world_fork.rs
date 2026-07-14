//! World-fork semantics (`Environment::fork_world` behind
//! `Environment::with_stdlib`/`with_prelude`): every `with_stdlib()` after
//! the first on a thread is a deep-copy fork of a per-thread prototype.
//! These tests pin the contract that makes that sound: forked worlds are
//! observably identical to freshly loaded ones AND fully isolated from each
//! other — plus a timing check that the fork is actually cheap.

use lamedh::environment::Environment;
use lamedh::eval_line;

/// Two environments from the same per-thread prototype: mutations of any
/// kind in one must be invisible in the other.
#[test]
fn forked_worlds_are_isolated() {
    lamedh::with_large_stack(|| {
        let a = Environment::with_stdlib();
        let b = Environment::with_stdlib();

        // Top-level def in A is unbound in B.
        eval_line("(def fork-iso-var 99)", &a);
        assert_eq!(eval_line("fork-iso-var", &a), "99");
        assert!(eval_line("fork-iso-var", &b).contains("Unbound"));

        // Redefining a stdlib function in A does not change B.
        eval_line("(defun cadr (x) 'clobbered)", &a);
        assert_eq!(eval_line("(cadr '(1 2 3))", &a), "CLOBBERED");
        assert_eq!(eval_line("(cadr '(1 2 3))", &b), "2");

        // Property-list writes in A are invisible in B.
        eval_line("(putp 'car \"fork-prop\" 'zork)", &a);
        assert_eq!(eval_line("(getp 'car \"fork-prop\")", &a), "ZORK");
        assert_eq!(eval_line("(getp 'car \"fork-prop\")", &b), "()");

        // Mutating a stdlib-owned hash table (the module registry) in A is
        // invisible in B.
        eval_line("(sethash $modules 'fork-ghost 'yes)", &a);
        assert_eq!(eval_line("(gethash $modules 'fork-ghost)", &a), "YES");
        assert_eq!(eval_line("(gethash $modules 'fork-ghost)", &b), "()");

        // Capability flags: enabling in A leaves B sandboxed.
        a.enable_feature("SHELL");
        assert!(a.feature_enabled("SHELL"));
        assert!(!b.feature_enabled("SHELL"));

        // Dynamic variables defined in A do not exist in B.
        eval_line("(defdynamic *fork-dyn* 7)", &a);
        assert_eq!(eval_line("*fork-dyn*", &a), "7");
        assert!(eval_line("*fork-dyn*", &b).contains("Unbound"));

        // defrecord in A (registers into the typed declaration plane and
        // defines constructor/accessors) is invisible in B.
        eval_line("(defrecord fork-pt (x int64) (y int64))", &a);
        assert_eq!(eval_line("(fork-pt-x (make-fork-pt 1 2))", &a), "1");
        assert!(eval_line("(make-fork-pt 1 2)", &b).contains("Unbound"));
    });
}

/// A mutated world never contaminates later forks: build A, mutate it, then
/// build C and verify C matches a virgin world.
#[test]
fn later_forks_see_a_clean_prototype() {
    lamedh::with_large_stack(|| {
        let a = Environment::with_stdlib();
        eval_line("(def fork-late-var 1)", &a);
        eval_line("(defun mapcar (f l) 'broken)", &a);
        a.enable_feature("SHELL");

        let c = Environment::with_stdlib();
        assert!(eval_line("fork-late-var", &c).contains("Unbound"));
        assert_eq!(eval_line("(mapcar #'1+ '(1 2))", &c), "(2 3)");
        assert!(!c.feature_enabled("SHELL"));
    });
}

/// Symbol identity inside a forked world: interning, quoted stdlib data,
/// and keywords are all one pointer per name, exactly as in a fresh world.
#[test]
fn fork_preserves_symbol_identity() {
    lamedh::with_large_stack(|| {
        let _first = Environment::with_stdlib();
        let env = Environment::with_stdlib(); // a fork

        assert_eq!(eval_line("(eq 'foo 'foo)", &env), "T");
        // A symbol stored inside stdlib data at prototype-build time must be
        // EQ to the same name freshly read in the fork.
        assert_eq!(eval_line("(eq (car $module-def-heads) 'defun)", &env), "T");
        assert_eq!(eval_line("(eq ':kw ':kw)", &env), "T");
        // T itself is the canonical interned T.
        assert_eq!(eval_line("(eq (eq 1 1) 't)", &env), "T");
    });
}

/// Global redefinition/late-binding semantics are unchanged in a fork:
/// variable references in compiled bodies read the call-time value cell.
#[test]
fn fork_preserves_global_late_binding() {
    lamedh::with_large_stack(|| {
        let _first = Environment::with_stdlib();
        let env = Environment::with_stdlib(); // a fork
        eval_line("(def fork-h 1)", &env);
        eval_line("(defun fork-rf () fork-h)", &env);
        assert_eq!(eval_line("(fork-rf)", &env), "1");
        eval_line("(def fork-h 2)", &env);
        assert_eq!(eval_line("(fork-rf)", &env), "2");
    });
}

/// A forked world has the complete stdlib vocabulary, including optional
/// embedded modules, the help database, pattern matching, protocols, and
/// the require registry (requires of already-embedded modules are no-ops).
#[test]
fn fork_has_full_vocabulary() {
    lamedh::with_large_stack(|| {
        let _first = Environment::with_stdlib();
        let env = Environment::with_stdlib(); // a fork

        assert_eq!(
            eval_line("(mapcar (lambda (x) (* x x)) '(1 2 3))", &env),
            "(1 4 9)"
        );
        assert_eq!(eval_line("(match '(1 2) ((?a ?b) (+ ?a ?b)))", &env), "3");
        // Optional embedded modules are loaded and marked provided.
        assert_eq!(
            eval_line("(base64:encode (text:string->utf8 \"hi\"))", &env),
            "\"aGk=\""
        );
        assert_eq!(eval_line("(require 'json)", &env), "JSON");
        assert_eq!(eval_line("(json:stringify 42)", &env), "\"42\"");
        // Help database (a global hash table built at load time) survived
        // the fork.
        assert_eq!(eval_line("(null (gethash HELP-DB 'mapcar))", &env), "()");
        // Protocol dispatch (THE dispatch system) works in a fork.
        assert_eq!(eval_line("(length \"abc\")", &env), "3");
        assert_eq!(eval_line("(length '(a b c))", &env), "3");
    });
}

/// Typed (`defun-typed`) definitions made inside a fork work and stay
/// inside that fork.
#[cfg(feature = "jit")]
#[test]
fn fork_typed_definitions_are_isolated() {
    lamedh::with_large_stack(|| {
        let a = Environment::with_stdlib();
        eval_line("(defun-typed (fork-inc int64) ((x int64)) (+ x 1))", &a);
        assert_eq!(eval_line("(fork-inc 41)", &a), "42");

        let b = Environment::with_stdlib();
        assert!(eval_line("(fork-inc 41)", &b).contains("Unbound"));

        // Stdlib defun*-registered typed editions still answer in a fork.
        let r = eval_line("(defun* fork-sq (x) (* x x))", &b);
        assert!(!r.contains("Error"), "defun* in fork failed: {r}");
        assert_eq!(eval_line("(fork-sq 6)", &b), "36");
    });
}

/// The point of the exercise: a fork must be dramatically cheaper than the
/// full load. Generous 2x bound so machine noise can never flake this while
/// still catching a fork that silently regresses to a full reload (the
/// measured ratio is ~40x in both debug and release).
#[test]
fn fork_is_cheaper_than_full_load() {
    lamedh::with_large_stack(|| {
        let t0 = std::time::Instant::now();
        let _proto_owner = Environment::with_stdlib(); // builds prototype + one fork
        let first = t0.elapsed();

        // Median of three forks, so one scheduler hiccup cannot flake.
        let mut times = Vec::new();
        for _ in 0..3 {
            let t = std::time::Instant::now();
            let _e = Environment::with_stdlib();
            times.push(t.elapsed());
        }
        times.sort();
        let fork = times[1];
        assert!(
            fork * 2 < first,
            "fork ({fork:?}) is not meaningfully cheaper than first load ({first:?})"
        );
    });
}

/// with_prelude gets the same treatment.
#[test]
fn prelude_fork_is_isolated_and_complete() {
    lamedh::with_large_stack(|| {
        let a = Environment::with_prelude();
        let b = Environment::with_prelude();
        eval_line("(def prelude-iso 5)", &a);
        assert!(eval_line("prelude-iso", &b).contains("Unbound"));
        assert_eq!(eval_line("(mapcar #'1+ '(1 2 3))", &b), "(2 3 4)");
        // Optional libraries still arrive via require in a forked prelude.
        // ('guard first: 23-match uses $GUARD-SEAL without requiring it —
        // pre-existing behavior, identical on with_prelude_fresh.)
        assert_eq!(eval_line("(require 'guard)", &b), "GUARD");
        assert_eq!(eval_line("(require 'match)", &b), "MATCH");
        assert_eq!(eval_line("(match '(1 2) ((?a ?b) ?a))", &b), "1");
    });
}

/// `with_stdlib_fresh` (the CLI's entry point) produces the same vocabulary
/// as the forked `with_stdlib` path.
#[test]
fn fresh_and_forked_worlds_agree() {
    lamedh::with_large_stack(|| {
        let fresh = Environment::with_stdlib_fresh();
        let _first = Environment::with_stdlib();
        let fork = Environment::with_stdlib();
        for expr in [
            "(mapcar (lambda (x) (* x x)) '(1 2 3))",
            "(match '(a b) ((?x ?y) ?y))",
            "(base64:encode (text:string->utf8 \"hi\"))",
            "(sort '(3 1 2) #'<)",
            "(eq (car $module-def-heads) 'defun)",
        ] {
            assert_eq!(
                eval_line(expr, &fresh),
                eval_line(expr, &fork),
                "fresh/fork disagree on {expr}"
            );
        }
    });
}
