//! #394: `array-div!`, `array-scale!`, `array-fma!`, `array-neg!` and the
//! allocating `array-add`/`array-sub`/`array-mul`. Every compiled result is
//! compared with the tree-walker's on the same inputs.

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line};

fn env() -> Shared<Environment> {
    let e = Environment::with_stdlib();
    eval_line("(defun l (x) ($list->array x))", &e);
    e
}

const DEFS: &[(&str, &str)] = &[
    (
        "t-fdiv",
        "(defun-typed (t-fdiv (array float64)) ((o (array float64)) (a (array float64)) (b (array float64))) (array-div! o a b))",
    ),
    (
        "t-iscale",
        "(defun-typed (t-iscale (array int64)) ((o (array int64)) (a (array int64)) (s int64)) (array-scale! o a s))",
    ),
    (
        "t-fscale",
        "(defun-typed (t-fscale (array float64)) ((o (array float64)) (a (array float64)) (s float64)) (array-scale! o a s))",
    ),
    (
        "t-ifma",
        "(defun-typed (t-ifma (array int64)) ((o (array int64)) (a (array int64)) (b (array int64)) (c (array int64))) (array-fma! o a b c))",
    ),
    (
        "t-ffma",
        "(defun-typed (t-ffma (array float64)) ((o (array float64)) (a (array float64)) (b (array float64)) (c (array float64))) (array-fma! o a b c))",
    ),
    (
        "t-ineg",
        "(defun-typed (t-ineg (array int64)) ((o (array int64)) (a (array int64))) (array-neg! o a))",
    ),
    (
        "t-fneg",
        "(defun-typed (t-fneg (array float64)) ((o (array float64)) (a (array float64))) (array-neg! o a))",
    ),
    (
        "t-iadd",
        "(defun-typed (t-iadd (array int64)) ((a (array int64)) (b (array int64))) (array-add a b))",
    ),
    (
        "t-fsub",
        "(defun-typed (t-fsub (array float64)) ((a (array float64)) (b (array float64))) (array-sub a b))",
    ),
    (
        "t-imul",
        "(defun-typed (t-imul (array int64)) ((a (array int64)) (b (array int64))) (array-mul a b))",
    ),
];

/// (compiled fn, interpreted op, args)
const CALLS: &[(&str, &str, &str)] = &[
    (
        "t-fdiv",
        "array-div!",
        "(l '(0.0 0.0 0.0 7.0)) (l '(1.0 -2.0 0.0 3.0)) (l '(2.0 0.0 -0.0 3.0))",
    ),
    (
        "t-iscale",
        "array-scale!",
        "(l '(0 0 0)) (l '(1 -2 4611686018427387904)) 2",
    ),
    (
        "t-fscale",
        "array-scale!",
        "(l '(0.0 0.0)) (l '(1.5 -0.0)) -2.0",
    ),
    (
        "t-ifma",
        "array-fma!",
        "(l '(0 0)) (l '(3 9223372036854775807)) (l '(4 2)) (l '(5 1))",
    ),
    (
        "t-ffma",
        "array-fma!",
        "(l '(0.0 0.0)) (l '(0.1 1.0e308)) (l '(10.0 10.0)) (l '(-1.0 -1.0e308))",
    ),
    (
        "t-ineg",
        "array-neg!",
        "(l '(0 0 0)) (l '(5 -9223372036854775808 0))",
    ),
    ("t-fneg", "array-neg!", "(l '(9.0 9.0 9.0)) (l '(0.0 -1.5))"),
    ("t-iadd", "array-add", "(l '(1 2 3)) (l '(10 20))"),
    ("t-fsub", "array-sub", "(l '(1.0 2.5)) (l '(0.5 0.5 9.0))"),
    ("t-imul", "array-mul", "(l '(3 -4)) (l '(5 6))"),
];

#[test]
fn every_new_op_compiles_and_matches_the_tree_walker() {
    let e = env();
    for (name, def) in DEFS {
        eval_line(def, &e);
        let tier = eval_line(&format!("(compiled-p '{name})"), &e);
        #[cfg(feature = "jit")]
        assert_eq!(tier, "NATIVE", "{name}");
        #[cfg(not(feature = "jit"))]
        assert_ne!(tier, "()", "{name}");
    }
    for (f, op, args) in CALLS {
        let got = eval_line(&format!("(array->list ({f} {args}))"), &e);
        let want = eval_line(&format!("(array->list ({op} {args}))"), &e);
        assert_eq!(got, want, "{f} vs {op} on {args}");
    }
}

#[test]
fn tree_walker_semantics() {
    let e = env();
    // FMA is fused: 0.1*10 - 1 rounds once, so it is not 0.0.
    let fused = eval_line(
        "(array->list (array-fma! (l '(0.0)) (l '(0.1)) (l '(10.0)) (l '(-1.0))))",
        &e,
    );
    assert_eq!(eval_line("(list (- (* 0.1 10.0) 1.0))", &e), "(0.0)");
    assert_ne!(fused, "(0.0)");
    // Negation flips the sign of zero; int negation wraps.
    assert_eq!(
        eval_line(
            "(array->list (array-neg! (l '(1.0 1)) (l '(0.0 -9223372036854775808))))",
            &e
        ),
        "(-0.0 -9223372036854775808)"
    );
    // min(len) of the array operands; `out` keeps its tail.
    assert_eq!(
        eval_line("(array->list (array-scale! (l '(7 7 7)) (l '(1 2)) 3))", &e),
        "(3 6 7)"
    );
    assert_eq!(
        eval_line("(array->list (array-add (l '(1 2 3)) (l '(10 20))))", &e),
        "(11 22)"
    );
    // In place, aliasing out with an input.
    assert_eq!(
        eval_line(
            "(let ((v (l '(1.0 2.0)))) (array-div! v v (l '(2.0 4.0))) (array->list v))",
            &e
        ),
        "(0.5 0.5)"
    );
}

#[test]
fn int_division_and_mixed_elements_are_rejected() {
    let e = env();
    let out = eval_line("(array-div! (l '(0 0)) (l '(1 2)) (l '(1 0)))", &e);
    assert!(out.contains("float64 elements only"), "{out}");
    let out = eval_line("(array-scale! (l '(0)) (l '(1)) 2.0)", &e);
    assert!(out.contains("not all int64 or all float64"), "{out}");
    // A typed int64 array-div! stays interpreted.
    eval_line(
        "(defun-typed (t-idiv (array int64)) ((o (array int64)) (a (array int64)) (b (array int64))) (array-div! o a b))",
        &e,
    );
    assert_eq!(eval_line("(compiled-p 't-idiv)", &e), "()");
}

#[test]
fn the_portable_gate_agrees() {
    let e = env();
    for (src, want) in [
        (
            "'(o a s) '((array-scale! o a (+ s 0)))",
            "(COMPILEABLE (-> ((ARRAY INT64) (ARRAY INT64) INT64) (ARRAY INT64)))",
        ),
        (
            "'(o a x) '((array-scale! a a (+ x 0.0)) (array-neg! o a) (array-div! o o a))",
            "(COMPILEABLE (-> ((ARRAY FLOAT64) (ARRAY FLOAT64) FLOAT64) (ARRAY FLOAT64)))",
        ),
        (
            "'(a b) '((array-add a (array-mul b (array-scale! b b (+ 1 0)))))",
            "(COMPILEABLE (-> ((ARRAY INT64) (ARRAY INT64)) (ARRAY INT64)))",
        ),
        (
            "'(o a b) '((array-div! o a (array-scale! b b (+ 1 0))))",
            "(BLOCKED \"array-div! is float64-only; int64 stays interpreted\")",
        ),
    ] {
        assert_eq!(
            eval_line(&format!("(hm-compile-lambda 'x {src})"), &e),
            want,
            "{src}"
        );
    }
}
