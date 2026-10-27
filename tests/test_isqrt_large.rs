mod test_helpers;
use lamedh::eval_line;
use test_helpers::env_with_stdlib;

#[test]
fn test_isqrt_large_integers_exact() {
    let env = env_with_stdlib();
    // 3037000499^2 - 1: f64 rounding used to round the root up to 3037000499.
    assert_eq!(eval_line("(isqrt 9223372030926249000)", &env), "3037000498");
    assert_eq!(eval_line("(isqrt 9223372030926249001)", &env), "3037000499");
    assert_eq!(eval_line("(isqrt 9223372036854775807)", &env), "3037000499");
    assert_eq!(eval_line("(isqrt 17)", &env), "4");
    assert_eq!(eval_line("(isqrt 0)", &env), "0");
}
