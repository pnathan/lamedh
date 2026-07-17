//! `JitFnHandle` / `Jit::call_by_id` — the pinned-id membrane fast path
//! (epic #427): no name hash, and all-scalar signatures cross the membrane
//! with a stack word buffer and no write-back machinery.
#![cfg(feature = "jit")]

use lamedh::environment::Environment;
use lamedh::jit::Value;
use lamedh::{eval_all, eval_str};

#[test]
fn scalar_fast_path_matches_named_call() {
    lamedh::with_large_stack(|| {
        let env = Environment::with_stdlib();
        eval_all(
            "(defun* mixy (a int64) (x float64) float64 (+ (float a) (* x 2.0)))",
            &env,
        )
        .unwrap();
        let h = env.jit_fn_handle("mixy").expect("handle");
        for (a, x) in [(1i64, 0.5f64), (-7, 3.25), (1000, -0.125)] {
            let via_handle = env
                .jit_call_handle(&h, &[Value::Int(a), Value::Float(x)])
                .unwrap();
            let via_name = env
                .jit_call("MIXY", &[Value::Int(a), Value::Float(x)])
                .unwrap()
                .unwrap();
            assert_eq!(via_handle, via_name, "a={a} x={x}");
        }
    });
}

#[test]
fn handle_follows_redefinition() {
    lamedh::with_large_stack(|| {
        let env = Environment::with_stdlib();
        eval_all("(defun* g2 (x int64) int64 (+ x 1))", &env).unwrap();
        let h = env.jit_fn_handle("g2").unwrap();
        assert_eq!(
            env.jit_call_handle(&h, &[Value::Int(41)]).unwrap(),
            Value::Int(42)
        );
        // Redefine with a new body; the pinned id must reach the NEW code.
        eval_all("(defun* g2 (x int64) int64 (* x 10))", &env).unwrap();
        assert_eq!(
            env.jit_call_handle(&h, &[Value::Int(41)]).unwrap(),
            Value::Int(410)
        );
    });
}

#[test]
fn arity_and_type_errors_match_named_path() {
    lamedh::with_large_stack(|| {
        let env = Environment::with_stdlib();
        eval_all("(defun* h2 (x int64) int64 (+ x 1))", &env).unwrap();
        let h = env.jit_fn_handle("h2").unwrap();
        let err = env
            .jit_call_handle(&h, &[Value::Int(1), Value::Int(2)])
            .unwrap_err();
        assert!(err.contains("expected 1 args"), "{err}");
        // Unknown name never yields a handle.
        assert!(env.jit_fn_handle("no-such-fn-anywhere").is_none());
    });
}

#[test]
fn compound_signature_falls_back_to_full_membrane() {
    lamedh::with_large_stack(|| {
        let env = Environment::with_stdlib();
        eval_all(
            "(defun* asum2 (a (array float64)) (n int64) float64\n\
               (let ((acc 0.0) (i 0))\n\
                 (while (< i n) (setq acc (+ acc (aref a i))) (setq i (+ i 1)))\n\
                 acc))",
            &env,
        )
        .unwrap();
        // Build a typed array on the Lisp side; pass it through the handle.
        eval_str("(setq *ta2* (typed-array 4 'float64))", &env).unwrap();
        eval_str(
            "(progn (defun* setup2 (a (array float64)) int64\n\
               (progn (aset a 0 1.0) (aset a 1 2.0) (aset a 2 3.0) (aset a 3 4.0) 0))\n\
             (setup2 *ta2*))",
            &env,
        )
        .unwrap();
        let ta = eval_str("*ta2*", &env).unwrap();
        let lamedh::LispVal::TypedArray(obj) = ta else {
            panic!("expected typed array");
        };
        let h = env.jit_fn_handle("asum2").unwrap();
        let got = env
            .jit_call_handle(&h, &[Value::TypedArray(obj), Value::Int(4)])
            .unwrap();
        assert_eq!(got, Value::Float(10.0));
    });
}
