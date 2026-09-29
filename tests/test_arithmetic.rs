mod test_helpers;
use lamedh::eval_line;
use test_helpers::env_with_stdlib;

#[test]
fn test_add_two_numbers() {
    let env = env_with_stdlib();
    let output = eval_line("(PLUS 1 2)", &env);
    assert_eq!(output, "3");
}

#[test]
fn test_numeric_compare() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(EQUAL-NUMBER 1 1)", &env), "T");
    assert_eq!(eval_line("(EQUAL-NUMBER 1 2)", &env), "()");
}

/// #517: the `mod` help entry must describe the Euclidean `mod` the
/// evaluator implements, and every example in it must evaluate to the
/// result it claims.
#[test]
fn test_mod_help_examples_match_behaviour() {
    let env = env_with_stdlib();
    let examples = "(doc-get (get-doc 'mod) 'EXAMPLES)";
    assert_eq!(eval_line(&format!("(length {examples})"), &env), "6");
    assert_eq!(
        eval_line(
            &format!(
                "(let ((bad nil))
                   (dolist (ex {examples})
                     (unless (equal (eval (car ex)) (cadr ex))
                       (setq bad (cons ex bad))))
                   bad)"
            ),
            &env
        ),
        "()"
    );
    // The examples cover every sign combination, including a negative divisor.
    assert_eq!(eval_line("(mod 7 -2)", &env), "1");
    assert_eq!(eval_line("(mod 5 -3)", &env), "2");
    assert_eq!(eval_line("(mod -7 3)", &env), "2");
    assert_eq!(eval_line("(mod -7 -3)", &env), "2");
    let desc = eval_line("(doc-get (get-doc 'mod) 'DESCRIPTION)", &env);
    assert!(desc.contains("Euclidean"), "{desc}");
    assert!(!desc.contains("same sign as divisor"), "{desc}");
}
