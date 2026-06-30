/// Tests for the DEFUN purity checker pass (lib/11-optimizer-vau.lisp).
///
/// The checker annotates a function symbol's plist with "pure" = :PURE when
/// the body contains no SETQ/SET and no calls to known IO builtins.
mod test_helpers;
use lamedh::eval_line;
use test_helpers::env_with_stdlib;

// ── body-check-pure-p unit tests ────────────────────────────────────────────

#[test]
fn pure_arithmetic_form_is_pure() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(body-check-pure-p '(+ 1 2))", &env), "T");
}

#[test]
fn atom_is_pure() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(body-check-pure-p 'x)", &env), "T");
}

#[test]
fn nil_is_pure() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(body-check-pure-p nil)", &env), "T");
}

#[test]
fn quoted_form_is_pure() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("(body-check-pure-p '(quote (setq x 1)))", &env),
        "T"
    );
}

#[test]
fn setq_is_impure() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(body-check-pure-p '(setq x 1))", &env), "()");
}

#[test]
fn set_is_impure() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(body-check-pure-p '(set 'x 1))", &env), "()");
}

#[test]
fn nested_setq_is_impure() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("(body-check-pure-p '(if t (setq x 1) 0))", &env),
        "()"
    );
}

#[test]
fn io_builtin_print_is_impure() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(body-check-pure-p '(print x))", &env), "()");
}

#[test]
fn io_builtin_read_is_impure() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(body-check-pure-p '(read))", &env), "()");
}

#[test]
fn io_builtin_shell_is_impure() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(body-check-pure-p '(shell \"ls\"))", &env), "()");
}

#[test]
fn io_nested_in_cond_is_impure() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("(body-check-pure-p '(cond (t (print \"hi\"))))", &env),
        "()"
    );
}

// ── body-all-pure-p tests ────────────────────────────────────────────────────

#[test]
fn all_pure_forms_returns_t() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("(body-all-pure-p '((+ 1 2) (* x y) (car z)))", &env),
        "T"
    );
}

#[test]
fn mixed_forms_with_setq_returns_nil() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("(body-all-pure-p '((+ 1 2) (setq x 3)))", &env),
        "()"
    );
}

#[test]
fn empty_form_list_is_pure() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(body-all-pure-p nil)", &env), "T");
}

// ── defun-check-purity! integration tests ───────────────────────────────────

#[test]
fn pure_defun_annotated_with_pure_property() {
    let env = env_with_stdlib();
    eval_line("(defun sq (x) (* x x))", &env);
    assert_eq!(eval_line("(getp 'sq \"pure\")", &env), ":PURE");
}

#[test]
fn defun_with_docstring_pure_annotated() {
    let env = env_with_stdlib();
    eval_line("(defun sq2 (x) \"Square of x.\" (* x x))", &env);
    assert_eq!(eval_line("(getp 'sq2 \"pure\")", &env), ":PURE");
}

#[test]
fn impure_defun_not_annotated() {
    let env = env_with_stdlib();
    eval_line("(defun inc! (x) (setq x (+ x 1)) x)", &env);
    assert_eq!(eval_line("(getp 'inc! \"pure\")", &env), "()");
}

#[test]
fn defun_with_print_not_annotated() {
    let env = env_with_stdlib();
    eval_line("(defun greet (name) (print name))", &env);
    assert_eq!(eval_line("(getp 'greet \"pure\")", &env), "()");
}

#[test]
fn pure_defun_still_evaluates_correctly() {
    let env = env_with_stdlib();
    eval_line("(defun double (n) (* n 2))", &env);
    assert_eq!(eval_line("(double 7)", &env), "14");
}

#[test]
fn impure_defun_still_evaluates_correctly() {
    let env = env_with_stdlib();
    eval_line("(defun bump (n) (setq n (+ n 1)) n)", &env);
    assert_eq!(eval_line("(bump 3)", &env), "4");
}

// ── *io-builtin-set* accessibility ──────────────────────────────────────────

#[test]
fn io_builtin_set_is_a_list() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(listp *io-builtin-set*)", &env), "T");
}

#[test]
fn print_is_in_io_builtin_set() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("(member 'print *io-builtin-set*)", &env),
        "(PRINT PRIN1 PRINC TERPRI SHELL WRITE-FILE READ-FILE LOAD LOAD-FILE OPEN-FILE CLOSE-FILE FORMAT)"
    );
}

#[test]
fn pure_function_remains_callable_after_check() {
    let env = env_with_stdlib();
    eval_line("(defun add3 (a b c) (+ a b c))", &env);
    assert_eq!(eval_line("(add3 1 2 3)", &env), "6");
    assert_eq!(eval_line("(getp 'add3 \"pure\")", &env), ":PURE");
}

#[test]
fn redefined_impure_clears_pure_property() {
    let env = env_with_stdlib();
    // First define as pure
    eval_line("(defun toggled (x) (* x 2))", &env);
    assert_eq!(eval_line("(getp 'toggled \"pure\")", &env), ":PURE");
    // Redefine as impure: checker should clear the "pure" property
    eval_line("(defun toggled (x) (print x))", &env);
    assert_eq!(eval_line("(getp 'toggled \"pure\")", &env), "()");
}
