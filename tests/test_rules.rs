//! Integration tests for the rulebook optimizer (lib/24-rules.lisp):
//! DEFRULE / UNDEFRULE / LIST-RULES / APPLY-RULES and the OPTIMIZE-FORM
//! pipeline hook — optimization passes as pattern-language data.

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line};

fn env() -> Shared<Environment> {
    Environment::with_stdlib()
}

#[test]
fn starter_rules_fold_pure_structural_identities() {
    let e = env();
    assert_eq!(
        eval_line("(apply-rules '(car (cons (+ 1 2) unused)))", &e),
        "(+ 1 2)"
    );
    assert_eq!(
        eval_line("(apply-rules '(cdr (cons unused (list a b))))", &e),
        "(LIST A B)"
    );
    assert_eq!(
        eval_line("(apply-rules '(f (append (g x) nil)))", &e),
        "(F (G X))"
    );
}

#[test]
fn purity_guards_block_effectful_folds() {
    let e = env();
    // Dropping (print ...) would lose an effect: the guard must refuse.
    assert_eq!(
        eval_line("(apply-rules '(car (cons 1 (print 'effect))))", &e),
        "(CAR (CONS 1 (PRINT (QUOTE EFFECT))))"
    );
    assert_eq!(
        eval_line("(apply-rules '(cdr (cons (setq x 9) tail)))", &e),
        "(CDR (CONS (SETQ X 9) TAIL))"
    );
}

#[test]
fn quoted_data_is_never_rewritten() {
    let e = env();
    assert_eq!(
        eval_line("(apply-rules '(quote (car (cons 1 2))))", &e),
        "(QUOTE (CAR (CONS 1 2)))"
    );
}

#[test]
fn user_rules_register_cascade_and_unregister() {
    let e = env();
    eval_line("(defrule tr-double (* ?x 2) (+ ?x ?x))", &e);
    // Bottom-up cascade: the starter rule folds the inner form first, then
    // the user rule fires on the rebuilt node.
    assert_eq!(
        eval_line("(apply-rules '(* (car (cons q nil)) 2))", &e),
        "(+ Q Q)"
    );
    // Redefinition replaces (same name), UNDEFRULE removes.
    eval_line("(defrule tr-double (* ?x 2) (shl ?x))", &e);
    assert_eq!(eval_line("(apply-rules '(* z 2))", &e), "(SHL Z)");
    eval_line("(undefrule 'tr-double)", &e);
    assert_eq!(eval_line("(apply-rules '(* z 2))", &e), "(* Z 2)");
}

#[test]
fn guards_see_matched_forms_as_data() {
    let e = env();
    // A guard that only fires for symbol operands: evaluating ?x inside the
    // guard yields the matched subform itself.
    eval_line(
        "(defrule tr-symbols-only (wrap ?x) (wrapped ?x) :when (symbolp ?x))",
        &e,
    );
    assert_eq!(eval_line("(apply-rules '(wrap abc))", &e), "(WRAPPED ABC)");
    assert_eq!(eval_line("(apply-rules '(wrap (f 1)))", &e), "(WRAP (F 1))");
    eval_line("(undefrule 'tr-symbols-only)", &e);
}

#[test]
fn cyclic_rulebooks_terminate_bounded() {
    let e = env();
    eval_line("(defrule tr-la (loopy ?x) (loopy2 ?x))", &e);
    eval_line("(defrule tr-lb (loopy2 ?x) (loopy ?x))", &e);
    // Bounded fixpoint: returns (no hang), landing on one of the two shapes.
    let out = eval_line("(apply-rules '(loopy 5))", &e);
    assert!(
        out == "(LOOPY 5)" || out == "(LOOPY2 5)",
        "cyclic rulebook must terminate, got: {out}"
    );
    eval_line("(undefrule 'tr-la)", &e);
    eval_line("(undefrule 'tr-lb)", &e);
}

#[test]
fn optimize_form_runs_the_rulebook_before_constant_folding() {
    let e = env();
    // rulebook: (car (cons (+ 1 2) dead)) -> (+ 1 2); builtin folder -> 3.
    assert_eq!(
        eval_line("(optimize-form '(car (cons (+ 1 2) dead)))", &e),
        "3"
    );
}

#[test]
fn list_rules_exposes_the_book_as_data() {
    let e = env();
    let out = eval_line("(mapcar (lambda (r) (car r)) (list-rules))", &e);
    for name in ["CAR-OF-CONS", "CDR-OF-CONS", "APPEND-NIL"] {
        assert!(out.contains(name), "rulebook missing {name}: {out}");
    }
    // And the rules themselves are inspectable patterns.
    let out = eval_line(
        "(match (assoc 'car-of-cons (list-rules)) ((?name ?pat ?tmpl) ?pat))",
        &e,
    );
    assert_eq!(out, "(CAR (CONS ?A ?B))");
}

#[test]
fn dollar_opt_still_evaluates_correctly_through_the_new_pipeline() {
    let e = env();
    assert_eq!(eval_line("($opt (+ (car (cons 20 dead)) 22))", &e), "42");
}
