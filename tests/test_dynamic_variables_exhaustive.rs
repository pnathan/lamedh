//! Exhaustive tests for dynamic variables (special variables) in Lamedh
//!
//! Dynamic scoping is often misunderstood. These tests thoroughly verify:
//! 1. Basic dynamic vs lexical scoping differences
//! 2. SETQ behavior with dynamic variables
//! 3. Nested binding behavior (stack semantics)
//! 4. Interaction with closures
//! 5. Interaction with higher-order functions
//! 6. Edge cases and error conditions

mod test_helpers;
use lamedh::{eval_line, load_file};
use test_helpers::env_with_stdlib;

// =============================================================================
// PART 1: FUNDAMENTAL DIFFERENCE TESTS
// =============================================================================

#[test]
fn test_lexical_vs_dynamic_basic() {
    let env = env_with_stdlib();

    // Setup: both lexical and dynamic variables
    eval_line("(def lex-var 'lexical-global)", &env);
    eval_line("(defdynamic *dyn-var* 'dynamic-global)", &env);

    // Create functions that reference these variables
    eval_line("(def get-lex (lambda () lex-var))", &env);
    eval_line("(def get-dyn (lambda () *dyn-var*))", &env);

    // From global context, both see global values
    assert_eq!(eval_line("(get-lex)", &env), "LEXICAL-GLOBAL");
    assert_eq!(eval_line("(get-dyn)", &env), "DYNAMIC-GLOBAL");

    // Shadow lexical - function still sees original (lexical scoping)
    eval_line("(def test-lex (lambda () (progn (def lex-var 'lexical-local) (get-lex))))", &env);
    assert_eq!(eval_line("(test-lex)", &env), "LEXICAL-GLOBAL");

    // Shadow dynamic - function sees new binding (dynamic scoping)
    assert_eq!(
        eval_line("(let ((*dyn-var* 'dynamic-local)) (get-dyn))", &env),
        "DYNAMIC-LOCAL"
    );

    // After LET, back to global
    assert_eq!(eval_line("(get-dyn)", &env), "DYNAMIC-GLOBAL");
}

#[test]
fn test_dynamic_follows_call_stack() {
    let env = env_with_stdlib();

    eval_line("(defdynamic *marker* 'global)", &env);

    // Three levels of function calls
    eval_line("(defun level-3 () *marker*)", &env);
    eval_line("(defun level-2 () (level-3))", &env);
    eval_line("(defun level-1 () (let ((*marker* 'from-level-1)) (level-2)))", &env);

    // level-3 sees binding from level-1, not global
    assert_eq!(eval_line("(level-1)", &env), "FROM-LEVEL-1");

    // Direct call sees global
    assert_eq!(eval_line("(level-3)", &env), "GLOBAL");
}

#[test]
fn test_dynamic_through_multiple_callers() {
    let env = env_with_stdlib();

    eval_line("(defdynamic *ctx* 'default)", &env);
    eval_line("(defun use-ctx () *ctx*)", &env);

    // Different callers set different contexts
    eval_line("(defun caller-a () (let ((*ctx* 'a)) (use-ctx)))", &env);
    eval_line("(defun caller-b () (let ((*ctx* 'b)) (use-ctx)))", &env);
    eval_line("(defun caller-c () (use-ctx))", &env); // Uses default

    assert_eq!(eval_line("(caller-a)", &env), "A");
    assert_eq!(eval_line("(caller-b)", &env), "B");
    assert_eq!(eval_line("(caller-c)", &env), "DEFAULT");
}

// =============================================================================
// PART 2: SETQ BEHAVIOR TESTS
// =============================================================================

#[test]
fn test_setq_modifies_dynamic_binding() {
    let env = env_with_stdlib();

    eval_line("(defdynamic *counter* 0)", &env);
    eval_line("(defun inc () (setq *counter* (+ *counter* 1)))", &env);

    // Modify global
    eval_line("(inc)", &env);
    assert_eq!(eval_line("*counter*", &env), "1");

    // Modify local binding
    let result = eval_line("(let ((*counter* 100)) (progn (inc) *counter*))", &env);
    assert_eq!(result, "101");

    // Global unchanged
    assert_eq!(eval_line("*counter*", &env), "1");
}

#[test]
fn test_setq_in_deeply_nested_call() {
    let env = env_with_stdlib();

    eval_line("(defdynamic *deep-val* 0)", &env);
    eval_line("(defun modify-deep () (setq *deep-val* 999))", &env);
    eval_line("(defun middle () (modify-deep))", &env);
    eval_line("(defun outer () (let ((*deep-val* 1)) (progn (middle) *deep-val*)))", &env);

    // modify-deep changes the binding from outer's LET
    assert_eq!(eval_line("(outer)", &env), "999");

    // Global still 0
    assert_eq!(eval_line("*deep-val*", &env), "0");
}

#[test]
fn test_setq_creates_if_not_bound() {
    let env = env_with_stdlib();

    // Define as dynamic but don't set initial value through normal means
    eval_line("(defdynamic *unset* nil)", &env);

    // SETQ should update the global binding
    eval_line("(setq *unset* 'now-set)", &env);
    assert_eq!(eval_line("*unset*", &env), "NOW-SET");
}

// =============================================================================
// PART 3: NESTED BINDING TESTS (Stack Behavior)
// =============================================================================

#[test]
fn test_nested_bindings_stack() {
    let env = env_with_stdlib();

    eval_line("(defdynamic *level* 0)", &env);
    eval_line("(defun get-level () *level*)", &env);

    // Nest three levels
    let result = eval_line(
        "(let ((*level* 1))
           (progn
             (let ((*level* 2))
               (progn
                 (let ((*level* 3))
                   (get-level))))))",
        &env,
    );
    assert_eq!(result, "3");

    // After all LETs, back to 0
    assert_eq!(eval_line("(get-level)", &env), "0");
}

#[test]
fn test_binding_restored_after_exception_like_behavior() {
    let env = env_with_stdlib();

    eval_line("(defdynamic *state* 'initial)", &env);
    eval_line("(defun get-state () *state*)", &env);

    // Even if we don't have real exceptions, verify LET properly restores
    eval_line(
        "(let ((*state* 'temporary))
           (progn
             (get-state)
             nil))",
        &env,
    );

    assert_eq!(eval_line("(get-state)", &env), "INITIAL");
}

// =============================================================================
// PART 4: RECURSION TESTS
// =============================================================================

#[test]
fn test_recursive_with_dynamic_counter() {
    let env = env_with_stdlib();

    eval_line("(defdynamic *rec-depth* 0)", &env);
    eval_line(
        "(defun count-depth (n)
           (if (zerop n)
               *rec-depth*
               (let ((*rec-depth* (+ *rec-depth* 1)))
                 (count-depth (- n 1)))))",
        &env,
    );

    assert_eq!(eval_line("(count-depth 5)", &env), "5");
    assert_eq!(eval_line("(count-depth 10)", &env), "10");
    assert_eq!(eval_line("*rec-depth*", &env), "0"); // Unchanged
}

#[test]
fn test_recursive_accumulator() {
    let env = env_with_stdlib();

    eval_line("(defdynamic *sum* 0)", &env);
    eval_line(
        "(defun sum-to (n)
           (if (zerop n)
               *sum*
               (let ((*sum* (+ *sum* n)))
                 (sum-to (- n 1)))))",
        &env,
    );

    // 5 + 4 + 3 + 2 + 1 = 15
    assert_eq!(eval_line("(sum-to 5)", &env), "15");
    assert_eq!(eval_line("*sum*", &env), "0");
}

// =============================================================================
// PART 5: CLOSURE INTERACTION TESTS
// =============================================================================

#[test]
fn test_closure_does_not_capture_dynamic() {
    let env = env_with_stdlib();

    eval_line("(defdynamic *not-captured* 'global)", &env);

    // Create closure inside a LET
    eval_line(
        "(def make-fn
           (lambda ()
             (let ((*not-captured* 'at-creation))
               (lambda () *not-captured*))))",
        &env,
    );

    eval_line("(def my-fn (make-fn))", &env);

    // The closure sees current binding, not creation-time binding
    assert_eq!(eval_line("(funcall my-fn)", &env), "GLOBAL");

    // With different binding at call time
    assert_eq!(
        eval_line("(let ((*not-captured* 'at-call)) (funcall my-fn))", &env),
        "AT-CALL"
    );
}

#[test]
fn test_closure_with_mixed_variables() {
    let env = env_with_stdlib();

    eval_line("(def lex-val 'lex-global)", &env);
    eval_line("(defdynamic *dyn-val* 'dyn-global)", &env);

    // Closure captures lexical, not dynamic
    eval_line(
        "(def make-mixed
           (lambda (lex-param)
             (lambda () (list lex-param lex-val *dyn-val*))))",
        &env,
    );

    eval_line("(def mixed-fn (make-mixed 'param-value))", &env);

    // lex-param is captured, *dyn-val* is looked up dynamically
    let result = eval_line("(funcall mixed-fn)", &env);
    assert_eq!(result, "(PARAM-VALUE LEX-GLOBAL DYN-GLOBAL)");

    // Change dynamic, lexical stays same
    let result = eval_line(
        "(let ((*dyn-val* 'dyn-changed))
           (funcall mixed-fn))",
        &env,
    );
    assert_eq!(result, "(PARAM-VALUE LEX-GLOBAL DYN-CHANGED)");
}

// =============================================================================
// PART 6: HIGHER-ORDER FUNCTION TESTS
// =============================================================================

#[test]
fn test_funcall_with_dynamic() {
    let env = env_with_stdlib();

    eval_line("(defdynamic *mult* 1)", &env);
    eval_line("(defun scale (x) (* x *mult*))", &env);

    assert_eq!(eval_line("(funcall #'scale 5)", &env), "5");
    assert_eq!(
        eval_line("(let ((*mult* 10)) (funcall #'scale 5))", &env),
        "50"
    );
}

#[test]
fn test_apply_with_dynamic() {
    let env = env_with_stdlib();

    eval_line("(defdynamic *offset* 0)", &env);
    eval_line("(defun add-offset (x) (+ x *offset*))", &env);

    assert_eq!(eval_line("(apply #'add-offset '(100))", &env), "100");
    assert_eq!(
        eval_line("(let ((*offset* 50)) (apply #'add-offset '(100)))", &env),
        "150"
    );
}

#[test]
fn test_mapcar_with_dynamic() {
    let env = env_with_stdlib();

    eval_line("(defdynamic *factor* 1)", &env);
    eval_line("(defun mul-factor (x) (* x *factor*))", &env);

    // Note: lamedh mapcar is (mapcar list function), not (mapcar function list)
    assert_eq!(
        eval_line("(mapcar '(1 2 3) #'mul-factor)", &env),
        "(1 2 3)"
    );
    assert_eq!(
        eval_line("(let ((*factor* 10)) (mapcar '(1 2 3) #'mul-factor))", &env),
        "(10 20 30)"
    );
}

// =============================================================================
// PART 7: MULTIPLE DYNAMIC VARIABLES
// =============================================================================

#[test]
fn test_multiple_dynamic_vars_independent() {
    let env = env_with_stdlib();

    eval_line("(defdynamic *a* 1)", &env);
    eval_line("(defdynamic *b* 2)", &env);
    eval_line("(defdynamic *c* 3)", &env);

    eval_line("(defun sum-abc () (+ *a* (+ *b* *c*)))", &env);

    assert_eq!(eval_line("(sum-abc)", &env), "6");

    // Change just one
    assert_eq!(eval_line("(let ((*a* 100)) (sum-abc))", &env), "105");

    // Change multiple
    assert_eq!(
        eval_line("(let ((*a* 10) (*b* 20) (*c* 30)) (sum-abc))", &env),
        "60"
    );
}

#[test]
fn test_nested_different_variables() {
    let env = env_with_stdlib();

    eval_line("(defdynamic *outer-var* 'outer)", &env);
    eval_line("(defdynamic *inner-var* 'inner)", &env);

    eval_line("(defun show-both () (list *outer-var* *inner-var*))", &env);

    let result = eval_line(
        "(let ((*outer-var* 'outer-changed))
           (let ((*inner-var* 'inner-changed))
             (show-both)))",
        &env,
    );
    assert_eq!(result, "(OUTER-CHANGED INNER-CHANGED)");
}

// =============================================================================
// PART 8: EDGE CASES
// =============================================================================

#[test]
fn test_redefine_dynamic_variable() {
    let env = env_with_stdlib();

    eval_line("(defdynamic *redef* 'first)", &env);
    assert_eq!(eval_line("*redef*", &env), "FIRST");

    // Redefining should update the value
    eval_line("(defdynamic *redef* 'second)", &env);
    assert_eq!(eval_line("*redef*", &env), "SECOND");
}

#[test]
fn test_dynamic_nil_binding() {
    let env = env_with_stdlib();

    eval_line("(defdynamic *nilable* 'not-nil)", &env);
    eval_line("(defun is-nil () (null *nilable*))", &env);

    // null returns () for non-nil, T for nil
    let result = eval_line("(is-nil)", &env);
    assert!(result == "NIL" || result == "()", "Expected NIL or (), got {}", result);

    let result = eval_line("(let ((*nilable* nil)) (is-nil))", &env);
    assert_eq!(result, "T");
}

#[test]
fn test_dynamic_numeric_values() {
    let env = env_with_stdlib();

    eval_line("(defdynamic *num* 0)", &env);

    // Arithmetic operations
    let result = eval_line(
        "(let ((*num* 10))
           (let ((*num* (+ *num* 5)))
             (let ((*num* (* *num* 2)))
               *num*)))",
        &env,
    );
    assert_eq!(result, "30"); // (10 + 5) * 2 = 30
}

#[test]
fn test_dynamic_with_list_values() {
    let env = env_with_stdlib();

    eval_line("(defdynamic *items* nil)", &env);

    let result = eval_line(
        "(let ((*items* '(a)))
           (let ((*items* (cons 'b *items*)))
             (let ((*items* (cons 'c *items*)))
               *items*)))",
        &env,
    );
    assert_eq!(result, "(C B A)");
}

// =============================================================================
// PART 9: DOCSTRING TESTS
// =============================================================================

#[test]
fn test_dynamic_docstring() {
    let env = env_with_stdlib();

    eval_line("(defdynamic *doc-test* 42 \"This is documentation\")", &env);

    assert_eq!(eval_line("*doc-test*", &env), "42");
    assert_eq!(
        eval_line("(getp '*doc-test* \"docstring\")", &env),
        "\"This is documentation\""
    );
}

#[test]
fn test_dynamic_without_docstring() {
    let env = env_with_stdlib();

    eval_line("(defdynamic *no-doc* 'value)", &env);

    // No docstring should return NIL (printed as () in lamedh)
    let result = eval_line("(getp '*no-doc* \"docstring\")", &env);
    assert!(result == "NIL" || result == "()", "Expected NIL or (), got {}", result);
}

// =============================================================================
// PART 10: COMPREHENSIVE INTEGRATION TESTS
// =============================================================================

#[test]
fn test_run_exhaustive_lisp_tests() {
    let env = env_with_stdlib();

    let result = load_file("tests/dynamic_variables_exhaustive_test.lisp", &env);
    assert!(
        result.is_ok(),
        "Exhaustive test file should run without errors: {:?}",
        result
    );
}

#[test]
fn test_run_example_programs() {
    let env = env_with_stdlib();

    let result = load_file("examples/dynamic_variables_examples.lisp", &env);
    assert!(
        result.is_ok(),
        "Example programs should run without errors: {:?}",
        result
    );
}

// =============================================================================
// PART 11: GOTCHA/PITFALL TESTS
// =============================================================================

#[test]
fn test_gotcha_function_defined_in_let() {
    let env = env_with_stdlib();

    eval_line("(defdynamic *gotcha* 'outer)", &env);

    // Define function globally first
    eval_line("(defun gotcha-fn () *gotcha*)", &env);

    // When called from global context, sees outer binding
    assert_eq!(eval_line("(gotcha-fn)", &env), "OUTER");

    // When called inside a LET with different binding, sees that binding
    // This is the key difference from lexical scoping!
    assert_eq!(
        eval_line("(let ((*gotcha* 'inner)) (gotcha-fn))", &env),
        "INNER"
    );

    // Back outside the LET, sees outer again
    assert_eq!(eval_line("(gotcha-fn)", &env), "OUTER");
}

#[test]
fn test_gotcha_lambda_vs_defun() {
    let env = env_with_stdlib();

    eval_line("(defdynamic *capture-test* 'global)", &env);

    // Lambda created and called inside LET
    let result = eval_line(
        "(let ((*capture-test* 'local))
           ((lambda () *capture-test*)))",
        &env,
    );
    assert_eq!(result, "LOCAL"); // Lambda called inside LET sees LET binding

    // Lambda created inside LET, called outside
    eval_line(
        "(def delayed-fn
           (let ((*capture-test* 'at-creation))
             (lambda () *capture-test*)))",
        &env,
    );
    assert_eq!(eval_line("(funcall delayed-fn)", &env), "GLOBAL");
}

#[test]
fn test_gotcha_mutual_recursion() {
    let env = env_with_stdlib();

    eval_line("(defdynamic *ping-pong* 0)", &env);

    eval_line(
        "(defun ping (n)
           (if (zerop n)
               *ping-pong*
               (let ((*ping-pong* (+ *ping-pong* 1)))
                 (pong (- n 1)))))",
        &env,
    );
    eval_line(
        "(defun pong (n)
           (if (zerop n)
               *ping-pong*
               (let ((*ping-pong* (+ *ping-pong* 10)))
                 (ping (- n 1)))))",
        &env,
    );

    // ping adds 1, pong adds 10
    // ping(3) -> pong(2) -> ping(1) -> pong(0)
    // Values: 0 -> 1 -> 11 -> 12 -> return 12
    assert_eq!(eval_line("(ping 3)", &env), "12");
}
