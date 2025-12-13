mod test_helpers;
use lamedh::eval_line;
use test_helpers::env_with_stdlib;

#[test]
fn test_rust_primitives() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(< 2 3)", &env), "T");
    assert_eq!(eval_line("(< 3 2)", &env), "()");
    assert_eq!(eval_line("(> 5 3)", &env), "T");
    assert_eq!(eval_line("(> 3 5)", &env), "()");
    assert_eq!(eval_line("(zerop 0)", &env), "T");
    assert_eq!(eval_line("(zerop 1)", &env), "()");
    assert_eq!(eval_line("(remainder 7 3)", &env), "1");
    assert_eq!(eval_line("(expt 2 3)", &env), "8");
    assert_eq!(eval_line("(numberp 123)", &env), "T");
    assert_eq!(eval_line("(numberp \"hello\")", &env), "()");
}

#[test]
fn test_lisp_derived_functions() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(onep 1)", &env), "T");
    assert_eq!(eval_line("(onep 0)", &env), "()");
    assert_eq!(eval_line("(minusp -5)", &env), "T");
    assert_eq!(eval_line("(minusp 5)", &env), "()");
    assert_eq!(eval_line("(add1 5)", &env), "6");
    assert_eq!(eval_line("(sub1 5)", &env), "4");
    assert_eq!(eval_line("(max 1 5 3 2)", &env), "5");
    assert_eq!(eval_line("(min 1 5 3 2)", &env), "1");
    assert_eq!(eval_line("(abs -42)", &env), "42");
    assert_eq!(eval_line("(abs 42)", &env), "42");
    assert_eq!(eval_line("(listp '(1 2))", &env), "T");
    assert_eq!(eval_line("(listp 'a)", &env), "()");
    assert_eq!(eval_line("(consp '(1 2))", &env), "T");
    assert_eq!(eval_line("(consp nil)", &env), "()");
}
