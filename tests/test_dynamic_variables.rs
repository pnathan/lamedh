mod test_helpers;
use lamedh::{eval_line, load_file};
use test_helpers::env_with_stdlib;

#[test]
fn test_defdynamic_basic() {
    let env = env_with_stdlib();

    // Test basic defdynamic
    let result = eval_line("(defdynamic *test* 42)", &env);
    assert_eq!(result, "*TEST*");

    let result = eval_line("*test*", &env);
    assert_eq!(result, "42");
}

#[test]
fn test_defvar_alias() {
    let env = env_with_stdlib();

    let result = eval_line("(defvar *var* 100)", &env);
    assert_eq!(result, "*VAR*");

    let result = eval_line("*var*", &env);
    assert_eq!(result, "100");
}

#[test]
fn test_dynamic_binding_with_let() {
    let env = env_with_stdlib();

    eval_line("(defdynamic *x* 'global)", &env);
    eval_line("(defun get-x () *x*)", &env);

    let result = eval_line("(get-x)", &env);
    assert_eq!(result, "GLOBAL");

    let result = eval_line("(let ((*x* 'local)) (get-x))", &env);
    assert_eq!(result, "LOCAL");

    let result = eval_line("(get-x)", &env);
    assert_eq!(result, "GLOBAL");
}

#[test]
fn test_nested_dynamic_bindings() {
    let env = env_with_stdlib();

    eval_line("(defdynamic *level* 0)", &env);
    eval_line("(defun get-level () *level*)", &env);

    let result = eval_line("(get-level)", &env);
    assert_eq!(result, "0");

    let result = eval_line("(let ((*level* 1)) (get-level))", &env);
    assert_eq!(result, "1");

    let result = eval_line("(let ((*level* 1)) (let ((*level* 2)) (get-level)))", &env);
    assert_eq!(result, "2");

    // After exiting LETs, should be back to global
    let result = eval_line("(get-level)", &env);
    assert_eq!(result, "0");
}

#[test]
fn test_setq_on_dynamic_variable() {
    let env = env_with_stdlib();

    eval_line("(defdynamic *counter* 0)", &env);
    eval_line("(defun inc () (setq *counter* (+ *counter* 1)))", &env);

    eval_line("(inc)", &env);
    let result = eval_line("*counter*", &env);
    assert_eq!(result, "1");

    // SETQ in LET should modify the local binding, not global
    // Note: LET only supports single body expression, use progn for multiple
    let result = eval_line("(let ((*counter* 10)) (progn (inc) (inc) *counter*))", &env);
    assert_eq!(result, "12");

    // Global should be unchanged
    let result = eval_line("*counter*", &env);
    assert_eq!(result, "1");
}

#[test]
fn test_lexical_vs_dynamic() {
    let env = env_with_stdlib();

    // Lexical variable
    eval_line("(def lex 'global)", &env);
    eval_line("(def get-lex (lambda () lex))", &env);

    // Dynamic variable
    eval_line("(defdynamic *dyn* 'global)", &env);
    eval_line("(def get-dyn (lambda () *dyn*))", &env);

    // Test from different binding context
    // Lexical should see original binding
    let result = eval_line("(get-lex)", &env);
    assert_eq!(result, "GLOBAL");

    // Dynamic should see current binding
    let result = eval_line("(let ((*dyn* 'local)) (get-dyn))", &env);
    assert_eq!(result, "LOCAL");

    // Lexical is unchanged by LET on same name (creates new binding)
    let result = eval_line("(let ((lex 'local)) (get-lex))", &env);
    assert_eq!(result, "GLOBAL");
}

#[test]
fn test_dynamic_across_multiple_functions() {
    let env = env_with_stdlib();

    eval_line("(defdynamic *context* 'global)", &env);
    eval_line("(defun inner () *context*)", &env);
    eval_line("(defun middle () (inner))", &env);
    eval_line("(defun outer () (let ((*context* 'outer-bound)) (middle)))", &env);

    let result = eval_line("(outer)", &env);
    assert_eq!(result, "OUTER-BOUND");

    let result = eval_line("(inner)", &env);
    assert_eq!(result, "GLOBAL");
}

#[test]
fn test_dynamic_with_recursive_function() {
    let env = env_with_stdlib();

    eval_line("(defdynamic *depth* 0)", &env);
    eval_line("(defun max-depth (n) (if (zerop n) *depth* (let ((*depth* (+ *depth* 1))) (max-depth (- n 1)))))", &env);

    let result = eval_line("(max-depth 5)", &env);
    assert_eq!(result, "5");

    // Global should be unchanged
    let result = eval_line("*depth*", &env);
    assert_eq!(result, "0");
}

#[test]
fn test_docstring_on_dynamic_variable() {
    let env = env_with_stdlib();

    eval_line("(defdynamic *documented* 42 \"This is a test variable\")", &env);

    let result = eval_line("(getp '*documented* \"docstring\")", &env);
    assert_eq!(result, "\"This is a test variable\"");
}

#[test]
fn test_multiple_dynamic_variables() {
    let env = env_with_stdlib();

    eval_line("(defdynamic *a* 1)", &env);
    eval_line("(defdynamic *b* 2)", &env);
    eval_line("(defun sum-ab () (+ *a* *b*))", &env);

    let result = eval_line("(sum-ab)", &env);
    assert_eq!(result, "3");

    let result = eval_line("(let ((*a* 10) (*b* 20)) (sum-ab))", &env);
    assert_eq!(result, "30");

    let result = eval_line("(sum-ab)", &env);
    assert_eq!(result, "3");
}

#[test]
fn test_dynamic_with_funcall() {
    let env = env_with_stdlib();

    eval_line("(defdynamic *multiplier* 2)", &env);
    eval_line("(defun double (x) (* x *multiplier*))", &env);

    let result = eval_line("(funcall #'double 5)", &env);
    assert_eq!(result, "10");

    let result = eval_line("(let ((*multiplier* 10)) (funcall #'double 5))", &env);
    assert_eq!(result, "50");
}

#[test]
fn test_run_comprehensive_lisp_tests() {
    let env = env_with_stdlib();

    // This will run all the tests in the Lisp file
    // The file should complete without errors
    let result = load_file("tests/dynamic_variables_test.lisp", &env);
    assert!(result.is_ok(), "Dynamic variables test file should run without errors: {:?}", result);
}
