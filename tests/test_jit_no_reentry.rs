//! #500: a native call that fails *after* entering its body must propagate
//! the error, never fall back to re-running the body interpreted. The
//! auto-typed membrane (`defun`) used to treat every native `Err` as "did not
//! fit" and re-ran the dynamic closure, so side effects performed before the
//! error (a `store` into a zero-copy typed array) happened twice, and a
//! recursion-limit error on a half-mutated structure turned into a silent
//! wrong answer. Only a call the native code *declined to enter* may fall
//! back.

use lamedh::environment::Environment;
use lamedh::printer::print;
use lamedh::{Shared, eval_str, with_large_stack};

fn ok(src: &str, e: &Shared<Environment>) -> String {
    print(&eval_str(src, e).unwrap_or_else(|err| panic!("{src}: {err}")))
}

fn err_of(src: &str, e: &Shared<Environment>) -> String {
    match eval_str(src, e) {
        Ok(v) => panic!("{src}: expected an error, got {}", print(&v)),
        Err(err) => format!("{err}"),
    }
}

/// Define `bump` with `definer`, check it compiled, and call it with an
/// out-of-range index: the increment happens before the failing fetch.
fn assert_side_effect_once(definer: &str) {
    let e = Environment::with_stdlib();
    eval_str(
        &format!("({definer} bump (a i) (store a 0 (+ (fetch a 0) 1)) (fetch a i))"),
        &e,
    )
    .unwrap();
    assert!(
        ok("(see-type 'bump)", &e).ends_with("COMPILED)"),
        "{definer}"
    );
    eval_str("(setq g (typed-array 2 'int64))", &e).unwrap();

    let msg = err_of("(bump g 5)", &e);
    assert!(
        msg.contains("out of bounds") || msg.contains("range"),
        "{msg}"
    );
    assert_eq!(ok("(fetch g 0)", &e), "1", "{definer}: body re-ran");

    // The ERRORSET form of the issue's repro.
    assert_eq!(ok("(errorset '(bump g 5))", &e), "()");
    assert_eq!(ok("(fetch g 0)", &e), "2", "{definer}: body re-ran");
}

#[test]
fn defun_native_error_runs_side_effect_once() {
    assert_side_effect_once("defun");
}

#[test]
fn defun_star_native_error_runs_side_effect_once() {
    assert_side_effect_once("defun*");
}

/// A non-tail recursion past the native depth cap that *consumes* its input
/// as it descends, like the issue's island-counting DFS clearing cells: slot
/// 0 holds the remaining budget, so each frame decrements it. The native
/// run spends ~50,000 of the 50,010 units before the recursion-limit error;
/// a fallback re-run would then see the few units left and return a small
/// wrong count with no error. The error must surface instead.
fn assert_recursion_limit_propagates(definer: &'static str) {
    with_large_stack(move || {
        let e = Environment::with_stdlib();
        eval_str(
            &format!(
                "({definer} consume (a) \
                 (if (= (fetch a 0) 0) 0 \
                     (progn (store a 0 (- (fetch a 0) 1)) (+ 1 (consume a)))))"
            ),
            &e,
        )
        .unwrap();
        assert!(
            ok("(see-type 'consume)", &e).ends_with("COMPILED)"),
            "{definer}"
        );
        eval_str("(setq g (typed-array 1 'int64))", &e).unwrap();
        eval_str("(store g 0 50010)", &e).unwrap();

        let msg = err_of("(consume g)", &e);
        assert!(msg.contains("recursion limit exceeded"), "{definer}: {msg}");
        // The budget the native frames spent stays spent, and no re-run
        // drained the rest.
        let left: i64 = ok("(fetch g 0)", &e).parse().unwrap();
        assert!(left > 0 && left < 50010, "{definer}: {left} units left");

        // Within the cap the same function still answers normally.
        eval_str("(store g 0 10)", &e).unwrap();
        assert_eq!(ok("(consume g)", &e), "10");
        assert_eq!(ok("(fetch g 0)", &e), "0");
    });
}

#[test]
fn defun_recursion_limit_propagates_as_error() {
    assert_recursion_limit_propagates("defun");
}

#[test]
fn defun_star_recursion_limit_propagates_as_error() {
    assert_recursion_limit_propagates("defun*");
}

/// Declining to enter is still transparent: arguments that do not fit the
/// inferred signature take the dynamic closure, as before.
#[test]
fn defun_declined_call_still_falls_back() {
    let e = Environment::with_stdlib();
    eval_str("(defun twice (x) (* x 2))", &e).unwrap();
    assert!(ok("(see-type 'twice)", &e).ends_with("COMPILED)"));
    assert_eq!(ok("(twice 21)", &e), "42");
    assert_eq!(ok("(twice 1.5)", &e), "3.0");
}
