//! Integration tests for issue #271: four typed-JIT paths that used to
//! panic/crash the host process where the tree-walking evaluator returns a
//! catchable Lisp error. Each now records a pending error on the call `Ctx`
//! and the membrane raises it as an ordinary error after the call returns.
//!
//! Several of these tests would previously have crashed the *test process*
//! (assert panic or native stack overflow) — passing in-process is the point.

use lamedh::environment::Environment;
use lamedh::{eval_line, with_large_stack};

// (a) Oversized array allocation inside a typed function: the evaluator's
// MAKE-ARRAY returns a Lisp error for an over-limit size; the JIT's
// `alloc_buffer` used to `assert!` (host panic). Now: membrane error.
#[test]
fn oversized_typed_allocation_errors_instead_of_panicking() {
    let env = Environment::new_with_builtins();
    eval_line(
        "(defun-typed (alloc-n int64) ((n int64)) (array-length (array n)))",
        &env,
    );
    // Sanity: a normal size works.
    assert_eq!(eval_line("(alloc-n 10)", &env), "10");
    // Oversized: must surface as an error value/message, not a panic.
    let out = eval_line("(alloc-n 999999999999)", &env);
    assert!(
        out.to_lowercase().contains("error") || out.contains("exceeds maximum"),
        "expected an over-limit allocation error, got: {out}"
    );
}

// (a2) A *negative* array size reinterprets as a huge usize on the native
// side (`n as u64 as usize`) — the nastier variant of the same panic.
#[test]
fn negative_typed_allocation_size_errors_instead_of_panicking() {
    let env = Environment::new_with_builtins();
    eval_line(
        "(defun-typed (alloc-n int64) ((n int64)) (array-length (array n)))",
        &env,
    );
    let out = eval_line("(alloc-n -5)", &env);
    assert!(
        out.to_lowercase().contains("error") || out.contains("exceeds maximum"),
        "expected an over-limit allocation error for a negative size, got: {out}"
    );
}

// (b) Deep non-tail typed recursion: the tree-walker has a recursion limit;
// typed calls used to grow the native stack unbounded until SIGSEGV. Now the
// per-call depth cap refuses with a recursion-limit error. Run on the
// 512 MiB `with_large_stack` thread the interpreter is documented to use —
// the cap must trip long before that stack is exhausted (in every tier:
// native fast path via jit_enter_call, and interpreter/closure editions via
// Ctx::call's own guard).
#[test]
fn deep_nontail_typed_recursion_errors_instead_of_stack_overflow() {
    let result = with_large_stack(|| {
        let env = Environment::new_with_builtins();
        eval_line(
            "(defun-typed (countup int64) ((n int64)) (if (= n 0) 0 (+ 1 (countup (- n 1)))))",
            &env,
        );
        // Under the cap: fine (and genuinely non-tail — the +1 forces a
        // real frame per level).
        let ok = eval_line("(countup 1000)", &env);
        // Far past the cap: a clean error, not a crash.
        let err = eval_line("(countup 100000000)", &env);
        (ok, err)
    });
    assert_eq!(result.0, "1000");
    assert!(
        result.1.to_lowercase().contains("recursion limit"),
        "expected a recursion-limit error, got: {}",
        result.1
    );
}

// (c) A `declare-typed` forward reference that is never defined: the public
// membrane guards this with `is_defined()`, but an *internal* typed→typed
// call used to hit `panic!("... called before it was defined")`. Now: a
// pending error raised at the membrane.
#[test]
fn calling_declared_but_undefined_typed_function_errors_cleanly() {
    let env = Environment::new_with_builtins();
    eval_line("(declare-typed (ghost int64) ((n int64)))", &env);
    eval_line("(defun-typed (summon int64) ((n int64)) (ghost n))", &env);
    let out = eval_line("(summon 5)", &env);
    assert!(
        out.contains("before it was defined") || out.to_lowercase().contains("error"),
        "expected a called-before-defined error, got: {out}"
    );
    // The environment is still healthy afterwards.
    assert_eq!(eval_line("(+ 1 2)", &env), "3");
}

// (d) The stale-arity guard on the interpreter fallthrough
// (`TypedFn::invoke`'s `copy_from_slice`) mirrors the native path's existing
// guard. An arity-changing redefinition recompiles all callers
// (`recompile_all_except`), so the stale case is not constructible from Lisp
// source alone — but redefinition through changed arity must at minimum keep
// working correctly end-to-end, old callers included.
#[test]
fn arity_changing_redefinition_stays_consistent() {
    let env = Environment::new_with_builtins();
    eval_line("(defun-typed (base int64) ((x int64)) (* x 2))", &env);
    eval_line("(defun-typed (caller int64) ((x int64)) (base x))", &env);
    assert_eq!(eval_line("(caller 21)", &env), "42");
    // Redefine `base` with a different arity; `caller` no longer matches it
    // and must fail cleanly (type error or arity error), never panic.
    eval_line(
        "(defun-typed (base int64) ((x int64) (y int64)) (+ x y))",
        &env,
    );
    let out = eval_line("(caller 21)", &env);
    // Whatever the surface behavior (recompile rejects the caller, or the
    // call errors), the process must survive and report something coherent.
    assert!(
        !out.is_empty(),
        "expected some result or error after arity-changing redefinition"
    );
    assert_eq!(eval_line("(base 20 22)", &env), "42");
}
