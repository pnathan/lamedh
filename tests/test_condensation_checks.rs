mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

fn install_invoice(env: &lamedh::Shared<lamedh::environment::Environment>) {
    eval_line(
        "(defconcept invoice (:fields ((id int64) (amount int64) (status symbol))) (:invariant (>= amount 0)))",
        env,
    );
}

#[test]
fn deflaw_attaches_predicate_and_trace_metadata() {
    let env = env_with_stdlib();
    install_invoice(&env);

    assert_eq!(
        eval_line(
            "(deflaw invoice-nonnegative (:for invoice) (:assert (>= amount 0)))",
            &env
        ),
        "INVOICE-NONNEGATIVE"
    );
    assert_eq!(
        eval_line("(invoice-nonnegative (make-invoice 1 10 'draft))", &env),
        "T"
    );
    assert_eq!(
        eval_line("(invoice-nonnegative (make-invoice 1 -10 'draft))", &env),
        "()"
    );
    assert_eq!(
        eval_line("(cdr (assoc 'laws (condense-trace 'invoice)))", &env),
        "(INVOICE-NONNEGATIVE)"
    );
    assert_eq!(
        eval_line(
            "(cdr (assoc 'concept (condense-trace 'invoice-nonnegative)))",
            &env
        ),
        "INVOICE"
    );
}

#[test]
fn example_attaches_executable_check_to_concept() {
    let env = env_with_stdlib();
    install_invoice(&env);

    assert_eq!(
        eval_line(
            "(example valid-draft-invoice (:for invoice) (:given (make-invoice 1 100 'draft)) (:expect (validate-invoice *it*)))",
            &env
        ),
        "VALID-DRAFT-INVOICE"
    );
    assert_eq!(eval_line("(valid-draft-invoice)", &env), "T");
    assert_eq!(
        eval_line("(cdr (assoc 'examples (condense-trace 'invoice)))", &env),
        "(VALID-DRAFT-INVOICE)"
    );
}

#[test]
fn condense_check_runs_concept_examples() {
    let env = env_with_stdlib();
    install_invoice(&env);
    eval_line(
        "(example valid-draft-invoice (:for invoice) (:given (make-invoice 1 100 'draft)) (:expect (validate-invoice *it*)))",
        &env,
    );
    eval_line(
        "(example invalid-negative-invoice (:for invoice) (:given (make-invoice 1 -100 'draft)) (:expect (validate-invoice *it*)))",
        &env,
    );

    assert_eq!(
        eval_line("(condense-check 'invoice)", &env),
        "(() (VALID-DRAFT-INVOICE . T) (INVALID-NEGATIVE-INVOICE))"
    );
    assert_eq!(
        eval_line("(condense-check 'valid-draft-invoice)", &env),
        "(T (VALID-DRAFT-INVOICE . T))"
    );
}

#[test]
fn laws_and_examples_require_concepts() {
    let env = env_with_stdlib();
    let law = eval_line("(deflaw nope (:for missing) (:assert t))", &env);
    assert!(law.contains("Error"), "got: {law}");
    assert!(law.contains("concept"), "got: {law}");

    let ex = eval_line(
        "(example nope-example (:for missing) (:given 1) (:expect t))",
        &env,
    );
    assert!(ex.contains("Error"), "got: {ex}");
    assert!(ex.contains("concept"), "got: {ex}");
}
