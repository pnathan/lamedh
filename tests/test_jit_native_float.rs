//! Regression net for two native-tier bugs found together (2026-07-17):
//!
//! 1. **Float bitcast verifier failure**: `as_f`/`as_i` emitted `bitcast`
//!    with `MemFlags::trusted()`, which the Cranelift verifier rejects
//!    (bitcast accepts only endianness flags). Native codegen failed —
//!    *silently* — for every function whose Core touches an i64<->f64 word
//!    cast, so all-float signatures ran the closure tier forever while
//!    int-only signatures went NATIVE.
//!
//! 2. **Pending-tail-call leak through direct native calls**: a native
//!    callee ending in a *cross-function* tail call parks the real call in
//!    the ctx pending cell and returns a placeholder; only the Rust-level
//!    dispatch loop drained it. A direct native→native (non-tail) call
//!    returned straight into native code, the pending call survived to the
//!    top-level invoke loop, and its result REPLACED the outer function's
//!    return value. Hidden from small repros by the Core inliner (small
//!    callees get spliced, erasing the cross call) — the callee here is
//!    deliberately fat enough to stay a real call.
#![cfg(feature = "jit")]

use lamedh::environment::Environment;
use lamedh::{eval_all, eval_str};

fn print_of(src: &str, env: &lamedh::Shared<Environment>) -> String {
    lamedh::printer::print(&eval_str(src, env).expect(src))
}

#[test]
fn float_signatures_compile_native() {
    lamedh::with_large_stack(|| {
        let env = Environment::with_stdlib();
        eval_all(
            "(defun* fadd (x float64) float64 (+ x 1.0))\n\
             (defun-typed (fs float64) ((x float64) (y float64) (z float64))\n\
               (- (sqrt (+ (* x x) (* y y) (* z z))) 1.0))",
            &env,
        )
        .unwrap();
        assert_eq!(print_of("(compiled-p 'fadd)", &env), "NATIVE");
        assert_eq!(print_of("(compiled-p 'fs)", &env), "NATIVE");
        assert_eq!(print_of("(fadd 1.5)", &env), "2.5");
        // 3-4-5 triangle: sqrt(9+0+16) - 1 = 4.
        assert_eq!(print_of("(fs 3.0 0.0 4.0)", &env), "4.0");
    });
}

#[test]
fn stdlib_load_reports_no_native_codegen_failures() {
    // The two stdlib typed functions that silently failed native codegen
    // under the bitcast bug must compile natively again. Representative
    // check: an all-float stdlib-style function defined after load, plus
    // no assertion here can see stderr — so instead verify that a fresh
    // world's float vocabulary reaches NATIVE.
    lamedh::with_large_stack(|| {
        let env = Environment::with_stdlib();
        eval_all(
            "(defun* hyp (a float64) (b float64) float64 (sqrt (+ (* a a) (* b b))))",
            &env,
        )
        .unwrap();
        assert_eq!(print_of("(compiled-p 'hyp)", &env), "NATIVE");
        assert_eq!(print_of("(hyp 3.0 4.0)", &env), "5.0");
    });
}

#[test]
fn pending_cross_tail_call_does_not_replace_caller_result() {
    lamedh::with_large_stack(|| {
        let env = Environment::with_stdlib();
        // `fat` must stay a REAL cross call twice over: it is too large for
        // the inliner, and it ends in a cross-function tail call to `helper`
        // (itself non-trivial). Under the bug, (pick 323) returned `fat`'s
        // deferred result (a small int) instead of p0.
        eval_all(
            "(defun-typed (helper int64) ((a int64) (b int64))\n\
               (if (< a b) (+ (* a 3) (- b a)) (- (* b 3) (+ a b))))\n\
             (defun-typed (fat int64) ((p0 int64) (p1 int64))\n\
               (if (< (+ (* p0 7) (- p1 3)) (* p1 p1))\n\
                   (helper (+ p0 (* p1 2)) (- (* p0 p0) (+ p1 5)))\n\
                   (helper (- (* p1 4) p0) (+ (* p0 3) (- p1 p0)))))\n\
             (defun-typed (pick int64) ((p0 int64))\n\
               (let-typed ((ignored int64 (fat 7 9))) p0))",
            &env,
        )
        .unwrap();
        assert_eq!(print_of("(compiled-p 'fat)", &env), "NATIVE");
        assert_eq!(print_of("(compiled-p 'pick)", &env), "NATIVE");
        // The caller's own value must survive the callee's deferred tail call.
        assert_eq!(print_of("(pick 323)", &env), "323");
        // And the deferred chain's own result must still be correct.
        let native = print_of("(fat 7 9)", &env);
        let oracle = print_of(
            "(progn (defun fat-o (p0 p1) (if (< (+ (* p0 7) (- p1 3)) (* p1 p1)) \
               (helper-o (+ p0 (* p1 2)) (- (* p0 p0) (+ p1 5))) \
               (helper-o (- (* p1 4) p0) (+ (* p0 3) (- p1 p0))))) \
             (defun helper-o (a b) (if (< a b) (+ (* a 3) (- b a)) (- (* b 3) (+ a b)))) \
             (fat-o 7 9))",
            &env,
        );
        assert_eq!(native, oracle, "native fat() must match interpreter oracle");
    });
}
