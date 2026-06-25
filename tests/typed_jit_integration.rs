//! Integration tests: the typed subset (`deffun-typed`) landing in the running
//! language. Typed functions are defined at the "REPL" via `eval_line` and then
//! called from ordinary (untyped) Lisp code through the membrane.

use lamedh::environment::Environment;
use lamedh::eval_line;

#[test]
fn define_and_call_typed_function() {
    let env = Environment::new_with_builtins();
    eval_line(
        "(deffun-typed (fib int64) ((n int64)) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2)))))",
        &env,
    );
    assert_eq!(eval_line("(fib 10)", &env), "55");
    assert_eq!(eval_line("(fib 20)", &env), "6765");
}

#[test]
fn typed_function_callable_from_untyped_code() {
    let env = Environment::new_with_builtins();
    eval_line("(deffun-typed (sq int64) ((x int64)) (* x x))", &env);
    // Ordinary builtin `+` consuming the result of a typed call.
    assert_eq!(eval_line("(+ (sq 6) 1)", &env), "37");
    // Bind it like any other value and use it.
    eval_line("(def y (sq 9))", &env);
    assert_eq!(eval_line("y", &env), "81");
}

#[test]
fn float_typed_function_lands() {
    let env = Environment::new_with_builtins();
    eval_line(
        "(deffun-typed (avg float64) ((a float64) (b float64)) (/ (+ a b) 2.0))",
        &env,
    );
    assert_eq!(eval_line("(avg 3.0 5.0)", &env), "4.0");
}

#[test]
fn ill_typed_definition_is_rejected_at_the_repl() {
    let env = Environment::new_with_builtins();
    let out = eval_line("(deffun-typed (bad int64) ((x int64)) (if x 1 2))", &env);
    assert!(
        out.to_lowercase().contains("bool"),
        "expected a type error, got: {out}"
    );
    // And nothing was bound under the name.
    let call = eval_line("(bad 1)", &env);
    assert!(
        call.to_lowercase().contains("error") || call.to_lowercase().contains("unbound"),
        "got: {call}"
    );
}

#[test]
fn redefinition_at_the_repl_updates_behavior() {
    let env = Environment::new_with_builtins();
    eval_line("(deffun-typed (f int64) ((x int64)) (* x x))", &env);
    assert_eq!(eval_line("(f 5)", &env), "25");
    eval_line("(deffun-typed (f int64) ((x int64)) (* x (* x x)))", &env);
    assert_eq!(eval_line("(f 5)", &env), "125");
}

#[test]
fn typed_calls_typed_across_definitions() {
    let env = Environment::new_with_builtins();
    eval_line("(deffun-typed (dbl int64) ((x int64)) (* x 2))", &env);
    eval_line(
        "(deffun-typed (quad int64) ((x int64)) (dbl (dbl x)))",
        &env,
    );
    assert_eq!(eval_line("(quad 5)", &env), "20");
}

#[test]
fn cross_type_call_via_eval_line() {
    // A bool-returning typed predicate consumed by an int-returning typed
    // function, all through the REPL. (True *mutual* recursion needs a forward
    // declaration so the checker sees both signatures before either body; that is
    // exposed via `Jit::declare` in the Rust API and covered in the unit tests —
    // there is no surface syntax for a forward declaration yet.)
    let env = Environment::new_with_builtins();
    eval_line(
        "(deffun-typed (is-even bool) ((n int64)) (= (mod n 2) 0))",
        &env,
    );
    eval_line(
        "(deffun-typed (classify int64) ((n int64)) (if (is-even n) 0 1))",
        &env,
    );
    assert_eq!(eval_line("(classify 4)", &env), "0");
    assert_eq!(eval_line("(classify 7)", &env), "1");
}
