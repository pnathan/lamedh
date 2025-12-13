mod test_helpers;
use lamedh::eval_line;
use test_helpers::env_with_stdlib;

#[test]
fn test_cons_dotted_pair() {
    let env = env_with_stdlib();
    let output = eval_line("(cons 'a 'b)", &env);
    assert_eq!(output, "(A . B)");
}

#[test]
fn test_atom() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(atom 'a)", &env), "T");
    assert_eq!(eval_line("(atom 1)", &env), "T");
    assert_eq!(eval_line("(atom \"s\")", &env), "T");
    assert_eq!(eval_line("(atom '(1 2))", &env), "()");
    assert_eq!(eval_line("(atom (cons 1 2))", &env), "()");
    assert_eq!(eval_line("(atom nil)", &env), "T");
}

#[test]
fn test_null() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(null nil)", &env), "T");
    assert_eq!(eval_line("(null '())", &env), "T");
    assert_eq!(eval_line("(null 1)", &env), "()");
    assert_eq!(eval_line("(null T)", &env), "()");
}

#[test]
fn test_cdr_of_dotted_list() {
    let env = env_with_stdlib();
    let output = eval_line("(cdr (cons 1 (cons 2 3)))", &env);
    assert_eq!(output, "(2 . 3)");
}

#[test]
fn test_pairlis() {
    let env = env_with_stdlib();
    // Test with equal length lists
    let output1 = eval_line("(pairlis '(a b c) '(1 2 3))", &env);
    assert_eq!(output1, "((A . 1) (B . 2) (C . 3))");

    // Test with keys list shorter
    let output2 = eval_line("(pairlis '(a b) '(1 2 3))", &env);
    assert_eq!(output2, "((A . 1) (B . 2))");

    // Test with values list shorter
    let output3 = eval_line("(pairlis '(a b c) '(1 2))", &env);
    assert_eq!(output3, "((A . 1) (B . 2))");

    // Test with one list empty
    let output4 = eval_line("(pairlis '() '(1 2 3))", &env);
    assert_eq!(output4, "()");

    // Test with both lists empty
    let output5 = eval_line("(pairlis '() '())", &env);
    assert_eq!(output5, "()");
}
