//! #398: natural `log` and float `expt` compile via the libm trampolines,
//! bit-identical to the evaluator; int^int `expt` stays interpreted (its
//! result type depends on the exponent's sign and it can overflow).

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line};

fn env() -> Shared<Environment> {
    Environment::with_stdlib()
}

fn assert_compiled(e: &Shared<Environment>, name: &str) {
    let tier = eval_line(&format!("(compiled-p '{name})"), e);
    #[cfg(feature = "jit")]
    assert_eq!(tier, "NATIVE", "{name}");
    #[cfg(not(feature = "jit"))]
    assert_ne!(tier, "()", "{name}");
}

#[test]
fn log_and_float_expt_compile_and_match_the_evaluator() {
    let e = env();
    eval_line("(defun el-ff (b x) (expt (+ b 0.0) (+ x 0.0)))", &e);
    eval_line("(defun el-fi (b n) (expt (+ b 0.0) (+ n 0)))", &e);
    eval_line("(defun el-if (n x) (expt (+ n 0) (+ x 0.0)))", &e);
    eval_line("(defun el-log (x) (log (+ x 0.0)))", &e);
    for f in ["el-ff", "el-fi", "el-if", "el-log"] {
        assert_compiled(&e, f);
    }
    for (b, x) in [
        ("2.0", "1.45"),
        ("0.5", "-2.25"),
        ("-8.0", "0.5"),
        ("0.0", "-1.0"),
        ("1e300", "2.0"),
    ] {
        assert_eq!(
            eval_line(&format!("(el-ff {b} {x})"), &e),
            eval_line(&format!("(expt {b} {x})"), &e),
            "ff {b} {x}"
        );
    }
    // powi takes `n as i32`: huge exponents truncate identically.
    for (b, n) in [
        ("1.5", "-3"),
        ("2.0", "10"),
        ("-2.0", "7"),
        ("2.0", "5000000000"),
    ] {
        assert_eq!(
            eval_line(&format!("(el-fi {b} {n})"), &e),
            eval_line(&format!("(expt {b} {n})"), &e),
            "fi {b} {n}"
        );
    }
    for (n, x) in [("2", "0.5"), ("-3", "2.0"), ("10", "-1.5")] {
        assert_eq!(
            eval_line(&format!("(el-if {n} {x})"), &e),
            eval_line(&format!("(expt {n} {x})"), &e),
            "if {n} {x}"
        );
    }
    for x in ["10.0", "1.0", "0.0", "-1.0", "2.718281828459045"] {
        assert_eq!(
            eval_line(&format!("(el-log {x})"), &e),
            eval_line(&format!("(log {x})"), &e),
            "log {x}"
        );
    }
}

#[test]
fn integer_expt_and_two_argument_log_stay_interpreted() {
    let e = env();
    eval_line("(defun el-ii (n m) (expt (+ n 0) (+ m 0)))", &e);
    let ex = eval_line("(explain-compile 'el-ii)", &e);
    assert!(
        ex.contains("`expt` of int64 by int64 stays interpreted"),
        "{ex}"
    );
    assert_eq!(eval_line("(el-ii 2 10)", &e), "1024");
    assert_eq!(eval_line("(el-ii 2 -1)", &e), "0.5");
    eval_line("(defun el-log2 (x) (log (+ x 0.0) 2.0))", &e);
    assert_eq!(eval_line("(compiled-p 'el-log2)", &e), "()");
    assert_eq!(eval_line("(el-log2 8.0)", &e), "3.0");
}

#[test]
fn the_portable_gate_agrees() {
    let e = env();
    assert_eq!(
        eval_line("(hm-compile-lambda 'f '(v) '((expt (+ v 0.0) 1.45)))", &e),
        "(COMPILEABLE (-> (FLOAT64) FLOAT64))"
    );
    assert_eq!(
        eval_line(
            "(hm-compile-lambda 'f '(n x) '((expt (+ n 0) (+ x 0.0))))",
            &e
        ),
        "(COMPILEABLE (-> (INT64 FLOAT64) FLOAT64))"
    );
    assert_eq!(
        eval_line("(hm-compile-lambda 'f '(x) '((log (+ x 0.0))))", &e),
        "(COMPILEABLE (-> (FLOAT64) FLOAT64))"
    );
    assert_eq!(
        eval_line(
            "(hm-compile-lambda 'f '(n m) '((expt (+ n 0) (+ m 0))))",
            &e
        ),
        "(BLOCKED \"`expt` of int64 by int64 stays interpreted\")"
    );
}
