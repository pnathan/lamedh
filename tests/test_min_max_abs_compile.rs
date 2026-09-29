//! #397: compiled `abs`/`min`/`max` evaluate each argument exactly once
//! (bound to a temp slot rather than cloned into the test and branches),
//! and `min`/`max` compile at any arity, folding like lib/05-math.lisp.

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line};

fn env() -> Shared<Environment> {
    Environment::with_stdlib()
}

fn compiled(e: &Shared<Environment>, name: &str) {
    let ex = eval_line(&format!("(explain-compile '{name})"), e);
    assert!(ex.starts_with("((TIER . COMPILED)"), "{name}: {ex}");
}

#[test]
fn abs_argument_runs_once() {
    let e = env();
    // The side-effecting argument bumps I once; recomputation would bump it 2-3x.
    let body = "(let ((i 0)) (+ (abs (progn (setq i (+ i n)) (- 0 i))) (* 100 i)))";
    eval_line(&format!("(defun ab-once (n) {body})"), &e);
    compiled(&e, "ab-once");
    for n in [-3, 0, 1, 7] {
        let want = eval_line(&format!("(let ((n {n})) {body})"), &e);
        assert_eq!(eval_line(&format!("(ab-once {n})"), &e), want, "n={n}");
    }
    assert_eq!(eval_line("(ab-once 1)", &e), "101");
}

#[test]
fn variadic_min_max_compile_and_run_each_argument_once() {
    let e = env();
    let body = "(let ((i 0)) (+ (max (progn (setq i (+ i 1)) n) 3 \
                (progn (setq i (+ i 10)) 2)) (* 100 i)))";
    eval_line(&format!("(defun mx-once (n) {body})"), &e);
    compiled(&e, "mx-once");
    for n in [1, 3, 7] {
        let want = eval_line(&format!("(let ((n {n})) {body})"), &e);
        assert_eq!(eval_line(&format!("(mx-once {n})"), &e), want, "n={n}");
    }
    assert_eq!(eval_line("(mx-once 7)", &e), "1107");
}

#[test]
fn variadic_min_max_match_the_interpreter() {
    let e = env();
    eval_line("(defun mm1 (a) (+ (max (+ a 0)) (min (+ a 0))))", &e);
    eval_line("(defun mx4 (a b c d) (max (+ a 0) b c d))", &e);
    eval_line("(defun mn4 (a b c d) (min (+ a 0) b c d))", &e);
    eval_line("(defun fmx3 (a b c) (max (+ a 0.0) b c))", &e);
    eval_line("(defun fmn3 (a b c) (min (+ a 0.0) b c))", &e);
    for f in ["mm1", "mx4", "mn4", "fmx3", "fmn3"] {
        compiled(&e, f);
    }
    assert_eq!(eval_line("(mm1 4)", &e), "8");
    for args in ["1 5 3 2", "9 -1 9 0", "-4 -4 -8 -2"] {
        for (f, g) in [("mx4", "max"), ("mn4", "min")] {
            assert_eq!(
                eval_line(&format!("({f} {args})"), &e),
                eval_line(&format!("({g} {args})"), &e),
                "{f} {args}"
            );
        }
    }
    // Signed zeros: the right fold's tie-breaking is observable. Reference:
    // the same body, interpreted.
    for args in [
        ["0.0", "-0.0", "0.0"],
        ["-0.0", "0.0", "-0.0"],
        ["1.5", "-2.5", "0.25"],
    ] {
        for (f, body) in [
            ("fmx3", "(max (+ a 0.0) b c)"),
            ("fmn3", "(min (+ a 0.0) b c)"),
        ] {
            let [a, b, c] = args;
            assert_eq!(
                eval_line(&format!("({f} {a} {b} {c})"), &e),
                eval_line(&format!("(let ((a {a}) (b {b}) (c {c})) {body})"), &e),
                "{f} {args:?}"
            );
        }
    }
}

#[test]
fn the_portable_gate_agrees_on_variadic_min_max() {
    let e = env();
    assert_eq!(
        eval_line("(hm-compile-lambda 'mx '(a b c) '((max (+ a 0) b c)))", &e),
        "(COMPILEABLE (-> (INT64 INT64 INT64) INT64))"
    );
    assert_eq!(
        eval_line("(hm-compile-lambda 'mx '(a) '((min (+ a 0.0))))", &e),
        "(COMPILEABLE (-> (FLOAT64) FLOAT64))"
    );
}

/// #501: a min/max/abs nested in a compiled min/max argument must not reuse
/// the outer call's temp slots. Every compiled result is compared with the
/// same body run by the tree-walker.
fn agrees_with_interpreter(e: &Shared<Environment>, name: &str, params: &[&str], body: &str) {
    for vals in [
        [5, 1, 9],
        [7, -3, 2],
        [-4, 6, -8],
        [0, 0, 0],
        [3, 10, -10],
        [-2, -9, 4],
    ] {
        let args: Vec<String> = vals[..params.len()].iter().map(i64::to_string).collect();
        let binds: Vec<String> = params
            .iter()
            .zip(&args)
            .map(|(p, v)| format!("({p} {v})"))
            .collect();
        assert_eq!(
            eval_line(&format!("({name} {})", args.join(" ")), e),
            eval_line(&format!("(let ({}) {body})", binds.join(" ")), e),
            "{name} {args:?}"
        );
    }
}

#[test]
fn nested_min_max_abs_argument_keeps_the_outer_slots() {
    let e = env();
    // The issue's repro forms, via `defun-typed`.
    for (name, body) in [
        ("n501-mxmn", "(max b (min x 9))"),
        ("n501-mxmx", "(max b (max x 0))"),
        ("n501-mxab", "(max b (abs x))"),
        ("n501-mnab", "(min b (abs x))"),
    ] {
        eval_line(
            &format!("(defun-typed ({name} int64) ((b int64) (x int64)) {body})"),
            &e,
        );
        compiled(&e, name);
        agrees_with_interpreter(&e, name, &["b", "x"], body);
    }
    assert_eq!(eval_line("(n501-mxmn 5 1)", &e), "5");
    assert_eq!(eval_line("(n501-mxab 7 -3)", &e), "7");
}

#[test]
fn nested_min_max_abs_at_depth_two_and_more_match_the_interpreter() {
    let e = env();
    let bodies = [
        (
            "d501-a",
            "(min (max a (abs (- b c))) (max (min a b) (abs c)) (abs (min a (max b c))))",
        ),
        (
            "d501-b",
            "(max (min a (max b (min c 1))) (abs (max a (min b c))))",
        ),
        ("d501-c", "(abs (max a (abs (min b (abs c)))))"),
        (
            "d501-d",
            "(max (+ a 0) (min b (max c (abs (- a b)))) (abs c))",
        ),
    ];
    for (name, body) in bodies {
        eval_line(
            &format!("(defun-typed ({name} int64) ((a int64) (b int64) (c int64)) {body})"),
            &e,
        );
        compiled(&e, name);
        agrees_with_interpreter(&e, name, &["a", "b", "c"], body);
    }
    // Float operands take the same path.
    let fbody = "(max (+ a 0.0) (min b (abs (max c -1.5))))";
    eval_line(
        &format!("(defun-typed (d501-f float64) ((a float64) (b float64) (c float64)) {fbody})"),
        &e,
    );
    compiled(&e, "d501-f");
    for [a, b, c] in [["0.5", "2.5", "-3.0"], ["-1.0", "0.25", "1.0"]] {
        assert_eq!(
            eval_line(&format!("(d501-f {a} {b} {c})"), &e),
            eval_line(&format!("(let ((a {a}) (b {b}) (c {c})) {fbody})"), &e),
            "d501-f {a} {b} {c}"
        );
    }
}

#[test]
fn nested_min_in_a_max_loop_accumulator() {
    // #501's real-world hit: Container With Most Water.
    let e = env();
    let body = "(let ((i 0) (j (- (array-length* h) 1)) (best 0)) \
                (while (< i j) \
                  (setq best (max best (* (- j i) (min (aref h i) (aref h j))))) \
                  (if (< (aref h i) (aref h j)) (setq i (+ i 1)) (setq j (- j 1)))) \
                best)";
    eval_line(
        &format!("(defun-typed (max-area int64) ((h (array int64))) {body})"),
        &e,
    );
    compiled(&e, "max-area");
    for (heights, want) in [
        ("(1 8 6 2 5 4 8 3 7)", "49"),
        ("(1 1)", "1"),
        ("(4 3 2 1 4)", "16"),
        ("(1 2 1)", "2"),
    ] {
        let h = format!("($list->array '{heights})");
        assert_eq!(eval_line(&format!("(max-area {h})"), &e), want, "{heights}");
        assert_eq!(
            eval_line(&format!("(let ((h {h})) {body})"), &e),
            want,
            "interpreted {heights}"
        );
    }
}
