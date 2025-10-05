mod test_helpers;
use lithhp::eval_line;
use test_helpers::env_with_prologue;

#[test]
fn test_eq() {
    let env = env_with_prologue();
    assert_eq!(eval_line("(EQ 1 1)", &env), "T");
    assert_eq!(eval_line("(EQ 1 2)", &env), "()");
    assert_eq!(eval_line("(EQ \"a\" \"a\")", &env), "T");
    assert_eq!(eval_line("(EQ \"a\" \"b\")", &env), "()");
    assert_eq!(eval_line("(EQ T T)", &env), "T");
    assert_eq!(eval_line("(EQ nil nil)", &env), "T");
    assert_eq!(eval_line("(EQ T nil)", &env), "()");
}

#[test]
fn test_logical_ops() {
    let env = env_with_prologue();
    assert_eq!(eval_line("(not T)", &env), "()");
    assert_eq!(eval_line("(not nil)", &env), "T");
    assert_eq!(eval_line("(and T T)", &env), "T");
    assert_eq!(eval_line("(and T nil)", &env), "()");
    assert_eq!(eval_line("(or T nil)", &env), "T");
    assert_eq!(eval_line("(or nil nil)", &env), "()");
}

#[test]
fn test_if_with_t_nil() {
    let env = env_with_prologue();
    assert_eq!(eval_line("(if T 1 2)", &env), "1");
    assert_eq!(eval_line("(if nil 1 2)", &env), "2");
}

#[test]
fn test_cond() {
    let env = env_with_prologue();
    assert_eq!(eval_line("(cond (T 1))", &env), "1");
    assert_eq!(eval_line("(cond (() 1) (T 2))", &env), "2");
    assert_eq!(eval_line("(cond (nil 1) (T 2))", &env), "2");
    assert_eq!(eval_line("(cond (() 1) (() 2))", &env), "()");
    assert_eq!(eval_line("(cond (T))", &env), "T");
    assert_eq!(eval_line("(cond (1))", &env), "1");
    assert_eq!(eval_line("(cond (T 1 2 3))", &env), "3");
}
