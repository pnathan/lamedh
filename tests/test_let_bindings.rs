//! Tests for `LET` and `LET*` binding forms, covering multi-form bodies and
//! the single-frame, sequential-scoping semantics of `LET*`.

mod test_helpers;
use lamedh::eval_line;
use test_helpers::env_with_stdlib;

// ---------------------------------------------------------------------------
// Multi-form bodies
// ---------------------------------------------------------------------------

#[test]
fn let_single_form_body_still_works() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(let ((x 2) (y 3)) (* x y))", &env), "6");
}

#[test]
fn let_multi_form_body_runs_in_order_and_returns_last() {
    let env = env_with_stdlib();
    // Two body forms: mutate x, then use it. Returns the value of the last form.
    assert_eq!(
        eval_line("(let ((x 2) (y 3)) (setq x (+ x 1)) (* x y))", &env),
        "9"
    );
}

#[test]
fn let_multi_form_body_sequences_side_effects() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line(
            "(progn (def lg nil) \
              (let ((x 1)) (setq lg (cons 'a lg)) (setq lg (cons 'b lg)) lg))",
            &env
        ),
        "(B A)"
    );
}

#[test]
fn let_star_multi_form_body() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("(let* ((x 1) (y (+ x 1))) (setq x 10) (+ x y))", &env),
        "12"
    );
}

// ---------------------------------------------------------------------------
// Parallel (LET) vs sequential (LET*) scoping
// ---------------------------------------------------------------------------

#[test]
fn let_is_parallel_bindings_see_outer_scope() {
    let env = env_with_stdlib();
    // Inner `y` binds to the OUTER `x` (= 1), because LET evaluates all binding
    // expressions in the enclosing environment, not against sibling bindings.
    assert_eq!(eval_line("(let ((x 1)) (let ((x 2) (y x)) y))", &env), "1");
}

#[test]
fn let_star_is_sequential_bindings_see_prior_bindings() {
    let env = env_with_stdlib();
    // Here `y` sees the just-bound inner `x` (= 2).
    assert_eq!(eval_line("(let ((x 1)) (let* ((x 2) (y x)) y))", &env), "2");
}

#[test]
fn let_star_chains_multiple_dependent_bindings() {
    let env = env_with_stdlib();
    // a=2, b=a*a=4, c=b+1=5
    assert_eq!(
        eval_line("(let* ((a 2) (b (* a a)) (c (+ b 1))) c)", &env),
        "5"
    );
}

#[test]
fn let_star_rebinding_same_name_sees_previous_value() {
    let env = env_with_stdlib();
    // Second `x` is evaluated with the first `x` (= 1) in scope → 2.
    assert_eq!(eval_line("(let* ((x 1) (x (+ x 1))) x)", &env), "2");
}

// ---------------------------------------------------------------------------
// Single-frame consequences
// ---------------------------------------------------------------------------

#[test]
fn let_star_closure_captures_single_frame() {
    let env = env_with_stdlib();
    // A lambda bound later in the same LET* can see an earlier binding.
    assert_eq!(
        eval_line("(let* ((a 5) (f (lambda () a))) (funcall f))", &env),
        "5"
    );
}

#[test]
fn let_does_not_leak_bindings() {
    let env = env_with_stdlib();
    eval_line("(let ((leak1 1)) leak1)", &env);
    assert_eq!(eval_line("leak1", &env), "Error: Unbound variable: LEAK1");
}

#[test]
fn let_star_does_not_leak_bindings() {
    let env = env_with_stdlib();
    eval_line("(let* ((leak2 1)) leak2)", &env);
    assert_eq!(eval_line("leak2", &env), "Error: Unbound variable: LEAK2");
}

// ---------------------------------------------------------------------------
// Error handling
// ---------------------------------------------------------------------------

#[test]
fn let_requires_a_body() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("(let ((x 1)))", &env),
        "Error: let requires a binding list and at least one body form"
    );
}

#[test]
fn let_star_requires_a_body() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("(let* ((x 1)))", &env),
        "Error: let* requires a binding list and at least one body form"
    );
}

#[test]
fn let_binding_must_be_a_pair() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("(let ((x 1 2)) x)", &env),
        "Error: let binding must be a pair"
    );
}

#[test]
fn let_star_binding_name_must_be_symbol() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("(let* ((1 1)) 1)", &env),
        "Error: let* binding name must be a symbol"
    );
}

#[test]
fn let_with_empty_bindings_and_body_is_fine() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(let () 42)", &env), "42");
    assert_eq!(eval_line("(let* () (+ 1 2) 42)", &env), "42");
}

// ---------------------------------------------------------------------------
// Issue #224 regression: optimizer must not substitute into sibling LET inits
// ---------------------------------------------------------------------------

#[test]
fn optimizer_let_sibling_inits_see_outer_binding_not_local() {
    let env = env_with_stdlib();
    // x=1 globally; (let ((x 99) (y (+ x 10))) ...) — y's init must use outer x=1
    // not the sibling x=99.  The optimizer previously inlined x→99 into y's init,
    // producing y=109 and a final result of 208 instead of 110.
    eval_line("(def x 1)", &env);
    assert_eq!(
        eval_line("(let ((x 99) (y (+ x 10))) (+ y x))", &env),
        "110"
    );
    // Verify the optimizer agrees with direct eval (the regression)
    assert_eq!(
        eval_line(
            "(eval (optimize-form '(let ((x 99) (y (+ x 10))) (+ y x))) (the-environment))",
            &env
        ),
        "110"
    );
}
