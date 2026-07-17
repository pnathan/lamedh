//! Host fast-call API (issue #423): `call_function`/`fn_handle`/`FnHandle::call`.
//!
//! These invoke a Lisp function directly with already-evaluated `LispVal`
//! arguments, skipping the reader and printer entirely. The tests check that
//! results match the equivalent `eval_str` call, that every callable kind
//! `funcall` accepts (builtin, lambda, typed native) works, and that error
//! cases (unbound name, macro rejection, arity, an error signalled inside
//! the callee) surface the same way `eval_str` would.

mod test_helpers;
use lamedh::{LispVal, Shared, call_function, eval_str, fn_handle};
use test_helpers::env_with_stdlib;

fn list(items: Vec<LispVal>) -> LispVal {
    items
        .into_iter()
        .rev()
        .fold(LispVal::Nil, |acc, v| LispVal::Cons {
            car: Shared::new(v),
            cdr: Shared::new(acc),
        })
}

#[test]
fn call_function_stdlib_function_on_a_built_list() {
    let env = env_with_stdlib();
    let xs = list(vec![
        LispVal::Number(1),
        LispVal::Number(2),
        LispVal::Number(3),
        LispVal::Number(4),
    ]);
    let result = call_function("length", &[xs], &env).unwrap();
    assert_eq!(result, LispVal::Number(4));
}

#[test]
fn call_function_builtin() {
    let env = env_with_stdlib();
    let result = call_function(
        "+",
        &[LispVal::Number(1), LispVal::Number(2), LispVal::Number(3)],
        &env,
    )
    .unwrap();
    assert_eq!(result, LispVal::Number(6));
}

#[test]
fn call_function_user_defun_matches_eval_str() {
    let env = env_with_stdlib();
    eval_str("(defun add2 (x y) (+ x y))", &env).unwrap();

    let via_call = call_function("add2", &[LispVal::Number(3), LispVal::Number(4)], &env).unwrap();
    let via_eval = eval_str("(add2 3 4)", &env).unwrap();
    assert_eq!(via_call, LispVal::Number(7));
    assert_eq!(via_call, via_eval);
}

#[test]
fn call_function_name_is_case_normalized() {
    let env = env_with_stdlib();
    eval_str("(defun add2 (x y) (+ x y))", &env).unwrap();
    // Lowercase host-side name, same as any other symbol reference — the
    // reader would uppercase "add2" to ADD2 too.
    let result = call_function("add2", &[LispVal::Number(1), LispVal::Number(1)], &env).unwrap();
    assert_eq!(result, LispVal::Number(2));
    // And the already-uppercased spelling resolves identically.
    let result2 = call_function("ADD2", &[LispVal::Number(1), LispVal::Number(1)], &env).unwrap();
    assert_eq!(result2, LispVal::Number(2));
}

#[cfg(feature = "jit")]
#[test]
fn call_function_defun_star_native_matches_eval_str() {
    lamedh::with_large_stack(|| {
        let env = env_with_stdlib();
        eval_str("(defun* faddi (x int64) int64 (+ x 1))", &env).unwrap();
        // Confirm this actually took the typed-native tier, not the plain
        // closure fallback, so the test exercises what it claims to.
        assert_eq!(
            lamedh::printer::print(&eval_str("(compiled-p 'faddi)", &env).unwrap()),
            "NATIVE"
        );
        let via_call = call_function("faddi", &[LispVal::Number(41)], &env).unwrap();
        let via_eval = eval_str("(faddi 41)", &env).unwrap();
        assert_eq!(via_call, LispVal::Number(42));
        assert_eq!(via_call, via_eval);
    });
}

#[test]
fn call_function_unbound_name_suggests_close_match() {
    let env = env_with_stdlib();
    // LENGTH is a real stdlib function; LENGHT (transposed) is distance 2,
    // within the did-you-mean threshold for a 6-character name.
    let err = call_function("LENGHT", &[], &env).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("Unbound variable"), "message was: {msg}");
    assert!(msg.contains("LENGTH"), "message was: {msg}");
}

#[test]
fn call_function_rejects_macro() {
    let env = env_with_stdlib();
    // DEFUN itself is a stdlib macro (lib/00-core.lisp) — it takes
    // unevaluated argument forms, so fast-call cannot supply it.
    let err = call_function(
        "defun",
        &[LispVal::Symbol(env.intern_symbol("X")), LispVal::Nil],
        &env,
    )
    .unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("macro"), "message was: {msg}");
    assert!(msg.contains("DEFUN"), "message was: {msg}");
}

#[test]
fn call_function_rejects_fexpr() {
    let env = env_with_stdlib();
    // SELECT is defined via DEFEXPR (lib/09-lisp15.lisp) — a fexpr.
    let err = call_function("select", &[LispVal::Nil], &env).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("fexpr"), "message was: {msg}");
}

#[test]
fn call_function_error_inside_callee_propagates() {
    let env = env_with_stdlib();
    eval_str("(defun boom () (error \"kaboom\"))", &env).unwrap();
    let err = call_function("boom", &[], &env).unwrap_err();
    let msg = format!("{err}");
    assert!(msg.contains("kaboom"), "message was: {msg}");
}

#[test]
fn call_function_arity_error_matches_eval_str() {
    let env = env_with_stdlib();
    eval_str("(defun add2 (x y) (+ x y))", &env).unwrap();
    let via_call = call_function("add2", &[LispVal::Number(1)], &env).unwrap_err();
    let via_eval = eval_str("(add2 1)", &env).unwrap_err();
    assert_eq!(format!("{via_call}"), format!("{via_eval}"));
}

#[test]
fn fn_handle_repeated_calls() {
    let env = env_with_stdlib();
    eval_str("(defun add2 (x y) (+ x y))", &env).unwrap();
    let h = fn_handle("add2", &env).unwrap();
    assert_eq!(
        h.call(&[LispVal::Number(1), LispVal::Number(2)], &env)
            .unwrap(),
        LispVal::Number(3)
    );
    assert_eq!(
        h.call(&[LispVal::Number(10), LispVal::Number(20)], &env)
            .unwrap(),
        LispVal::Number(30)
    );
}

#[test]
fn fn_handle_picks_up_redefinition() {
    let env = env_with_stdlib();
    eval_str("(defun tick (x) (+ x 1))", &env).unwrap();
    let h = fn_handle("tick", &env).unwrap();
    assert_eq!(
        h.call(&[LispVal::Number(1)], &env).unwrap(),
        LispVal::Number(2)
    );

    // Redefine TICK between calls — the handle is a name pin, not a closure
    // pin, so the next call must see the new definition.
    eval_str("(defun tick (x) (+ x 100))", &env).unwrap();
    assert_eq!(
        h.call(&[LispVal::Number(1)], &env).unwrap(),
        LispVal::Number(101)
    );
}

#[test]
fn fn_handle_unbound_name_errors_at_creation() {
    let env = env_with_stdlib();
    let err = fn_handle("no-such-function-anywhere", &env).unwrap_err();
    assert!(format!("{err}").contains("Unbound variable"));
}

#[test]
fn fn_handle_created_after_definition_succeeds() {
    let env = env_with_stdlib();
    eval_str("(defun late-bound (x) (* x 2))", &env).unwrap();
    let h = fn_handle("late-bound", &env).unwrap();
    assert_eq!(
        h.call(&[LispVal::Number(21)], &env).unwrap(),
        LispVal::Number(42)
    );
}
