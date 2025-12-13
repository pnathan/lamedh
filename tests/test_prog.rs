mod test_helpers;
use lamedh::{eval_line, evaluator, reader};
use test_helpers::env_with_stdlib;

#[test]
fn test_prog_feature() {
    let env = env_with_stdlib();
    let test_code = std::fs::read_to_string("tests/prog_test.lisp").unwrap();
    let expressions = reader::read_all(&test_code, &env).unwrap();
    for expr in expressions {
        evaluator::eval(&expr, &env).unwrap();
    }

    // test-prog-basic: returns 10 + 20 = 30
    assert_eq!(eval_line("(test-prog-basic)", &env), "30");

    // test-prog-return: (RETURN X) where X is 100
    assert_eq!(eval_line("(test-prog-return)", &env), "100");

    // test-prog-go-forward: (RETURN X) where X is 101
    assert_eq!(eval_line("(test-prog-go-forward)", &env), "111");

    // test-prog-go-backward-loop: (RETURN SUM) where SUM is 1+2+3+4+5=15
    assert_eq!(eval_line("(test-prog-go-backward-loop)", &env), "15");

    // test-prog-fall-through: falls through, returns NIL
    assert_eq!(eval_line("(test-prog-fall-through)", &env), "()");

    // test-nested-prog: The inner prog returns 10, which is then returned by the outer prog.
    assert_eq!(eval_line("(test-nested-prog)", &env), "10");
}
