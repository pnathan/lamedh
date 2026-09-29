//! Issue #521: errors about non-callable values (and other "got X" errors)
//! render the value with the Lisp printer, never Rust `Debug` output such as
//! `Symbol("COMPILED-P")` or `Number(5)`, and say why an operative or a bare
//! symbol cannot be applied.

mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

#[test]
fn quoted_symbol_naming_a_builtin_points_at_sharp_quote() {
    let e = env_with_stdlib();
    assert_eq!(
        eval_line("(mapcar 'compiled-p '(car))", &e),
        "Error: Not a function: the symbol COMPILED-P (pass the function it names with #'COMPILED-P)"
    );
    // The suggested spelling works.
    assert_eq!(eval_line("(mapcar #'compiled-p '(car))", &e), "(())");
}

#[test]
fn quoted_symbol_naming_a_lambda_points_at_sharp_quote() {
    let e = env_with_stdlib();
    eval_line("(defun issue-521-id (x) x)", &e);
    assert_eq!(
        eval_line("(mapcar 'issue-521-id '(1))", &e),
        "Error: Not a function: the symbol ISSUE-521-ID (pass the function it names with #'ISSUE-521-ID)"
    );
}

#[test]
fn quoted_symbol_naming_nothing() {
    let e = env_with_stdlib();
    assert_eq!(
        eval_line("(mapcar 'issue-521-unbound '(1))", &e),
        "Error: Not a function: the symbol ISSUE-521-UNBOUND"
    );
}

#[test]
fn non_callable_values_are_lisp_printed() {
    let e = env_with_stdlib();
    assert_eq!(eval_line("(5 1)", &e), "Error: Not a function: 5");
    assert_eq!(eval_line("(funcall 5 1)", &e), "Error: Not a function: 5");
    assert_eq!(
        eval_line("(mapcar \"s\" '(1))", &e),
        "Error: Not a function: \"s\""
    );
    assert_eq!(
        eval_line("(funcall '(1 2) 1)", &e),
        "Error: Not a function: (1 2)"
    );
}

#[test]
fn macro_is_not_applicable_and_says_so() {
    let e = env_with_stdlib();
    eval_line("(defmacro issue-521-m (x) x)", &e);
    assert_eq!(
        eval_line("(funcall #'issue-521-m 1)", &e),
        "Error: Not a function: <macro> (a macro transforms unevaluated code and cannot be applied)"
    );
    assert_eq!(
        eval_line("(mapcar 'issue-521-m '(1))", &e),
        "Error: Not a function: the symbol ISSUE-521-M, which names a macro; a macro cannot be applied"
    );
}

#[test]
fn fexpr_is_not_applicable_and_says_so() {
    let e = env_with_stdlib();
    eval_line("(defexpr issue-521-fx (x) x)", &e);
    assert_eq!(
        eval_line("(mapcar #'issue-521-fx '(1))", &e),
        "Error: Not a function: <fexpr> (an operative receives unevaluated operands and cannot be applied)"
    );
    assert_eq!(
        eval_line("(mapcar 'issue-521-fx '(1))", &e),
        "Error: Not a function: the symbol ISSUE-521-FX, which names an operative; an operative cannot be applied"
    );
}

#[test]
fn introspection_type_errors_are_lisp_printed() {
    let e = env_with_stdlib();
    assert_eq!(
        eval_line("(compiled-p 5)", &e),
        "Error: compiled-p requires a symbol, got 5"
    );
    assert_eq!(
        eval_line("(signature \"car\")", &e),
        "Error: signature requires a symbol, got \"car\""
    );
    assert_eq!(
        eval_line("(read-string 'abc)", &e),
        "Error: read-string requires a string, got ABC"
    );
    assert_eq!(
        eval_line("(for (i 0 'a) i)", &e),
        "Error: for end must be an integer, got A"
    );
}

#[test]
fn no_debug_formatting_leaks() {
    let e = env_with_stdlib();
    for src in [
        "(mapcar 'compiled-p '(car))",
        "(funcall 5 1)",
        "(funcall '(1 2) 1)",
        "(see-type 5)",
        "(disassemble '(a))",
    ] {
        let out = eval_line(src, &e);
        for leak in ["Symbol(", "Number(", "Cons(", "String(", "Nil"] {
            assert!(!out.contains(leak), "{src} leaked Debug text: {out}");
        }
    }
}
