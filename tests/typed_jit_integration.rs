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
    // A bool-returning typed predicate consumed by an int-returning typed function.
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

#[test]
fn mutual_recursion_via_declare_typed() {
    // Forward-declare both signatures, then define both bodies — true mutual
    // recursion at the REPL through the new `declare-typed` surface syntax.
    let env = Environment::new_with_builtins();
    eval_line("(declare-typed (even? bool) ((n int64)))", &env);
    eval_line("(declare-typed (odd? bool) ((n int64)))", &env);
    eval_line(
        "(deffun-typed (even? bool) ((n int64)) (if (= n 0) true (odd? (- n 1))))",
        &env,
    );
    eval_line(
        "(deffun-typed (odd? bool) ((n int64)) (if (= n 0) false (even? (- n 1))))",
        &env,
    );
    assert_eq!(eval_line("(even? 10)", &env), "T");
    assert_eq!(eval_line("(odd? 10)", &env), "()");
    assert_eq!(eval_line("(even? 7)", &env), "()");
    assert_eq!(eval_line("(odd? 7)", &env), "T");
}

#[test]
fn calling_declared_but_undefined_errors_cleanly() {
    // Declaring without defining must not panic when called — it returns an error.
    let env = Environment::new_with_builtins();
    eval_line("(declare-typed (ghost int64) ((n int64)))", &env);
    let out = eval_line("(ghost 1)", &env);
    assert!(
        out.to_lowercase().contains("not defined") || out.to_lowercase().contains("error"),
        "got: {out}"
    );
}

#[test]
fn typed_bool_returns_t_and_nil_usable_untyped() {
    // Typed predicates return real Lisp booleans (T / NIL) usable as `if`
    // conditions in ordinary code, and accept Lisp truthiness as bool arguments.
    let env = Environment::new_with_builtins();
    eval_line("(deffun-typed (big? bool) ((n int64)) (> n 100))", &env);
    assert_eq!(eval_line("(big? 200)", &env), "T");
    assert_eq!(eval_line("(big? 5)", &env), "()");
    assert_eq!(eval_line("(if (big? 200) 'yes 'no)", &env), "YES");
    // A bool parameter accepts T / NIL via Lisp truthiness.
    eval_line("(deffun-typed (negate bool) ((p bool)) (not p))", &env);
    assert_eq!(eval_line("(negate t)", &env), "()");
    assert_eq!(eval_line("(negate nil)", &env), "T");
}

#[test]
fn typed_string_processing_lands_at_repl() {
    // A string is (array char): the typed function indexes it natively and the
    // membrane passes a Lisp string straight in.
    let env = Environment::new_with_builtins();
    eval_line(
        "(deffun-typed (first-code int64) ((s (array char))) (char-code (fetch s 0)))",
        &env,
    );
    assert_eq!(eval_line("(first-code \"ABC\")", &env), "65");
    eval_line(
        "(deffun-typed (slen int64) ((s (array char))) (array-length s))",
        &env,
    );
    assert_eq!(eval_line("(slen \"hello\")", &env), "5");
}

#[test]
fn typed_char_arguments_reject_out_of_range_numbers() {
    let env = Environment::new_with_builtins();
    eval_line("(deffun-typed (idc char) ((c char)) c)", &env);

    assert_eq!(eval_line("(char-code (idc 'A'))", &env), "65");
    assert_eq!(eval_line("(char-code (idc 65))", &env), "65");

    for expr in ["(idc 256)", "(idc -1)"] {
        let out = eval_line(expr, &env);
        assert!(
            out.starts_with("Error:"),
            "expected {expr} to reject out-of-range char input, got: {out}"
        );
    }
}

#[test]
fn typed_int_array_kernel_lands_at_repl() {
    // `with_stdlib` for `list->array`; the kernel itself is typed/native.
    let env = Environment::with_stdlib();
    eval_line(
        "(deffun-typed (suml int64) ((a (array int64)) (i int64)) \
           (if (= i (array-length a)) 0 (+ (fetch a i) (suml a (+ i 1)))))",
        &env,
    );
    eval_line(
        "(deffun-typed (sum int64) ((a (array int64))) (suml a 0))",
        &env,
    );
    // Pass an untyped Lisp array through the membrane.
    assert_eq!(
        eval_line("(sum (list->array (list 1 2 3 4 5)))", &env),
        "15"
    );
}

#[test]
fn typed_struct_lands_at_repl() {
    let env = Environment::new_with_builtins();
    eval_line("(defstruct-typed Point (x int64) (y int64))", &env);
    assert_eq!(eval_line("(point-x (make-point 3 4))", &env), "3");
    assert_eq!(eval_line("(point-y (make-point 3 4))", &env), "4");
    // A struct flows through the typed membrane with its nominal type intact.
    eval_line(
        "(deffun-typed (manhattan int64) ((p Point)) (+ (point-x p) (point-y p)))",
        &env,
    );
    assert_eq!(eval_line("(manhattan (make-point 3 4))", &env), "7");
}

#[test]
fn typed_struct_accessors_reject_plain_arrays() {
    let env = Environment::new_with_builtins();
    eval_line("(defstruct-typed Point (x int64) (y int64))", &env);
    let out = eval_line(
        "(progn (def forged (array 2)) (store forged 0 10) (store forged 1 20) (point-x forged))",
        &env,
    );
    assert!(
        out.starts_with("Error:"),
        "typed struct accessor accepted a plain array: {out}"
    );
}

#[test]
fn jit_optimize_makes_untyped_defun_run_typed_with_fallback() {
    // A plain `defun` (no annotations); `(jit-optimize ...)` infers int64->int64
    // and installs the native fast path transparently. HM fired under the hood.
    let env = Environment::with_stdlib();
    eval_line("(defun inc (n) (+ n 1))", &env);
    assert_eq!(eval_line("(inc 41)", &env), "42");
    eval_line("(jit-optimize inc)", &env);
    // Same answer, now via the native typed edition.
    assert_eq!(eval_line("(inc 41)", &env), "42");

    // Ergonomic wrap form: optimize at definition time.
    eval_line(
        "(jit-optimize (defun fib (n) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2))))))",
        &env,
    );
    assert_eq!(eval_line("(fib 20)", &env), "6765");
}

#[test]
fn jit_optimize_is_noop_for_untypeable_functions() {
    // A list-processing function cannot be typed; `(jit-optimize ...)` leaves it
    // working exactly as before (silent fallback, no error).
    let env = Environment::with_stdlib();
    eval_line(
        "(defun mylen (xs) (if (null xs) 0 (+ 1 (mylen (cdr xs)))))",
        &env,
    );
    assert_eq!(eval_line("(mylen (list 1 2 3))", &env), "3");
    eval_line("(jit-optimize mylen)", &env); // no-op: cons/null are untyped
    assert_eq!(eval_line("(mylen (list 1 2 3))", &env), "3");
}

#[test]
fn jit_optimized_function_falls_back_on_non_matching_args() {
    // After optimizing for int64, a float argument doesn't fit the inferred type
    // and transparently uses the dynamic definition instead of erroring.
    let env = Environment::with_stdlib();
    eval_line("(jit-optimize (defun dbl (n) (+ n n)))", &env);
    assert_eq!(eval_line("(dbl 21)", &env), "42"); // typed fast path
    assert_eq!(eval_line("(dbl 2.5)", &env), "5.0"); // dynamic fallback
}

// --- non-compiled type checker (#162) --------------------------------------

#[test]
fn check_type_reports_polymorphic_identity() {
    let env = Environment::with_stdlib();
    let out = eval_line("(check-type (defun id (x) x))", &env);
    // ∀a. a -> a
    assert!(
        out.contains("forall") && out.contains("(-> (a) a)"),
        "got: {out}"
    );
}

#[test]
fn check_type_reports_concrete_numeric() {
    let env = Environment::with_stdlib();
    eval_line("(defun inc (n) (+ n 1))", &env);
    let out = eval_line("(check-type inc)", &env);
    assert!(out.contains("(-> (int64) int64)"), "got: {out}");
}

#[test]
fn check_type_infers_list_function() {
    // A recursive list sum: xs is inferred to be a (list int64) and returns int64
    // — a *checkable* type that is not compileable, caught for free.
    let env = Environment::with_stdlib();
    eval_line(
        "(defun lsum (xs) (if (null xs) 0 (+ (car xs) (lsum (cdr xs)))))",
        &env,
    );
    let out = eval_line("(check-type lsum)", &env);
    assert!(out.contains("(-> ((list int64)) int64)"), "got: {out}");
}

#[test]
fn check_type_catches_a_type_error() {
    // Mixing element types in a list is a genuine type clash.
    let env = Environment::with_stdlib();
    eval_line("(defun bad (x) (list 1 x (+ x x) nil))", &env);
    let out = eval_line("(check-type bad)", &env);
    assert!(out.to_lowercase().contains("type error"), "got: {out}");
}

#[test]
fn check_type_is_gradual_at_the_untyped_frontier() {
    // `print` is an unknown/untyped callee: it degrades to `any` rather than
    // failing the check, so the function still type-checks.
    let env = Environment::with_stdlib();
    eval_line("(defun greet (n) (cons n (cons n nil)))", &env);
    let out = eval_line("(check-type greet)", &env);
    // n is unconstrained → polymorphic list builder.
    assert!(out.contains("forall") && out.contains("list"), "got: {out}");
}

// --- stage 4: unified check+compile reporting; stage 5: control-flow ---------

#[test]
fn jit_optimize_reports_native_checked_and_type_error() {
    let env = Environment::with_stdlib();
    // Compileable -> native.
    let n = eval_line("(jit-optimize (defun inc (n) (+ n 1)))", &env);
    assert!(n.contains("int64") && n.contains("native"), "got: {n}");
    // Well-typed but not compileable (list) -> checked, dynamic.
    let c = eval_line(
        "(jit-optimize (defun lsum (xs) (if (null xs) 0 (+ (car xs) (lsum (cdr xs))))))",
        &env,
    );
    assert!(
        c.contains("list int64") && c.contains("checked"),
        "got: {c}"
    );
    assert_eq!(eval_line("(lsum (list 1 2 3))", &env), "6"); // still runs dynamically
    // Genuine type error -> reported, function still defined dynamically.
    let e = eval_line(
        "(jit-optimize (defun clash (x) (+ (car x) (array-length x))))",
        &env,
    );
    assert!(e.to_lowercase().contains("type error"), "got: {e}");
}

#[test]
fn check_type_handles_cond_and_truthiness() {
    let env = Environment::with_stdlib();
    // `cond` clauses unify; `if`/list truthiness allowed; classic recursive list len.
    eval_line(
        "(defun llen (xs) (cond ((null xs) 0) (t (+ 1 (llen (cdr xs))))))",
        &env,
    );
    let out = eval_line("(check-type llen)", &env);
    assert!(
        out.contains("(-> ((list a)) int64)") || out.contains("(list"),
        "got: {out}"
    );
    // truthiness: (if xs (car xs) 0) — xs used as a condition directly.
    eval_line("(defun head0 (xs) (if xs (car xs) 0))", &env);
    let h = eval_line("(check-type head0)", &env);
    assert!(h.contains("(-> ((list int64)) int64)"), "got: {h}");
}
