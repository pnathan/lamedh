//! Tests for the `FOR` and `WHILE` iterative special forms.
//!
//! `FOR`:   `(for (var start end [step]) body...)` — integer-counted loop with
//!          an **inclusive** end bound; one reused environment frame.
//! `WHILE`: `(while cond body...)` — loop while `cond` is truthy.
//!
//! Both always return `NIL`.

mod test_helpers;
use lamedh::eval_line;
use test_helpers::env_with_stdlib;

// ---------------------------------------------------------------------------
// FOR — basic counting
// ---------------------------------------------------------------------------

#[test]
fn for_sums_inclusive_range() {
    let env = env_with_stdlib();
    // 1+2+3+4+5 = 15 — proves the end bound is inclusive.
    assert_eq!(
        eval_line("(progn (def s 0) (for (i 1 5) (setq s (+ s i))) s)", &env),
        "15"
    );
}

#[test]
fn for_returns_nil() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(for (i 1 3) i)", &env), "()");
}

#[test]
fn for_single_iteration_when_start_equals_end() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("(progn (def n 0) (for (i 7 7) (setq n (+ n 1))) n)", &env),
        "1"
    );
}

#[test]
fn for_zero_iterations_when_start_past_end() {
    let env = env_with_stdlib();
    // Positive step but start > end → body never runs.
    assert_eq!(
        eval_line("(progn (def n 0) (for (i 5 1) (setq n (+ n 1))) n)", &env),
        "0"
    );
}

#[test]
fn for_with_positive_step() {
    let env = env_with_stdlib();
    // i = 0,2,4,6,8,10 → sum 30
    assert_eq!(
        eval_line(
            "(progn (def s 0) (for (i 0 10 2) (setq s (+ s i))) s)",
            &env
        ),
        "30"
    );
}

#[test]
fn for_counts_down_with_negative_step() {
    let env = env_with_stdlib();
    // i = 10,8,6,4,2 → sum 30
    assert_eq!(
        eval_line(
            "(progn (def s 0) (for (i 10 1 -2) (setq s (+ s i))) s)",
            &env
        ),
        "30"
    );
}

#[test]
fn for_negative_step_zero_iterations_when_start_below_end() {
    let env = env_with_stdlib();
    // Negative step but start < end → body never runs.
    assert_eq!(
        eval_line(
            "(progn (def n 0) (for (i 1 5 -1) (setq n (+ n 1))) n)",
            &env
        ),
        "0"
    );
}

#[test]
fn for_step_lands_exactly_on_end() {
    let env = env_with_stdlib();
    // 1,4,7,10 all <= 10 → count 4
    assert_eq!(
        eval_line(
            "(progn (def n 0) (for (i 1 10 3) (setq n (+ n 1))) n)",
            &env
        ),
        "4"
    );
}

// ---------------------------------------------------------------------------
// FOR — scoping and slot semantics
// ---------------------------------------------------------------------------

#[test]
fn for_var_does_not_leak_into_outer_scope() {
    let env = env_with_stdlib();
    eval_line("(for (zzz 1 3) zzz)", &env);
    assert_eq!(
        eval_line("zzz", &env),
        "Error: Unbound variable: ZZZ",
        "loop variable must stay confined to the loop frame"
    );
}

#[test]
fn for_body_assignment_to_var_does_not_change_iteration_count() {
    let env = env_with_stdlib();
    // Clobbering `i` inside the body must not affect the loop driver.
    assert_eq!(
        eval_line(
            "(progn (def n 0) (for (i 1 3) (setq i 100) (setq n (+ n 1))) n)",
            &env
        ),
        "3"
    );
}

#[test]
fn for_closures_share_the_reused_slot() {
    let env = env_with_stdlib();
    // Documented behavior: one frame is reused, so a closure made in the body
    // observes the final value the slot held (3 — the last value set).
    assert_eq!(
        eval_line(
            "(progn (def f nil) (for (i 1 3) (setq f (lambda () i))) (funcall f))",
            &env
        ),
        "3"
    );
}

// ---------------------------------------------------------------------------
// FOR — nesting and scale
// ---------------------------------------------------------------------------

#[test]
fn for_nested_loops() {
    let env = env_with_stdlib();
    // 10 x 10 grid, +1 each cell → 100
    assert_eq!(
        eval_line(
            "(progn (def n 0) (for (i 1 10) (for (j 1 10) (setq n (+ n 1)))) n)",
            &env
        ),
        "100"
    );
}

#[test]
fn for_large_range_is_correct() {
    let env = env_with_stdlib();
    // sum 1..1000 = 500500
    assert_eq!(
        eval_line(
            "(progn (def s 0) (for (i 1 1000) (setq s (+ s i))) s)",
            &env
        ),
        "500500"
    );
}

#[test]
fn for_bounds_evaluated_as_expressions() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line(
            "(progn (def lo 2) (def hi 6) (def s 0) (for (i lo hi) (setq s (+ s i))) s)",
            &env
        ),
        "20" // 2+3+4+5+6
    );
}

// ---------------------------------------------------------------------------
// FOR — error handling
// ---------------------------------------------------------------------------

#[test]
fn for_step_zero_is_error() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("(for (i 1 3 0) i)", &env),
        "Error: for step must be non-zero"
    );
}

#[test]
fn for_non_symbol_var_is_error() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("(for (1 1 3) i)", &env),
        "Error: for loop variable must be a symbol"
    );
}

#[test]
fn for_non_integer_bound_is_error() {
    let env = env_with_stdlib();
    assert!(
        eval_line("(for (i 1.0 3) i)", &env).starts_with("Error: for start must be an integer"),
        "float bounds should be rejected"
    );
}

#[test]
fn for_bad_spec_arity_is_error() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("(for (i 1) i)", &env),
        "Error: for spec must be (var start end [step])"
    );
    assert_eq!(
        eval_line("(for (i 1 2 3 4) i)", &env),
        "Error: for spec must be (var start end [step])"
    );
}

#[test]
fn for_missing_spec_is_error() {
    let env = env_with_stdlib();
    assert!(eval_line("(for)", &env).starts_with("Error: for requires a spec list"));
}

// ---------------------------------------------------------------------------
// WHILE
// ---------------------------------------------------------------------------

#[test]
fn while_counts_up() {
    let env = env_with_stdlib();
    // c = 0+1+2+3 = 6 over n = 0..3
    assert_eq!(
        eval_line(
            "(progn (def c 0) (def n 0) (while (< n 4) (setq c (+ c n)) (setq n (+ n 1))) c)",
            &env
        ),
        "6"
    );
}

#[test]
fn while_returns_nil() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("(progn (def n 0) (while (< n 2) (setq n (+ n 1))))", &env),
        "()"
    );
}

#[test]
fn while_body_never_runs_when_cond_false() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("(progn (def n 0) (while nil (setq n 99)) n)", &env),
        "0"
    );
}

#[test]
fn while_with_no_body_terminates() {
    let env = env_with_stdlib();
    // A false condition with no body forms is legal and returns nil.
    assert_eq!(eval_line("(while nil)", &env), "()");
}

#[test]
fn while_missing_condition_is_error() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("(while)", &env),
        "Error: while requires a condition and a body"
    );
}

#[test]
fn while_drains_a_list() {
    let env = env_with_stdlib();
    // Walk a list with WHILE + cdr until empty, counting elements.
    assert_eq!(
        eval_line(
            "(progn (def lst '(a b c d)) (def n 0) \
              (while lst (setq n (+ n 1)) (setq lst (cdr lst))) n)",
            &env
        ),
        "4"
    );
}

#[test]
fn while_and_for_compose() {
    let env = env_with_stdlib();
    // For each i in 1..3, run an inner WHILE that adds i three times: 3*(1+2+3)=18
    assert_eq!(
        eval_line(
            "(progn (def s 0) \
              (for (i 1 3) (progn (def k 0) (while (< k 3) (setq s (+ s i)) (setq k (+ k 1))))) s)",
            &env
        ),
        "18"
    );
}
