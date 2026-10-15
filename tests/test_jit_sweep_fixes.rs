//! Regression tests for the typed-JIT bug sweep: each case compares the
//! compiled (`defun-typed`) result with what the evaluator produces for the
//! same operation, or with the documented contract.

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line};

fn env() -> Shared<Environment> {
    Environment::with_stdlib()
}

/// Unary `-` on a `float64` is IEEE negation: `(- 0.0)` is `-0.0`, exactly as
/// the evaluator's `(- 0.0)`. The compiled form used to be `0 - x`, which
/// loses the sign of zero (`0.0 - 0.0` is `+0.0`).
#[test]
fn float_unary_minus_keeps_the_sign_of_zero() {
    let e = env();
    eval_line("(defun-typed (neg float64) ((x float64)) (- x))", &e);
    for x in ["0.0", "-0.0", "1.5", "-1.5"] {
        let compiled = eval_line(&format!("(neg {x})"), &e);
        let evaluator = eval_line(&format!("(- {x})"), &e);
        assert_eq!(compiled, evaluator, "(- {x}): compiled vs evaluator");
    }
    assert_eq!(eval_line("(neg 0.0)", &e), "-0.0");
    assert_eq!(eval_line("(neg -0.0)", &e), "0.0");
}
