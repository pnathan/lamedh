//! #508: the `rplaca`/`rplacd` help entries must describe what the builtins
//! actually do. They return a NEW cons cell and leave their argument
//! unmodified (`src/evaluator/builtins_extra.rs`); the help once claimed
//! Lisp 1.5 destructive mutation and showed an example that did not hold.
//! Each documented EXAMPLES row is evaluated here and compared with its
//! stated result, so the help text cannot drift from the behaviour again.

use lamedh::environment::Environment;
use lamedh::eval_line;

/// Evaluate every `(form expected)` row of `name`'s help EXAMPLES; returns
/// the list of rows whose actual value is not `equal` to the stated one
/// (as `(form expected actual)`), so a failure names the offending example.
fn mismatched_examples(name: &str) -> String {
    let env = Environment::with_stdlib();
    eval_line(
        &format!("(def $examples (doc-get (get-doc '{name}) 'EXAMPLES))"),
        &env,
    );
    eval_line(
        concat!(
            "(def $bad (let ((acc ()))",
            " (dolist (ex $examples (reverse acc))",
            "  (let ((actual (eval (car ex))))",
            "    (if (not (equal actual (cadr ex)))",
            "        (setq acc (cons (list (car ex) (cadr ex) actual) acc))",
            "        nil)))))"
        ),
        &env,
    );
    assert_eq!(
        eval_line("(> (length $examples) 0)", &env),
        "T",
        "{name} must document at least one example"
    );
    eval_line("$bad", &env)
}

#[test]
fn rplaca_help_examples_match_behaviour() {
    assert_eq!(mismatched_examples("rplaca"), "()");
}

#[test]
fn rplacd_help_examples_match_behaviour() {
    assert_eq!(mismatched_examples("rplacd"), "()");
}

#[test]
fn rplac_help_does_not_claim_mutation() {
    let env = Environment::with_stdlib();
    for name in ["rplaca", "rplacd"] {
        let desc = eval_line(
            &format!("(string-upcase (doc-get (get-doc '{name}) 'DESCRIPTION))"),
            &env,
        );
        assert!(
            desc.contains("NEW CONS CELL") && desc.contains("NOT MODIFIED"),
            "{name} help must say it returns a new cell and does not mutate: {desc}"
        );
        assert!(
            !desc.contains("DESTRUCTIVELY REPLACES"),
            "{name} help still claims mutation: {desc}"
        );
    }
}

#[test]
fn rplac_leaves_argument_unmodified() {
    let env = Environment::with_stdlib();
    assert_eq!(
        eval_line("(let ((x (cons 1 2))) (rplaca x 99) x)", &env),
        "(1 . 2)"
    );
    assert_eq!(
        eval_line("(let ((x (list 1 2))) (rplacd x (list 9)) x)", &env),
        "(1 2)"
    );
    assert_eq!(eval_line("(rplaca (cons 1 2) 99)", &env), "(99 . 2)");
    assert_eq!(eval_line("(rplacd (cons 1 2) 99)", &env), "(1 . 99)");
}
