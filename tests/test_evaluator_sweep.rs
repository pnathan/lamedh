//! Regression tests from the evaluator / special-form bug sweep.
//!
//! Each section pins one defect against the contract in `KERNEL.md`.

mod test_helpers;
use lamedh::eval_line;
use test_helpers::env_with_stdlib;

// ---------------------------------------------------------------------------
// QUASIQUOTE: `,@` splice semantics (stack-depth regression lives in
// `src/evaluator/quasiquote.rs`, which can reach the private helper)
// ---------------------------------------------------------------------------

#[test]
fn unquote_splicing_keeps_order_tail_and_errors() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("`(1 ,@(list 2 3) 4)", &env), "(1 2 3 4)");
    assert_eq!(eval_line("`(1 ,@(list 2 3) . 4)", &env), "(1 2 3 . 4)");
    assert_eq!(eval_line("`(,@nil)", &env), "()");
    assert_eq!(eval_line("`(,@nil . 5)", &env), "5");
    // Spliced cells are copied; the original list is untouched and unshared.
    assert_eq!(
        eval_line(
            "(let ((l (list 1 2))) (list (eq (cdr `(0 ,@l)) l) l))",
            &env
        ),
        "(() (1 2))"
    );
    // Non-list and improper splices are errors.
    assert!(eval_line("`(1 ,@5)", &env).contains("expected a list argument"));
    assert!(eval_line("`(1 ,@'(2 . 3))", &env).contains("expected a list argument"));
}

// ---------------------------------------------------------------------------
// Definition forms must refuse `T` and keywords (KERNEL.md: SETQ/LAMBDA/LET/DEF
// all reject them; DEFINE, DEFMACRO, DEFEXPR and LABEL did not)
// ---------------------------------------------------------------------------

#[test]
fn definition_forms_refuse_to_rebind_t() {
    for form in [
        "(define ((t 1)))",
        "(defmacro t () 1)",
        "(defexpr t (x) x)",
        "(label t (lambda () 1))",
        "(defun clobber () (define ((t 1)))) (clobber)",
    ] {
        let env = env_with_stdlib();
        let out = eval_line(&format!("(progn {form})"), &env);
        assert!(
            out.contains("cannot rebind the constant T"),
            "{form} => {out}"
        );
        // T must still be the truth constant afterwards.
        assert_eq!(
            eval_line("(list t (if t 'yes 'no))", &env),
            "(T YES)",
            "{form}"
        );
    }
}

#[test]
fn definition_forms_refuse_keywords() {
    for form in [
        "(define ((:k 1)))",
        "(defmacro :k () 1)",
        "(defexpr :k (x) x)",
        "(label :k (lambda () 1))",
    ] {
        let env = env_with_stdlib();
        let out = eval_line(form, &env);
        assert!(
            out.contains("cannot bind the keyword :K"),
            "{form} => {out}"
        );
        assert_eq!(eval_line(":k", &env), ":K", "{form}");
    }
}

#[test]
fn definition_forms_still_accept_ordinary_names() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("(define ((dq-a 5) (dq-b 6)))", &env),
        "(DQ-A DQ-B)"
    );
    assert_eq!(eval_line("(list dq-a dq-b)", &env), "(5 6)");
    assert_eq!(
        eval_line("(progn (defmacro dq-m (x) x) (dq-m 7))", &env),
        "7"
    );
    assert_eq!(
        eval_line("(progn (defexpr dq-e (x) x) (dq-e (a b)))", &env),
        "((A B))"
    );
    assert_eq!(
        eval_line(
            "((label dq-f (lambda (n) (if (= n 0) 0 (dq-f (- n 1))))) 3)",
            &env
        ),
        "0"
    );
}

// ---------------------------------------------------------------------------
// LET binds in parallel even when some bound names are dynamic
// (KERNEL.md: "every init is evaluated in the outer environment ... before
// the body sees any new binding")
// ---------------------------------------------------------------------------

#[test]
fn let_inits_do_not_see_earlier_dynamic_bindings() {
    let env = env_with_stdlib();
    eval_line("(defdynamic *lx* 1)", &env);
    eval_line("(defdynamic *ly* 10)", &env);
    eval_line("(defun lx-now () *lx*)", &env);
    // A later init reads the OUTER value, directly or through a callee.
    assert_eq!(eval_line("(let ((*lx* 2) (y *lx*)) y)", &env), "1");
    assert_eq!(eval_line("(let ((*lx* 2) (y (lx-now))) y)", &env), "1");
    // Swap: both inits see the outer values.
    assert_eq!(
        eval_line("(let ((*lx* *ly*) (*ly* *lx*)) (list *lx* *ly*))", &env),
        "(10 1)"
    );
    // The body sees the new bindings; they are restored afterwards.
    assert_eq!(
        eval_line("(let ((*lx* 2) (y 3)) (list *lx* y))", &env),
        "(2 3)"
    );
    assert_eq!(eval_line("(list *lx* *ly*)", &env), "(1 10)");
}

#[test]
fn let_with_dynamic_binding_is_parallel_inside_a_function_body() {
    let env = env_with_stdlib();
    eval_line("(defdynamic *lz* 1)", &env);
    // `no-compile` keeps this on the interpreter's LET (the typed JIT path is a
    // separate component with its own handling of dynamic names).
    eval_line(
        "(defun lz-probe () (declare (no-compile)) (let ((*lz* 2) (y *lz*)) y))",
        &env,
    );
    assert_eq!(eval_line("(lz-probe)", &env), "1");
}

#[test]
fn let_dynamic_restored_when_a_later_init_fails() {
    let env = env_with_stdlib();
    eval_line("(defdynamic *la* 1)", &env);
    eval_line("(defdynamic *lb* 1)", &env);
    eval_line("(errorset '(let ((*la* 2) (*lb* (car 5))) 1))", &env);
    assert_eq!(eval_line("(list *la* *lb*)", &env), "(1 1)");
}

#[test]
fn let_star_still_sees_earlier_dynamic_bindings() {
    let env = env_with_stdlib();
    eval_line("(defdynamic *ls* 1)", &env);
    assert_eq!(eval_line("(let* ((*ls* 2) (y *ls*)) y)", &env), "2");
}
