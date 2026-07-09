//! TRACE / UNTRACE / TIME / STEP-COUNT (lib/26) and the slot-frame
//! soundness fix their implementation exposed: a `&rest` closure under an
//! intervening LET must read ENCLOSING variables correctly (the rest-call
//! frame now contributes an Opaque scope level, so outer LocalGet depths
//! count it).

mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

#[test]
fn step_count_returns_steps_and_value() {
    let e = env_with_stdlib();
    let out = eval_line("(step-count (+ 1 2))", &e);
    // (steps . 3) with some small positive step count.
    assert!(out.ends_with(" . 3)"), "got: {out}");
    let steps: i64 = out
        .trim_start_matches('(')
        .split(' ')
        .next()
        .unwrap()
        .parse()
        .unwrap();
    assert!(steps > 0 && steps < 1000, "got: {steps}");
}

#[test]
fn step_count_is_the_with_fuel_unit() {
    let e = env_with_stdlib();
    eval_line("(defun fact (n) (if (< n 2) 1 (* n (fact (- n 1)))))", &e);
    // A form measured at S steps runs under a fuel budget of 10*S and dies
    // under a budget of S/2: same counter, same unit.
    assert_eq!(
        eval_line(
            "(let ((s (car (step-count (fact 10)))))
               (list (not (null (errorset (list 'with-fuel (* s 10) '(fact 10)))))
                     (null (errorset (list 'with-fuel (max 1 (/ s 2)) '(fact 10))))))",
            &e
        ),
        "(T T)"
    );
    // step-count nests inside an armed fence.
    assert_eq!(
        eval_line("(cdr (with-fuel 100000 (step-count (fact 5))))", &e),
        "120"
    );
}

#[test]
fn step_count_disarms_even_on_error() {
    let e = env_with_stdlib();
    eval_line("(errorset '(step-count (car 5)))", &e);
    // Fuel must be unarmed again afterwards.
    assert_eq!(eval_line("(kernel-fuel-remaining)", &e), "()");
}

#[test]
fn time_prints_and_returns_the_value() {
    let e = env_with_stdlib();
    assert_eq!(eval_line("(time (+ 20 22))", &e), "42");
}

#[test]
fn trace_wraps_and_untrace_restores() {
    let e = env_with_stdlib();
    eval_line("(defun sq (x) (* x x))", &e);
    eval_line("(trace 'sq)", &e);
    // Traced function still computes correctly.
    assert_eq!(eval_line("(sq 6)", &e), "36");
    eval_line("(untrace 'sq)", &e);
    assert_eq!(eval_line("(sq 7)", &e), "49");
    // Untraced binding is the original (no wrapper indirection left).
    assert_eq!(eval_line("(gethash 'sq $trace-originals)", &e), "()");
    // Double-trace is idempotent; untrace of untraced is a no-op.
    eval_line("(trace 'sq)", &e);
    eval_line("(trace 'sq)", &e);
    eval_line("(untrace 'sq)", &e);
    assert_eq!(eval_line("(sq 3)", &e), "9");
    assert_eq!(eval_line("(untrace 'sq)", &e), "SQ");
}

#[test]
fn rest_closures_read_enclosing_variables_through_let() {
    let e = env_with_stdlib();
    // The soundness regression TRACE exposed: without the Opaque scope
    // level these read the LET's slots instead of the outer variable.
    eval_line("(defun zz (x) x)", &e);
    eval_line(
        "(defun mk (n) (let ((o (+ 1 2))) (lambda (&rest r) (list n o r))))",
        &e,
    );
    assert_eq!(eval_line("(funcall (mk 'zz) 7 8)", &e), "(ZZ 3 (7 8))");
    // With an interp-only init (eval) — the original repro.
    eval_line(
        "(defun mk2 (n) (let ((o (eval n))) (lambda (&rest r) n)))",
        &e,
    );
    assert_eq!(eval_line("(funcall (mk2 'zz))", &e), "ZZ");
}
