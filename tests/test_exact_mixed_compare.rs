//! Issue #516: mixed int/float comparison must be exact (no rounding of the
//! integer through f64). Covers `= < > <= >= /=` in both argument orders.

use lamedh::LispVal;
use lamedh::environment::Environment;
use lamedh::evaluator::eval;
use lamedh::reader::read;

fn truthy(env: &lamedh::Shared<Environment>, src: &str) -> bool {
    let expr = read(src, env).unwrap_or_else(|e| panic!("parse {src}: {e}"));
    let v = eval(&expr, env).unwrap_or_else(|e| panic!("eval {src}: {e:?}"));
    v != LispVal::Nil
}

/// Each row: (a, b, [=, <, >, <=, >=, /=] expected for (op a b)).
/// The reversed order is derived and checked too.
const ROWS: &[(&str, &str, [bool; 6])] = &[
    // 2^53 + 1 vs 2^53 as float: the repro.
    (
        "9007199254740993",
        "9007199254740992.0",
        [false, false, true, false, true, true],
    ),
    (
        "9007199254740992",
        "9007199254740992.0",
        [true, false, false, true, true, false],
    ),
    (
        "9007199254740991",
        "9007199254740992.0",
        [false, true, false, true, false, true],
    ),
    (
        "-9007199254740993",
        "-9007199254740992.0",
        [false, true, false, true, false, true],
    ),
    // 2^63 boundary: i64::MAX < 2^63 as float; i64::MIN == -2^63 as float.
    (
        "9223372036854775807",
        "9223372036854775808.0",
        [false, true, false, true, false, true],
    ),
    (
        "-9223372036854775808",
        "-9223372036854775808.0",
        [true, false, false, true, true, false],
    ),
    (
        "-9223372036854775807",
        "-9223372036854775808.0",
        [false, false, true, false, true, true],
    ),
    // Beyond i64: compare by sign.
    (
        "9223372036854775807",
        "1e30",
        [false, true, false, true, false, true],
    ),
    (
        "-9223372036854775808",
        "-1e30",
        [false, false, true, false, true, true],
    ),
    // Infinities.
    (
        "9223372036854775807",
        "(/ 1.0 0.0)",
        [false, true, false, true, false, true],
    ),
    (
        "-9223372036854775808",
        "(/ -1.0 0.0)",
        [false, false, true, false, true, true],
    ),
    // Signed zero: -0.0 == 0.
    ("0", "-0.0", [true, false, false, true, true, false]),
    ("0", "0.0", [true, false, false, true, true, false]),
    // Fractions.
    ("2", "2.5", [false, true, false, true, false, true]),
    ("3", "2.5", [false, false, true, false, true, true]),
    ("-2", "-2.5", [false, false, true, false, true, true]),
    ("-3", "-2.5", [false, true, false, true, false, true]),
    ("0", "0.5", [false, true, false, true, false, true]),
    ("0", "-0.5", [false, false, true, false, true, true]),
    // Ordinary exact values.
    ("3", "3.0", [true, false, false, true, true, false]),
];

fn check(env: &lamedh::Shared<Environment>) {
    let ops = ["=", "<", ">", "<=", ">=", "/="];
    for (a, b, exp) in ROWS {
        for (i, op) in ops.iter().enumerate() {
            let got = truthy(env, &format!("({op} {a} {b})"));
            assert_eq!(got, exp[i], "({op} {a} {b})");
        }
        // Swapped operands: = and /= symmetric; < <-> >, <= <-> >=.
        let swapped = [exp[0], exp[2], exp[1], exp[4], exp[3], exp[5]];
        for (i, op) in ops.iter().enumerate() {
            let got = truthy(env, &format!("({op} {b} {a})"));
            assert_eq!(got, swapped[i], "({op} {b} {a})");
        }
    }
}

#[test]
fn exact_mixed_compare_table() {
    lamedh::with_large_stack(|| check(&Environment::with_stdlib()));
}

#[test]
fn repro_issue_516() {
    lamedh::with_large_stack(|| {
        let env = Environment::with_stdlib();
        assert!(!truthy(&env, "(= 9007199254740993 9007199254740992.0)"));
        assert!(!truthy(&env, "(= 9007199254740992.0 9007199254740993)"));
    });
}

#[test]
fn nan_is_unordered_with_ints() {
    lamedh::with_large_stack(|| {
        let env = Environment::with_stdlib();
        for nan in ["(- (/ 1.0 0.0) (/ 1.0 0.0))"] {
            for i in ["0", "1", "-1", "9223372036854775807"] {
                for op in ["=", "<", ">", "<=", ">="] {
                    assert!(
                        !truthy(&env, &format!("({op} {i} {nan})")),
                        "({op} {i} NaN)"
                    );
                    assert!(
                        !truthy(&env, &format!("({op} {nan} {i})")),
                        "({op} NaN {i})"
                    );
                }
                assert!(truthy(&env, &format!("(/= {i} {nan})")));
                assert!(truthy(&env, &format!("(/= {nan} {i})")));
            }
        }
    });
}

#[test]
fn variadic_chains_mixed() {
    lamedh::with_large_stack(|| {
        let env = Environment::with_stdlib();
        assert!(truthy(&env, "(< 1 1.5 2 2.5)"));
        assert!(!truthy(
            &env,
            "(< 9007199254740992.0 9007199254740993 9007199254740993.0)"
        ));
        assert!(truthy(&env, "(< 9007199254740992.0 9007199254740993)"));
        assert!(!truthy(
            &env,
            "(= 9007199254740993 9007199254740993 9007199254740992.0)"
        ));
        assert!(truthy(&env, "(= 3 3.0 3)"));
    });
}

#[test]
fn same_type_unchanged() {
    lamedh::with_large_stack(|| {
        let env = Environment::with_stdlib();
        assert!(truthy(&env, "(< 9007199254740992 9007199254740993)"));
        assert!(!truthy(&env, "(= 9007199254740992 9007199254740993)"));
        assert!(truthy(&env, "(< 1.5 2.5)"));
        assert!(truthy(&env, "(= 0.0 -0.0)"));
        assert!(truthy(&env, "(<= 2 2)"));
        assert!(truthy(&env, "(>= 2.5 2.5)"));
        assert!(truthy(&env, "(/= 1 2)"));
    });
}

#[cfg(feature = "jit")]
#[test]
fn typed_literal_vs_float_agrees_with_interpreter() {
    lamedh::with_large_stack(|| {
        let env = Environment::with_stdlib();
        let run = |s: &str| {
            let e = read(s, &env).unwrap();
            eval(&e, &env)
        };
        let t = |s: &str| run(s).unwrap_or_else(|e| panic!("{s}: {e:?}")) != LispVal::Nil;
        // A literal beyond 2^53 is not coerced to float64 (that would round
        // it): the typed form is rejected instead of compiled inexactly.
        assert!(
            run("(defun-typed (jit-eq516 bool) ((x float64)) (= 9007199254740993 x))").is_err()
        );
        // The dynamic (untyped `defun`) path is exact.
        run("(defun dyn-eq516 (x) (= 9007199254740993 x))").unwrap();
        assert!(!t("(dyn-eq516 9007199254740992.0)"));
        // An exactly representable literal still compiles and agrees.
        run("(defun-typed (jit-lt516 bool) ((x float64)) (< x 9007199254740992))").unwrap();
        assert!(t("(jit-lt516 9007199254740991.0)"));
        assert!(!t("(jit-lt516 9007199254740992.0)"));
    });
}

#[test]
fn portable_checker_agrees_with_native_on_literal_bound() {
    lamedh::with_large_stack(|| {
        let env = Environment::with_stdlib();
        let run = |s: &str| {
            let e = read(s, &env).unwrap();
            eval(&e, &env).map(|v| lamedh::printer::print(&v))
        };
        // Within 2^53 the literal is coerced to float64 and the island
        // compiles; beyond it the operands disagree, as in the native checker.
        run("(defun h516a (x) (< (+ x 0.5) 9007199254740992))").unwrap();
        run("(defun h516b (x) (< (+ x 0.5) 9007199254740993))").unwrap();
        let ok = run("(typed-island '(h516a))").unwrap();
        let bad = run("(typed-island '(h516b))").unwrap();
        assert!(ok.contains("MEMBERS (H516A"), "{ok}");
        assert!(
            bad.contains("REJECTED (H516B") && bad.contains("operands disagree"),
            "{bad}"
        );
    });
}
