//! EQ on cons cells: pointer/identity comparison (issue #454).
//!
//! Historically `BuiltinFunc::Eq` made `EQ` unconditionally `NIL` whenever
//! either argument was a cons cell, including comparing a cons cell against
//! itself. That was a stronger restriction than the Lisp 1.5 manual actually
//! requires (it only leaves `EQ` on non-atoms *unspecified*, not `false`),
//! and diverged from ordinary Lisp practice where `(eq x x)` is true for the
//! same cons cell. This suite pins down the corrected behavior: `EQ` on cons
//! cells is pointer identity of the underlying `Shared`/`Rc` allocation, and
//! every other `EQ` rule (Part IV of KERNEL.md) is unchanged.

use lamedh::{self, Shared, environment::Environment, eval_line};

fn env_with_stdlib() -> Shared<Environment> {
    Environment::with_stdlib()
}

#[test]
fn eq_same_cons_cell_is_true() {
    let env = env_with_stdlib();
    // The same actual cons cell, referenced through the same binding, is EQ
    // to itself.
    assert_eq!(eval_line("(let ((x (cons 1 2))) (eq x x))", &env), "T");
    assert_eq!(eval_line("(let ((x (list 1 2 3))) (eq x x))", &env), "T");
    // Aliasing via a second binding (same Rc allocation, refcount bumped)
    // must also compare EQ.
    assert_eq!(
        eval_line("(let* ((x (cons 1 2)) (y x)) (eq x y))", &env),
        "T"
    );
    // EQ on a sub-structure reached two different ways still resolves to
    // the same shared allocation.
    assert_eq!(
        eval_line("(let ((x (cons 1 2))) (eq (cdr (cons 0 x)) x))", &env),
        "T"
    );
}

#[test]
fn eq_distinct_but_structurally_equal_cons_is_false() {
    let env = env_with_stdlib();
    // Two freshly allocated, structurally-identical cons cells are two
    // different allocations and must not be EQ.
    assert_eq!(eval_line("(eq (cons 1 2) (cons 1 2))", &env), "()");
    assert_eq!(eval_line("(eq (list 1 2 3) (list 1 2 3))", &env), "()");
    assert_eq!(
        eval_line("(let ((x (cons 1 2)) (y (cons 1 2))) (eq x y))", &env),
        "()"
    );
    // EQUAL must still be true for these (structural equality is untouched).
    assert_eq!(eval_line("(equal (cons 1 2) (cons 1 2))", &env), "T");
    assert_eq!(eval_line("(equal (list 1 2 3) (list 1 2 3))", &env), "T");
}

#[test]
fn eq_cons_vs_non_cons_is_false() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(eq (cons 1 2) nil)", &env), "()");
    assert_eq!(eval_line("(eq nil (cons 1 2))", &env), "()");
    assert_eq!(eval_line("(eq (cons 1 2) 5)", &env), "()");
    assert_eq!(eval_line("(eq (cons 1 2) 'a)", &env), "()");
}

#[test]
fn eq_nil_identity_unaffected() {
    let env = env_with_stdlib();
    // Nil is not represented as a cons cell; `(eq nil nil)` must remain T,
    // as must `()` and the empty-list results of car/cdr-of-nil.
    assert_eq!(eval_line("(eq nil nil)", &env), "T");
    assert_eq!(eval_line("(eq '() '())", &env), "T");
    assert_eq!(eval_line("(eq (cdr (list 1)) nil)", &env), "T");
}

#[test]
fn eq_atom_semantics_untouched() {
    let env = env_with_stdlib();
    // Fixnums, floats, characters, and strings still compare by value, not
    // identity of allocation.
    assert_eq!(eval_line("(eq 100000 100000)", &env), "T");
    assert_eq!(eval_line("(eq 5 5)", &env), "T");
    assert_eq!(eval_line("(eq 5 6)", &env), "()");
    // Fixnum vs float: never EQ even when numerically equal (different
    // types).
    assert_eq!(eval_line("(eq 5 5.0)", &env), "()");
    assert_eq!(eval_line("(eq 5.0 5)", &env), "()");
    // Floats compare by IEEE value, with the NaN-eq-NaN carve-out and
    // 0.0 == -0.0.
    assert_eq!(eval_line("(eq 1.5 1.5)", &env), "T");
    assert_eq!(eval_line("(eq (/ 0.0 0.0) (/ 0.0 0.0))", &env), "T");
    assert_eq!(eval_line("(eq 0.0 -0.0)", &env), "T");
    // Characters and strings by value.
    assert_eq!(eval_line("(eq 'a' 'a')", &env), "T");
    assert_eq!(eval_line("(eq \"ab\" (concat \"a\" \"b\"))", &env), "T");
    assert_eq!(eval_line("(eq \"ab\" \"ac\")", &env), "()");
    // Symbols: interned, so identity of the interned symbol object; two
    // reads of the same name are the same symbol.
    assert_eq!(eval_line("(eq 'foo 'foo)", &env), "T");
    assert_eq!(eval_line("(eq 'foo 'bar)", &env), "()");
}
