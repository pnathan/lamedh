//! #507: `expt` with an integer exponent outside `i32` used to truncate it
//! (`*exp as i32`), so `(expt 2.0 4294967296)` was `1.0`. The result is now
//! IEEE `pow`: saturating to `inf`/`0.0`, sign from the exponent's parity.

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line};

fn env() -> Shared<Environment> {
    Environment::with_stdlib()
}

#[test]
fn out_of_i32_exponents_saturate() {
    let e = env();
    for (expr, want) in [
        ("(expt 2.0 4294967296)", "inf"),
        ("(expt 2 -4294967296)", "0.0"),
        ("(expt 2.0 -9223372036854775808)", "0.0"),
        ("(expt 2 -9223372036854775808)", "0.0"),
        ("(expt 0.5 4294967296)", "0.0"),
        ("(expt 0.5 -4294967296)", "inf"),
        ("(expt 1.0 9223372036854775807)", "1.0"),
    ] {
        assert_eq!(eval_line(expr, &e), want, "{expr}");
    }
}

#[test]
fn negative_base_sign_follows_exponent_parity() {
    let e = env();
    for (expr, want) in [
        ("(expt -2.0 4294967297)", "-inf"),
        ("(expt -2.0 4294967296)", "inf"),
        ("(expt -2.0 -4294967297)", "-0.0"),
        ("(expt -2 -4294967297)", "-0.0"),
        // 2^53 + 1 is odd but rounds to an even f64.
        ("(expt -1.0 9007199254740993)", "-1.0"),
        ("(expt -1.0 9223372036854775807)", "-1.0"),
        ("(expt -1.0 -9223372036854775808)", "1.0"),
    ] {
        assert_eq!(eval_line(expr, &e), want, "{expr}");
    }
}

#[test]
fn in_range_exponents_are_unchanged() {
    let e = env();
    for (expr, want) in [
        ("(expt 2.0 10)", "1024.0"),
        ("(expt 2 -2)", "0.25"),
        ("(expt -2.0 3)", "-8.0"),
        ("(expt 2.0 2147483647)", "inf"),
        ("(expt 2.0 -2147483648)", "0.0"),
    ] {
        assert_eq!(eval_line(expr, &e), want, "{expr}");
    }
}

#[test]
fn near_one_base_is_not_saturated() {
    // |x| close to 1 with a huge exponent stays finite: no shortcut to inf.
    let e = env();
    let got: f64 = eval_line("(expt 1.0000000001 4294967296)", &e)
        .parse()
        .unwrap();
    let want = 1.0000000001f64.powf(4294967296.0);
    assert_eq!(got, want);
    assert!(got.is_finite() && got > 1.0);
}
