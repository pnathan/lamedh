mod test_helpers;

use lamedh::{LispVal, eval_line, reader::read};
use test_helpers::env_with_stdlib;

#[test]
fn reader_accepts_keyword_symbols() {
    let env = env_with_stdlib();
    let parsed = read(":op", &env).expect("keyword should parse");
    match parsed {
        LispVal::Symbol(s) => assert_eq!(s.borrow().name, ":OP"),
        other => panic!("expected keyword symbol, got {other:?}"),
    }
}

#[test]
fn keywords_are_self_evaluating() {
    let env = env_with_stdlib();
    assert_eq!(eval_line(":op", &env), ":OP");
}

#[test]
fn keywords_work_as_unquoted_data() {
    let env = env_with_stdlib();
    assert_eq!(
        eval_line("(list :op 'eqv 'point-equal)", &env),
        "(:OP EQV POINT-EQUAL)"
    );
}

#[test]
fn keyword_identity_uses_symbol_interning() {
    let env = env_with_stdlib();
    assert_eq!(eval_line("(eq :op ':op)", &env), "T");
}
