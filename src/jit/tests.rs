//! Stress tests for the typed-JIT prototype.
//!
//! The `agree` helper runs every example through *both* the compiled and the
//! interpreted edition and asserts they match, so essentially every functional
//! test is also a compiled-vs-interpreter differential test.

use super::*;
use crate::environment::Environment;
use crate::reader::read;

// --- helpers ---------------------------------------------------------------

fn build(srcs: &[&str]) -> Jit {
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    for s in srcs {
        let form = read(s, &env).expect("read failed");
        j.define(&form)
            .unwrap_or_else(|e| panic!("define `{s}` failed: {e}"));
    }
    j
}

fn def_err(src: &str) -> String {
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    let form = read(src, &env).expect("read failed");
    j.define(&form).expect_err("expected a type error")
}

/// Call through both editions; assert agreement; leave the jit compiled.
fn agree(j: &Jit, name: &str, args: &[Value]) -> Value {
    j.compile_all();
    let compiled = j.call(name, args).unwrap();
    j.deoptimize_all();
    let interpreted = j.call(name, args).unwrap();
    assert_eq!(
        compiled, interpreted,
        "compiled vs interpreted disagree on {name}{args:?}"
    );
    j.compile_all();
    compiled
}

fn i(n: i64) -> Value {
    Value::Int(n)
}
fn fl(x: f64) -> Value {
    Value::Float(x)
}
fn bo(x: bool) -> Value {
    Value::Bool(x)
}
fn ints(xs: &[i64]) -> Value {
    Value::Array(xs.iter().map(|n| Value::Int(*n)).collect())
}
fn floats(xs: &[f64]) -> Value {
    Value::Array(xs.iter().map(|x| Value::Float(*x)).collect())
}
fn chars(s: &str) -> Value {
    Value::Array(s.bytes().map(Value::Char).collect())
}

// --- arithmetic: int -------------------------------------------------------

#[test]
fn int_add_sub_mul() {
    let j = build(&["(defun-typed (f int64) ((a int64) (b int64)) (+ (* a b) (- a b)))"]);
    assert_eq!(agree(&j, "f", &[i(6), i(4)]), i(24 + 2));
    assert_eq!(agree(&j, "f", &[i(-3), i(7)]), i(-21 + -10));
}

#[test]
fn int_div_and_mod() {
    let j = build(&[
        "(defun-typed (d int64) ((a int64) (b int64)) (/ a b))",
        "(defun-typed (m int64) ((a int64) (b int64)) (mod a b))",
    ]);
    assert_eq!(agree(&j, "d", &[i(17), i(5)]), i(3));
    assert_eq!(agree(&j, "m", &[i(17), i(5)]), i(2));
    assert_eq!(agree(&j, "d", &[i(-17), i(5)]), i(-3)); // truncating
}

/// Issue #209: unlike `+`/`-`/`*`, `/` and `MOD` are strictly binary in the
/// evaluator (`BuiltinFunc::Divide`/`mod` both reject anything but exactly 2
/// arguments) -- no unary reciprocal, no variadic chain division/modulus.
/// `elab_bin` used to accept 0/1/3+ arities anyway, silently compiling a
/// made-up unary/N-ary meaning for a form the evaluator would refuse to run.
/// Every wrong arity must now be rejected at every arity boundary. (The
/// check-type/checker-mode side of this is covered separately in
/// tests/typed_jit_integration.rs, since `check-type` goes through
/// `Jit::infer_untyped`/`check_untyped`, not `Jit::define`.)
#[test]
fn div_mod_reject_every_arity_but_two() {
    for op in ["/", "mod"] {
        let err0 = def_err(&format!("(defun-typed (z int64) () ({op}))"));
        assert!(err0.contains("exactly 2"), "0-arg `{op}` must be rejected");
        assert!(
            err0.contains("got 0"),
            "0-arg `{op}` error must name the actual arity, got: {err0}"
        );
        let err1 = def_err(&format!("(defun-typed (u int64) ((a int64)) ({op} a))"));
        assert!(err1.contains("exactly 2"), "1-arg `{op}` must be rejected");
        assert!(
            err1.contains("got 1"),
            "1-arg `{op}` error must name the actual arity, got: {err1}"
        );
        let err3 = def_err(&format!(
            "(defun-typed (t3 int64) ((a int64) (b int64) (c int64)) ({op} a b c))"
        ));
        assert!(err3.contains("exactly 2"), "3-arg `{op}` must be rejected");
        assert!(
            err3.contains("got 3"),
            "3-arg `{op}` error must name the actual arity, got: {err3}"
        );
    }
}

/// `+`/`-`/`*` must be entirely unaffected by #209's `/`/`MOD` arity guard
/// at every arity the evaluator actually supports (0-arg identity, unary
/// negate for `-`, and 3+-ary left-fold for all three).
#[test]
fn plus_minus_times_unaffected_by_div_mod_arity_fix() {
    let j = build(&[
        "(defun-typed (p0 int64) () (+))",
        "(defun-typed (t0 int64) () (*))",
        "(defun-typed (neg int64) ((a int64)) (- a))",
        "(defun-typed (p3 int64) ((a int64) (b int64) (c int64)) (+ a b c))",
        "(defun-typed (s3 int64) ((a int64) (b int64) (c int64)) (- a b c))",
        "(defun-typed (t3 int64) ((a int64) (b int64) (c int64)) (* a b c))",
    ]);
    assert_eq!(agree(&j, "p0", &[]), i(0));
    assert_eq!(agree(&j, "t0", &[]), i(1));
    assert_eq!(agree(&j, "neg", &[i(5)]), i(-5));
    assert_eq!(agree(&j, "p3", &[i(1), i(2), i(3)]), i(6));
    assert_eq!(agree(&j, "s3", &[i(10), i(3), i(2)]), i(5));
    assert_eq!(agree(&j, "t3", &[i(2), i(3), i(4)]), i(24));
}

#[test]
fn int_div_by_zero_is_zero_not_panic() {
    let j = build(&["(defun-typed (d int64) ((a int64) (b int64)) (/ a b))"]);
    assert_eq!(agree(&j, "d", &[i(5), i(0)]), i(0));
}

#[test]
fn int_div_mod_overflow_min_by_neg1_is_zero_not_trap() {
    // `i64::MIN / -1` and `i64::MIN % -1` overflow signed division and trap
    // (SIGFPE) on hardware `sdiv`/`srem`. The reference editions use
    // `wrapping_div` (sets OVERFLOW flag); Euclidean mod yields the exact 0
    // with no flag (#280). Every edition
    // (interpreter, closure, and the native Cranelift backend under
    // `--features jit`) must match without faulting.
    let j = build(&[
        "(defun-typed (d int64) ((a int64) (b int64)) (/ a b))",
        "(defun-typed (m int64) ((a int64) (b int64)) (mod a b))",
    ]);
    assert_eq!(agree(&j, "d", &[i(i64::MIN), i(-1)]), i(i64::MIN));
    assert_eq!(agree(&j, "m", &[i(i64::MIN), i(-1)]), i(0));
    // Sanity: ordinary division around the boundary still works.
    assert_eq!(agree(&j, "d", &[i(i64::MIN), i(1)]), i(i64::MIN));
    assert_eq!(agree(&j, "d", &[i(i64::MIN), i(2)]), i(i64::MIN / 2));
    // Euclidean MOD values (#280), every edition: the result carries the
    // divisor's sign space, never the dividend's.
    assert_eq!(agree(&j, "m", &[i(-7), i(3)]), i(2));
    assert_eq!(agree(&j, "m", &[i(7), i(-3)]), i(7i64.rem_euclid(-3)));
    assert_eq!(agree(&j, "m", &[i(-7), i(-3)]), i((-7i64).rem_euclid(-3)));
    assert_eq!(
        agree(&j, "m", &[i(i64::MIN), i(3)]),
        i(i64::MIN.rem_euclid(3))
    );
}

#[test]
fn int_overflow_wraps() {
    let j = build(&["(defun-typed (g int64) ((x int64)) (* x x))"]);
    let big = 5_000_000_000i64;
    assert_eq!(agree(&j, "g", &[i(big)]), i(big.wrapping_mul(big)));
}

/// The overflow / div-by-zero flags must agree between the native and the
/// reference (closure) editions — not just the result values. `agree` only
/// compares values, so a flag-only divergence (e.g. `i64::MIN % -1` setting
/// OVERFLOW natively but not in the reference `int_bin`, issue #228) slips past
/// it. This asserts the *flags* match across editions.
#[test]
fn int_bin_condition_flags_agree_across_editions() {
    // (name, args, expect_overflow, expect_div_by_zero)
    let cases: &[(&str, [i64; 2], bool, bool)] = &[
        ("mul", [i64::MAX, i64::MAX], true, false),
        ("add", [i64::MAX, 1], true, false),
        ("sub", [i64::MIN, 1], true, false),
        ("divv", [i64::MIN, -1], true, false), // MIN / -1 overflows
        // #280: MOD is Euclidean — MIN % -1 is exactly 0, NO flag (only DIV
        // wraps there). This row used to expect the flag under the truncated-
        // remainder assumption (the #228 gap).
        ("modd", [i64::MIN, -1], false, false),
        ("divv", [10, 0], false, true), // division by zero
        ("modd", [10, 0], false, true), // remainder by zero
        ("add", [2, 3], false, false),  // ordinary: no flags
        ("divv", [10, 3], false, false),
        ("modd", [10, 3], false, false),
        ("modd", [-7, 3], false, false), // Euclidean: 2, not -1 (#280)
    ];
    let j = build(&[
        "(defun-typed (add int64) ((a int64) (b int64)) (+ a b))",
        "(defun-typed (sub int64) ((a int64) (b int64)) (- a b))",
        "(defun-typed (mul int64) ((a int64) (b int64)) (* a b))",
        "(defun-typed (divv int64) ((a int64) (b int64)) (/ a b))",
        "(defun-typed (modd int64) ((a int64) (b int64)) (mod a b))",
    ]);

    let flags_of = |j: &Jit, name: &str, args: &[i64; 2]| -> JitFlags {
        let (_r, _u, flags) = j
            .call_with_array_writeback(name, &[i(args[0]), i(args[1])])
            .unwrap();
        flags
    };

    for (name, args, want_ovf, want_dbz) in cases {
        j.compile_all();
        let native = flags_of(&j, name, args);
        j.deoptimize_all();
        let reference = flags_of(&j, name, args);
        assert_eq!(
            (native.overflow, native.div_by_zero),
            (reference.overflow, reference.div_by_zero),
            "native vs reference flags disagree for {name}{args:?}: native=({},{}) reference=({},{})",
            native.overflow,
            native.div_by_zero,
            reference.overflow,
            reference.div_by_zero,
        );
        assert_eq!(
            (native.overflow, native.div_by_zero),
            (*want_ovf, *want_dbz),
            "unexpected flags for {name}{args:?}",
        );
    }
}

// --- arithmetic: float -----------------------------------------------------

#[test]
fn float_arithmetic_roundtrips_bits() {
    let j = build(&["(defun-typed (avg float64) ((x float64) (y float64)) (/ (+ x y) 2.0))"]);
    assert_eq!(agree(&j, "avg", &[fl(3.0), fl(5.0)]), fl(4.0));
    assert_eq!(agree(&j, "avg", &[fl(-1.5), fl(2.5)]), fl(0.5));
}

#[test]
fn float_sub_mul_div() {
    let j = build(&["(defun-typed (f float64) ((x float64) (y float64)) (- (* x y) (/ x y)))"]);
    assert_eq!(
        agree(&j, "f", &[fl(6.0), fl(2.0)]),
        fl(6.0 * 2.0 - 6.0 / 2.0)
    );
    assert_eq!(
        agree(&j, "f", &[fl(1.0), fl(4.0)]),
        fl(1.0 * 4.0 - 1.0 / 4.0)
    );
}

#[test]
fn negative_and_fractional_floats() {
    let j = build(&["(defun-typed (id float64) ((x float64)) x)"]);
    for v in [-0.0, 0.25, -123.456, 1e300, -1e-300] {
        assert_eq!(agree(&j, "id", &[fl(v)]), fl(v));
    }
}

// --- comparisons -----------------------------------------------------------

#[test]
fn all_int_comparisons() {
    let j = build(&[
        "(defun-typed (lt bool) ((a int64) (b int64)) (< a b))",
        "(defun-typed (gt bool) ((a int64) (b int64)) (> a b))",
        "(defun-typed (le bool) ((a int64) (b int64)) (<= a b))",
        "(defun-typed (ge bool) ((a int64) (b int64)) (>= a b))",
        "(defun-typed (eq bool) ((a int64) (b int64)) (= a b))",
        "(defun-typed (ne bool) ((a int64) (b int64)) (/= a b))",
    ]);
    assert_eq!(agree(&j, "lt", &[i(2), i(3)]), bo(true));
    assert_eq!(agree(&j, "gt", &[i(2), i(3)]), bo(false));
    assert_eq!(agree(&j, "le", &[i(3), i(3)]), bo(true));
    assert_eq!(agree(&j, "ge", &[i(3), i(4)]), bo(false));
    assert_eq!(agree(&j, "eq", &[i(5), i(5)]), bo(true));
    assert_eq!(agree(&j, "ne", &[i(5), i(5)]), bo(false));
}

#[test]
fn float_comparisons() {
    let j = build(&["(defun-typed (le bool) ((a float64) (b float64)) (<= a b))"]);
    assert_eq!(agree(&j, "le", &[fl(0.5), fl(0.5)]), bo(true));
    assert_eq!(agree(&j, "le", &[fl(0.5), fl(0.25)]), bo(false));
}

// --- boolean logic ---------------------------------------------------------

#[test]
fn and_or_not_truth_tables() {
    let j = build(&[
        "(defun-typed (a2 bool) ((p bool) (q bool)) (and p q))",
        "(defun-typed (o2 bool) ((p bool) (q bool)) (or p q))",
        "(defun-typed (n1 bool) ((p bool)) (not p))",
    ]);
    for (p, q) in [(false, false), (false, true), (true, false), (true, true)] {
        assert_eq!(agree(&j, "a2", &[bo(p), bo(q)]), bo(p && q));
        assert_eq!(agree(&j, "o2", &[bo(p), bo(q)]), bo(p || q));
    }
    assert_eq!(agree(&j, "n1", &[bo(true)]), bo(false));
    assert_eq!(agree(&j, "n1", &[bo(false)]), bo(true));
}

/// Issue #202: `AND`/`OR` are fully variadic special forms in the evaluator
/// (zero or more operands, short-circuiting left to right); the typed
/// checker used to reject anything but exactly 2 operands as a type error,
/// even though the interpreter runs it fine. Covers the vacuous (0-operand),
/// single-operand, and 3+/4-operand (folded) cases via `agree` (differential
/// vs. the interpreter), verifying both the value and the short-circuit
/// result.
#[test]
fn and_or_variadic_arities() {
    let j = build(&[
        "(defun-typed (a0 bool) () (and))",
        "(defun-typed (o0 bool) () (or))",
        "(defun-typed (a1 bool) ((p bool)) (and p))",
        "(defun-typed (o1 bool) ((p bool)) (or p))",
        "(defun-typed (a3 bool) ((p bool) (q bool) (r bool)) (and p q r))",
        "(defun-typed (o4 bool) ((p bool) (q bool) (r bool) (s bool)) (or p q r s))",
    ]);
    // 0-ary: vacuous identity (matches the evaluator's `(and)` = T, `(or)` = NIL).
    assert_eq!(agree(&j, "a0", &[]), bo(true));
    assert_eq!(agree(&j, "o0", &[]), bo(false));
    // 1-ary: passes the single operand's value straight through.
    assert_eq!(agree(&j, "a1", &[bo(true)]), bo(true));
    assert_eq!(agree(&j, "a1", &[bo(false)]), bo(false));
    assert_eq!(agree(&j, "o1", &[bo(true)]), bo(true));
    assert_eq!(agree(&j, "o1", &[bo(false)]), bo(false));
    // 3-ary AND: true only if every operand is true.
    assert_eq!(agree(&j, "a3", &[bo(true), bo(true), bo(true)]), bo(true));
    assert_eq!(agree(&j, "a3", &[bo(true), bo(false), bo(true)]), bo(false));
    assert_eq!(agree(&j, "a3", &[bo(false), bo(true), bo(true)]), bo(false));
    // 4-ary OR: true if any operand is true.
    assert_eq!(
        agree(&j, "o4", &[bo(false), bo(false), bo(false), bo(false)]),
        bo(false)
    );
    assert_eq!(
        agree(&j, "o4", &[bo(false), bo(false), bo(false), bo(true)]),
        bo(true)
    );
    assert_eq!(
        agree(&j, "o4", &[bo(true), bo(false), bo(false), bo(false)]),
        bo(true)
    );
}

/// The folded representation (`(and a b c)` -> `And(a, And(b, c))`) must
/// still short-circuit left to right exactly like the variadic evaluator —
/// not evaluate every operand regardless of an earlier falsy one. Checked
/// via the interpreter's execution trace (a later operand that isn't
/// reached leaves no step), mirroring the existing
/// `trace_short_circuits_leave_no_step` test for the 2-ary case.
#[test]
fn and_or_variadic_short_circuits() {
    let j = build(&["(defun-typed (sc bool) ((p bool) (q bool) (r bool)) (and p q r))"]);
    let (val, log) = j
        .trace_call("SC", &[bo(false), bo(true), bo(true)])
        .unwrap();
    assert_eq!(val, bo(false));
    // Only `p` (the first `var` step) should be evaluated; `q`/`r` short-circuit away.
    let var_steps = log.iter().filter(|s| s.op == "var").count();
    assert_eq!(
        var_steps, 1,
        "and should short-circuit at the first falsy operand: {log:?}"
    );
}

#[test]
fn range_predicates_compose_logic_and_comparison() {
    let j = build(&[
        "(defun-typed (between bool) ((x int64) (lo int64) (hi int64)) (and (>= x lo) (<= x hi)))",
        "(defun-typed (outside bool) ((x int64) (lo int64) (hi int64)) (or (< x lo) (> x hi)))",
    ]);
    assert_eq!(agree(&j, "between", &[i(5), i(1), i(10)]), bo(true));
    assert_eq!(agree(&j, "between", &[i(11), i(1), i(10)]), bo(false));
    assert_eq!(agree(&j, "outside", &[i(11), i(1), i(10)]), bo(true));
    assert_eq!(agree(&j, "outside", &[i(5), i(1), i(10)]), bo(false));
}

#[test]
fn bool_literals() {
    let j = build(&["(defun-typed (pick bool) ((p bool)) (if p false true))"]);
    assert_eq!(agree(&j, "pick", &[bo(true)]), bo(false));
    assert_eq!(agree(&j, "pick", &[bo(false)]), bo(true));
}

// --- if --------------------------------------------------------------------

#[test]
fn if_branches() {
    let j = build(&[
        "(defun-typed (absish int64) ((x int64)) (if (< x 0) (- 0 x) x))",
        "(defun-typed (maxi int64) ((a int64) (b int64)) (if (> a b) a b))",
        "(defun-typed (sign int64) ((x int64)) (if (> x 0) 1 (if (< x 0) (- 0 1) 0)))",
    ]);
    assert_eq!(agree(&j, "absish", &[i(-9)]), i(9));
    assert_eq!(agree(&j, "absish", &[i(9)]), i(9));
    assert_eq!(agree(&j, "maxi", &[i(3), i(8)]), i(8));
    assert_eq!(agree(&j, "sign", &[i(-4)]), i(-1));
    assert_eq!(agree(&j, "sign", &[i(0)]), i(0));
    assert_eq!(agree(&j, "sign", &[i(4)]), i(1));
}

// --- setq/while/for (loops & mutation) --------------------------------------

#[test]
fn setq_mutates_local_slot() {
    let j = build(&[
        "(defun-typed (f int64) ((x int64)) (let-typed ((y int64 x)) (setq y (+ y 1)) y))",
    ]);
    assert_eq!(agree(&j, "f", &[i(5)]), i(6));
}

#[test]
fn setq_on_param_updates_it_for_rest_of_body() {
    let j = build(&["(defun-typed (f int64) ((x int64)) (progn (setq x (* x 2)) (+ x 1)))"]);
    assert_eq!(agree(&j, "f", &[i(5)]), i(11));
}

#[test]
fn setq_unbound_variable_is_a_define_error() {
    let e = def_err("(defun-typed (f int64) ((x int64)) (setq y 1))");
    assert!(e.contains("not a local binding"), "unexpected error: {e}");
}

#[test]
fn while_loop_accumulates() {
    let j = build(&["(defun-typed (count-while int64) ((n int64)) \
           (let-typed ((i int64 0) (acc int64 0)) \
             (while (< i n) (setq acc (+ acc i)) (setq i (+ i 1))) acc))"]);
    assert_eq!(agree(&j, "count-while", &[i(10)]), i(45));
    assert_eq!(agree(&j, "count-while", &[i(0)]), i(0));
    assert_eq!(agree(&j, "count-while", &[i(1)]), i(0));
}

#[test]
fn while_false_test_never_runs_body() {
    let j = build(&["(defun-typed (f int64) ((n int64)) \
           (let-typed ((acc int64 0)) (while (> 0 1) (setq acc 999)) acc))"]);
    assert_eq!(agree(&j, "f", &[i(1)]), i(0));
}

#[test]
fn for_ascending_sums_inclusive() {
    let j = build(&["(defun-typed (sum-to int64) ((n int64)) \
           (let-typed ((acc int64 0)) (for (i 1 n) (setq acc (+ acc i))) acc))"]);
    assert_eq!(agree(&j, "sum-to", &[i(100)]), i(5050));
    assert_eq!(agree(&j, "sum-to", &[i(1)]), i(1));
    // start > end: zero iterations.
    assert_eq!(agree(&j, "sum-to", &[i(0)]), i(0));
}

#[test]
fn for_descending_with_negative_step() {
    let j = build(&["(defun-typed (down int64) ((n int64)) \
           (let-typed ((acc int64 0)) (for (i n 1 -1) (setq acc (+ acc i))) acc))"]);
    assert_eq!(agree(&j, "down", &[i(5)]), i(15));
}

#[test]
fn for_custom_step_counts_iterations() {
    let j = build(&["(defun-typed (evens int64) ((n int64)) \
           (let-typed ((acc int64 0)) (for (i 0 n 2) (setq acc (+ acc 1))) acc))"]);
    // 0, 2, 4, 6, 8, 10 inclusive -> 6 iterations.
    assert_eq!(agree(&j, "evens", &[i(10)]), i(6));
}

#[test]
fn for_zero_step_errors_in_every_edition() {
    let j = build(&["(defun-typed (f int64) ((n int64)) \
           (let-typed ((acc int64 0)) (for (i 1 n 0) (setq acc (+ acc i))) acc))"]);
    j.compile_all();
    assert_eq!(
        j.call("f", &[i(5)]).unwrap_err(),
        "for step must be non-zero"
    );
    j.deoptimize_all();
    assert_eq!(
        j.call("f", &[i(5)]).unwrap_err(),
        "for step must be non-zero"
    );
    j.compile_all();
}

#[test]
fn for_loop_var_is_scoped_to_the_body() {
    // The loop variable's slot can be reused by a later let binding without
    // interference, proving `for` truncates its scope on exit.
    let j = build(&["(defun-typed (f int64) ((n int64)) \
           (progn (for (i 1 n) i) (let-typed ((z int64 42)) z)))"]);
    assert_eq!(agree(&j, "f", &[i(3)]), i(42));
}

#[test]
fn for_overflow_breaks_the_loop() {
    // A step that overflows i64 on the first increment stops the loop after
    // exactly one iteration (special_forms.rs::eval_for's checked_add
    // contract) rather than panicking or wrapping into an infinite loop.
    let j = build(&["(defun-typed (f int64) () \
           (let-typed ((acc int64 0)) \
             (for (i 9223372036854775807 9223372036854775807 1) (setq acc (+ acc 1))) \
             acc))"]);
    assert_eq!(agree(&j, "f", &[]), i(1));
}

// --- let-typed -------------------------------------------------------------

#[test]
fn let_single_and_sequential() {
    let j = build(&[
        "(defun-typed (poly int64) ((x int64)) (let-typed ((y int64 (* x x))) (+ y x)))",
        "(defun-typed (seq int64) ((x int64)) \
           (let-typed ((y int64 (+ x 1)) (z int64 (* y 2))) (+ y z)))",
    ]);
    assert_eq!(agree(&j, "poly", &[i(3)]), i(12));
    // x=4 -> y=5 -> z=10 -> 15
    assert_eq!(agree(&j, "seq", &[i(4)]), i(15));
}

#[test]
fn let_shadowing_uses_innermost() {
    let j = build(&["(defun-typed (sh int64) ((x int64)) \
           (let-typed ((x int64 (+ x 1))) (let-typed ((x int64 (* x 10))) x)))"]);
    // x=5 -> 6 -> 60
    assert_eq!(agree(&j, "sh", &[i(5)]), i(60));
}

#[test]
fn nested_let_in_branch_does_not_corrupt_outer_slot() {
    let src = "(defun-typed (f int64) ((c int64)) \
               (let-typed ((a int64 (if (> c 0) (let-typed ((tmp int64 1)) tmp) 0))) \
                 (+ a 100)))";
    let j = build(&[src]);
    assert_eq!(agree(&j, "f", &[i(5)]), i(101));
    assert_eq!(agree(&j, "f", &[i(-5)]), i(100));
}

#[test]
fn let_with_float() {
    let j = build(&["(defun-typed (hyp float64) ((a float64) (b float64)) \
           (let-typed ((s float64 (+ (* a a) (* b b)))) s))"]);
    assert_eq!(agree(&j, "hyp", &[fl(3.0), fl(4.0)]), fl(25.0));
}

// --- inference: optional-annotation let-typed (#135) -----------------------

#[test]
fn let_infers_type_from_initializer() {
    // `(name init)` omits the type — it is inferred from the initializer. The
    // int and float versions are the *same surface form*, monomorphized by
    // inference to different representations.
    let j = build(&[
        "(defun-typed (poly int64) ((x int64)) (let-typed ((y (* x x))) (+ y x)))",
        "(defun-typed (hyp float64) ((a float64) (b float64)) \
           (let-typed ((s (+ (* a a) (* b b)))) s))",
    ]);
    assert_eq!(agree(&j, "poly", &[i(3)]), i(12));
    assert_eq!(agree(&j, "hyp", &[fl(3.0), fl(4.0)]), fl(25.0));
}

#[test]
fn let_inferred_binding_flows_into_typed_context() {
    // An inferred `char` binding must still type-check as a `char` downstream
    // (here, fed back through `char-code`), proving the inferred type — not a
    // default — is what propagates.
    let j = build(&["(defun-typed (f int64) ((n int64)) \
           (let-typed ((c (code-char n))) (char-code c)))"]);
    assert_eq!(agree(&j, "f", &[i(65)]), i(65));
    assert_eq!(agree(&j, "f", &[i(200)]), i(200));
    // A code point outside the typed byte-`char` range now errors rather than
    // silently masking (issue #281), in every edition.
    j.compile_all();
    assert_eq!(
        j.call("f", &[i(321)]).unwrap_err(),
        "CODE-CHAR: code point 321 is outside the typed char range 0-255"
    );
    j.deoptimize_all();
    assert_eq!(
        j.call("f", &[i(321)]).unwrap_err(),
        "CODE-CHAR: code point 321 is outside the typed char range 0-255"
    );
    j.compile_all();
}

#[test]
fn let_inferred_and_explicit_mix() {
    let j = build(&["(defun-typed (g int64) ((x int64)) \
           (let-typed ((y (+ x 1)) (z int64 (* y 2))) (+ y z)))"]);
    // x=4 -> y=5 -> z=10 -> 15
    assert_eq!(agree(&j, "g", &[i(4)]), i(15));
}

#[test]
fn explicit_annotation_agreeing_with_inference_is_accepted() {
    // The pin matches what inference would derive — accepted.
    let j = build(&["(defun-typed (h float64) ((x float64)) \
           (let-typed ((y float64 (* x x))) y))"]);
    assert_eq!(agree(&j, "h", &[fl(2.5)]), fl(6.25));
}

#[test]
fn explicit_annotation_conflicting_with_inference_is_rejected() {
    // The initializer is `int64` but the binding is pinned `float64`: the pin
    // and the inferred type fail to unify.
    let err = def_err(
        "(defun-typed (f int64) ((x int64)) \
           (let-typed ((y float64 (* x x))) x))",
    );
    assert!(
        err.contains("declared") && err.contains("init"),
        "got: {err}"
    );
}

#[test]
fn inferred_binding_used_at_two_types_is_rejected() {
    // `y` is inferred from an int initializer, then used where a float is
    // required — a conflict surfaced through the binding's resolved type.
    let err = def_err(
        "(defun-typed (f float64) ((a float64)) \
           (let-typed ((y (+ 1 2))) (+ y a)))",
    );
    assert!(err.contains("operands disagree"), "got: {err}");
}

// --- arrays: inference + flat representation (#137/#138) --------------------

#[test]
fn array_new_store_fetch_length_roundtrip() {
    // Build a 3-int array, fill it, read it back, and report its length. The
    // element type is inferred (int64) from the `store` of an int.
    let j = build(&[
        "(defun-typed (build int64) ((x int64)) \
           (let-typed ((a (array 3))) \
             (store a 0 x) (store a 1 (* x 2)) (store a 2 (* x 3)) \
             (+ (fetch a 0) (+ (fetch a 1) (fetch a 2)))))",
        "(defun-typed (len3 int64) () (let-typed ((a (array 3))) (array-length* a)))",
    ]);
    assert_eq!(agree(&j, "build", &[i(5)]), i(5 + 10 + 15));
    assert_eq!(agree(&j, "len3", &[]), i(3));
}

#[test]
fn array_out_of_bounds_is_panic_free() {
    // An out-of-range access stays memory-safe (no panic, no UB) — the runtime
    // still bounds-checks and substitutes a fetch → 0 / store → no-op *inside*
    // the call — but it now records the evaluator's index error and the
    // membrane raises it once the call returns (issue #282), instead of
    // silently returning the substitute value. In-bounds access is unchanged.
    let j = build(&[
        "(defun-typed (oobget int64) ((idx int64)) \
           (let-typed ((a (array 2))) (store a 0 7) (store a 1 9) (fetch a idx)))",
        "(defun-typed (oobset int64) ((idx int64)) \
           (let-typed ((a (array 2))) (store a idx 42) (+ (fetch a 0) (fetch a 1))))",
    ]);
    // In-bounds: still works, and both editions agree.
    assert_eq!(agree(&j, "oobget", &[i(0)]), i(7));

    // Out-of-range: every edition errors with the evaluator's exact message.
    for (name, args, expected) in [
        ("oobget", i(5), "fetch: index 5 out of bounds (length 2)"),
        (
            "oobget",
            i(-1),
            "FETCH: index must be a non-negative integer, got -1",
        ),
        ("oobset", i(9), "store: index 9 out of bounds (length 2)"),
    ] {
        j.compile_all();
        assert_eq!(
            j.call(name, std::slice::from_ref(&args)).unwrap_err(),
            expected,
            "compiled {name} OOB message"
        );
        j.deoptimize_all();
        assert_eq!(
            j.call(name, &[args]).unwrap_err(),
            expected,
            "interpreted {name} OOB message"
        );
    }
    j.compile_all();
}

/// Issue #216: `Jit::call`'s plain path (used by `agree`/`build` throughout
/// this file) never wrote array mutations back to the caller -- `store`
/// only ever touched a throwaway arena copy. `call_with_array_writeback`
/// is the fix's primitive: it must report the post-call contents of a
/// flat-scalar-array argument, whether or not the callee mutated it.
#[test]
fn call_with_array_writeback_reports_post_call_array_contents() {
    let j =
        build(&["(defun-typed (bump int64) ((a (array int64))) (store a 0 (+ (fetch a 0) 1)))"]);
    let (result, updated, _flags) = j
        .call_with_array_writeback("BUMP", &[ints(&[10, 20, 30])])
        .unwrap();
    assert_eq!(result, i(11));
    assert_eq!(updated, vec![Some(ints(&[11, 20, 30]))]);

    // A non-array argument must report `None` (not eligible).
    let j2 = build(&["(defun-typed (sq int64) ((x int64)) (* x x))"]);
    let (r2, u2, _) = j2.call_with_array_writeback("SQ", &[i(5)]).unwrap();
    assert_eq!(r2, i(25));
    assert_eq!(u2, vec![None]);
}

/// Issue #216 x #133 Tier 2a: an array parameter threaded through a
/// *cross-function tail call* must still write back the whole chain's
/// final state, not just the outer function's own mutation. The array
/// argument is one arena-pointer word forwarded unchanged through every
/// tail hop (never re-copied -- only a *top-level* argument gets its own
/// arena buffer, at `to_word`), so `g`'s mutation and `f`'s mutation land
/// in the same buffer that `call_inner` reads back after the whole
/// trampoline drains.
#[test]
fn array_writeback_sees_mutations_from_the_whole_tail_call_chain() {
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    j.declare("G216", &[("A", Ty::Array(Box::new(Ty::Int64)))], Ty::Int64);
    j.define(
        &read(
            "(defun-typed (f216 int64) ((a (array int64))) (store a 0 42) (g216 a))",
            &env,
        )
        .unwrap(),
    )
    .unwrap();
    j.define(
        &read(
            "(defun-typed (g216 int64) ((a (array int64))) (store a 1 99))",
            &env,
        )
        .unwrap(),
    )
    .unwrap();
    let (result, updated, _) = j
        .call_with_array_writeback("F216", &[ints(&[1, 2, 3])])
        .unwrap();
    assert_eq!(result, i(99));
    assert_eq!(updated, vec![Some(ints(&[42, 99, 3]))]);
}

/// `Jit::call_lisp` (the public embedder API, `src/jit/registry.rs`) must
/// write a mutated array back into the caller's `LispVal::Array` in place
/// too, mirroring the interpreter's own typed membrane
/// (`make_typed_native`). Confirms the `Rc` identity is preserved (a second
/// clone of the same `LispVal::Array` sees the update) rather than the
/// argument being silently replaced with a disconnected new array.
#[test]
fn call_lisp_writes_back_array_mutation_in_place() {
    let j =
        build(&["(defun-typed (bump int64) ((a (array int64))) (store a 0 (+ (fetch a 0) 1)))"]);
    let arr = LispVal::Array(crate::Shared::new(crate::SharedCell::new(vec![
        LispVal::Number(10),
        LispVal::Number(20),
    ])));
    let alias = arr.clone(); // same Rc -- must see the mutation too.
    let result = j.call_lisp("BUMP", std::slice::from_ref(&arr)).unwrap();
    assert_eq!(result, LispVal::Number(11));
    let LispVal::Array(rc) = &alias else {
        unreachable!()
    };
    assert_eq!(
        rc.borrow().clone(),
        vec![LispVal::Number(11), LispVal::Number(20)]
    );
}

#[test]
fn array_param_element_inferred_from_body() {
    // `a` is declared with the bare `array` keyword; its element type is
    // inferred to int64 from the `fetch`+arithmetic in the body.
    let j = build(&["(defun-typed (first-plus int64) ((a array) (k int64)) (+ (fetch a 0) k))"]);
    assert_eq!(agree(&j, "first-plus", &[ints(&[10, 20, 30]), i(5)]), i(15));
}

#[test]
fn array_int_sum_recursive_kernel() {
    // sum-array via recursion over a pinned (array int64).
    let j = build(&[
        "(defun-typed (suml int64) ((a (array int64)) (i int64)) \
           (if (= i (array-length* a)) 0 (+ (fetch a i) (suml a (+ i 1)))))",
        "(defun-typed (sum int64) ((a (array int64))) (suml a 0))",
    ]);
    assert_eq!(agree(&j, "sum", &[ints(&[1, 2, 3, 4, 5])]), i(15));
    assert_eq!(agree(&j, "sum", &[ints(&[])]), i(0));
}

#[test]
fn array_float_dot_product_kernel() {
    let j = build(&[
        "(defun-typed (dotl float64) ((a (array float64)) (b (array float64)) (i int64)) \
           (if (= i (array-length* a)) 0.0 \
             (+ (* (fetch a i) (fetch b i)) (dotl a b (+ i 1)))))",
        "(defun-typed (dot float64) ((a (array float64)) (b (array float64))) (dotl a b 0))",
    ]);
    assert_eq!(
        agree(
            &j,
            "dot",
            &[floats(&[1.0, 2.0, 3.0]), floats(&[4.0, 5.0, 6.0])]
        ),
        fl(32.0)
    );
}

#[test]
fn same_source_monomorphizes_to_int_and_float_arrays() {
    // Identical `(array 1)`/`store`/`fetch` source, but inference picks int64 in
    // one function and float64 in the other from the value stored.
    let j = build(&[
        "(defun-typed (boxi int64) ((x int64)) (let-typed ((a (array 1))) (store a 0 x) (fetch a 0)))",
        "(defun-typed (boxf float64) ((x float64)) (let-typed ((a (array 1))) (store a 0 x) (fetch a 0)))",
    ]);
    assert_eq!(agree(&j, "boxi", &[i(7)]), i(7));
    assert_eq!(agree(&j, "boxf", &[fl(2.5)]), fl(2.5));
}

#[test]
fn string_is_array_of_char_levenshtein() {
    // A string is `(array char)`: native byte indexing + comparison + recursion.
    let j = build(&[
        "(defun-typed (min3 int64) ((a int64) (b int64) (c int64)) \
           (if (<= a b) (if (<= a c) a c) (if (<= b c) b c)))",
        "(defun-typed (lev int64) ((a (array char)) (b (array char)) (i int64) (j int64)) \
           (if (= i (array-length* a)) (- (array-length* b) j) \
             (if (= j (array-length* b)) (- (array-length* a) i) \
               (if (= (fetch a i) (fetch b j)) \
                   (lev a b (+ i 1) (+ j 1)) \
                   (+ 1 (min3 (lev a b (+ i 1) j) \
                              (lev a b i (+ j 1)) \
                              (lev a b (+ i 1) (+ j 1))))))))",
        "(defun-typed (edit int64) ((a (array char)) (b (array char))) (lev a b 0 0))",
    ]);
    assert_eq!(
        agree(&j, "edit", &[chars("kitten"), chars("sitting")]),
        i(3)
    );
    assert_eq!(agree(&j, "edit", &[chars("abc"), chars("abc")]), i(0));
    assert_eq!(agree(&j, "edit", &[chars(""), chars("xyz")]), i(3));
}

#[test]
fn char_array_fetch_resolves_element_to_char() {
    // `(fetch s i)` on a char array is a char: feeding it to `char-code`
    // type-checks only because the element resolved to `char`.
    let j = build(
        &["(defun-typed (code-at int64) ((s (array char)) (i int64)) \
           (char-code (fetch s i)))"],
    );
    assert_eq!(agree(&j, "code-at", &[chars("ABC"), i(0)]), i(65));
    assert_eq!(agree(&j, "code-at", &[chars("ABC"), i(2)]), i(67));
}

#[test]
fn array_returned_across_membrane() {
    // A typed function may build and return an array; the membrane copies it out.
    let j = build(&["(defun-typed (iota (array int64)) ((n int64)) \
           (let-typed ((a (array n))) (store a 0 0) a))"]);
    // (only element 0 written; rest zero-initialized)
    assert_eq!(j.call("iota", &[i(3)]).unwrap(), ints(&[0, 0, 0]));
}

#[test]
fn array_element_type_conflict_rejected() {
    // Storing an int then a float into the same array is a def-time element clash.
    let err = def_err(
        "(defun-typed (bad int64) () \
           (let-typed ((a (array 2))) (store a 0 1) (store a 1 2.0) 0))",
    );
    assert!(
        err.contains("operands disagree")
            || err.contains("cannot unify")
            || err.contains("does not match"),
        "got: {err}"
    );
}

#[test]
fn string_membrane_roundtrip_via_call_lisp() {
    use crate::reader::read;
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    // length of a string (array char) through the LispVal membrane.
    let f = read(
        "(defun-typed (slen int64) ((s (array char))) (array-length* s))",
        &env,
    )
    .unwrap();
    j.define(&f).unwrap();
    let arg = LispVal::String("hello".to_string());
    assert_eq!(j.call_lisp("slen", &[arg]).unwrap(), LispVal::Number(5));
}

// --- typed structs ---------------------------------------------------------

/// Build a Jit with struct definitions then function definitions.
fn build_with(structs: &[&str], funcs: &[&str]) -> Jit {
    use crate::reader::read;
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    for s in structs {
        let form = read(s, &env).expect("read failed");
        j.define_struct(&form)
            .unwrap_or_else(|e| panic!("defstruct `{s}` failed: {e}"));
    }
    for s in funcs {
        let form = read(s, &env).expect("read failed");
        j.define(&form)
            .unwrap_or_else(|e| panic!("define `{s}` failed: {e}"));
    }
    j
}

#[test]
fn struct_constructor_and_accessors() {
    let j = build_with(&["(defstruct-typed Point (x int64) (y int64))"], &[]);
    let p = j.call("make-point", &[i(3), i(4)]).unwrap();
    assert_eq!(p, Value::Struct(vec![i(3), i(4)]));
    assert_eq!(j.call("point-x", std::slice::from_ref(&p)).unwrap(), i(3));
    assert_eq!(j.call("point-y", std::slice::from_ref(&p)).unwrap(), i(4));
}

#[test]
fn struct_setter_returns_value() {
    let j = build_with(&["(defstruct-typed Cell (v int64))"], &[]);
    let c = j.call("make-cell", &[i(7)]).unwrap();
    // The setter returns the stored value; the materialized struct copy at the
    // membrane is independent, so re-read through the same buffer is in-call only.
    assert_eq!(j.call("set-cell-v", &[c, i(9)]).unwrap(), i(9));
}

#[test]
fn struct_used_as_typed_parameter() {
    // A struct name is a usable parameter type; the body calls generated
    // accessors. Mixed field types (int64 + float64) exercise per-field reads.
    let j = build_with(
        &["(defstruct-typed Vec2 (x float64) (y float64))"],
        &["(defun-typed (norm2 float64) ((v Vec2)) \
               (+ (* (vec2-x v) (vec2-x v)) (* (vec2-y v) (vec2-y v))))"],
    );
    j.compile_all();
    let v = j.call("make-vec2", &[fl(3.0), fl(4.0)]).unwrap();
    assert_eq!(j.call("norm2", std::slice::from_ref(&v)).unwrap(), fl(25.0));
    // Differential: interpreter agrees with the compiled edition.
    j.deoptimize_all();
    assert_eq!(j.call("norm2", &[v]).unwrap(), fl(25.0));
}

#[test]
fn struct_constructed_and_consumed_in_one_typed_function() {
    // make + accessors all inside one typed body (no membrane round-trip): the
    // struct pointer stays in the call arena the whole time.
    let j = build_with(
        &["(defstruct-typed Pair (a int64) (b int64))"],
        &["(defun-typed (sum-pair int64) ((a int64) (b int64)) \
             (let-typed ((p (make-pair a b))) (+ (pair-a p) (pair-b p))))"],
    );
    assert_eq!(agree(&j, "sum-pair", &[i(10), i(32)]), i(42));
}

#[test]
fn let_typed_accepts_nominal_struct_annotations() {
    let j = build_with(
        &["(defstruct-typed Box (n int64))"],
        &["(defun-typed (unwrap-box int64) ((n int64)) \
             (let-typed ((b Box (make-box n))) (box-n b)))"],
    );
    assert_eq!(agree(&j, "unwrap-box", &[i(42)]), i(42));
}

#[test]
fn let_typed_nominal_struct_annotation_rejects_plain_value() {
    let mut j = build_with(&["(defstruct-typed Box (n int64))"], &[]);
    let err = j
        .define(
            &crate::reader::read(
                "(defun-typed (bad int64) () \
                   (let-typed ((b Box 7)) (box-n b)))",
                &Environment::new_with_builtins(),
            )
            .unwrap(),
        )
        .unwrap_err();
    assert!(
        err.contains("declared") && err.contains("init"),
        "got: {err}"
    );
}

#[test]
fn struct_field_type_is_checked() {
    // Passing a float where the field/accessor expects int64 is rejected.
    let mut j = build_with(&["(defstruct-typed Box (n int64))"], &[]);
    let err = j
        .define(
            &crate::reader::read(
                "(defun-typed (bad float64) ((b Box)) (box-n b))",
                &Environment::new_with_builtins(),
            )
            .unwrap(),
        )
        .unwrap_err();
    assert!(err.contains("declared return"), "got: {err}");
}

// --- HM inference of un-annotated functions (jit-optimize / #134) -----------

#[test]
fn infer_untyped_types_a_concrete_numeric_function() {
    use crate::reader::read;
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    // `(defun inc (n) (+ n 1))` — `1` pins `n` to int64, so it fully infers.
    let body = [read("(+ n 1)", &env).unwrap()];
    let id = j
        .infer_untyped("INC", &["N".to_string()], &body)
        .expect("should infer int64 -> int64");
    assert_eq!(j.name_of(id).as_deref(), Some("INC"));
    assert_eq!(j.signature("INC").unwrap(), (vec![Ty::Int64], Ty::Int64));
    assert_eq!(j.call("INC", &[i(41)]).unwrap(), i(42));
}

#[test]
fn infer_untyped_rejects_and_rolls_back_polymorphic() {
    use crate::reader::read;
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    // `(* x x)` is ambiguous (int or float): not monomorphic ⇒ rejected, and the
    // registry is left clean (no `SQ` registered).
    let body = [read("(* x x)", &env).unwrap()];
    assert!(j.infer_untyped("SQ", &["X".to_string()], &body).is_err());
    assert!(j.id("SQ").is_none(), "failed inference must not register");
}

#[test]
fn infer_untyped_rejects_untyped_call_island_escape() {
    use crate::reader::read;
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    // `cons` is not a typed function: the body escapes the typed island.
    let body = [read("(cons n n)", &env).unwrap()];
    assert!(j.infer_untyped("F", &["N".to_string()], &body).is_err());
    assert!(j.id("F").is_none());
}

// --- self-recursion --------------------------------------------------------

#[test]
fn factorial() {
    let j = build(&["(defun-typed (fact int64) ((n int64)) (if (<= n 1) 1 (* n (fact (- n 1)))))"]);
    assert_eq!(agree(&j, "fact", &[i(5)]), i(120));
    assert_eq!(agree(&j, "fact", &[i(10)]), i(3_628_800));
    assert_eq!(agree(&j, "fact", &[i(20)]), i(2_432_902_008_176_640_000));
}

#[test]
fn fibonacci() {
    let j = build(&[
        "(defun-typed (fib int64) ((n int64)) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2)))))",
    ]);
    for (n, want) in [(0, 0), (1, 1), (10, 55), (20, 6765), (25, 75025)] {
        assert_eq!(agree(&j, "fib", &[i(n)]), i(want), "fib({n})");
    }
}

#[test]
fn gcd_euclid() {
    let j = build(&[
        "(defun-typed (gcd int64) ((a int64) (b int64)) (if (= b 0) a (gcd b (mod a b))))",
    ]);
    assert_eq!(agree(&j, "gcd", &[i(48), i(36)]), i(12));
    assert_eq!(agree(&j, "gcd", &[i(17), i(5)]), i(1));
    assert_eq!(agree(&j, "gcd", &[i(1071), i(462)]), i(21));
}

#[test]
fn integer_power() {
    let j = build(&[
        "(defun-typed (pw int64) ((base int64) (e int64)) (if (= e 0) 1 (* base (pw base (- e 1)))))",
    ]);
    assert_eq!(agree(&j, "pw", &[i(2), i(10)]), i(1024));
    assert_eq!(agree(&j, "pw", &[i(3), i(5)]), i(243));
}

#[test]
fn ackermann() {
    let j = build(&["(defun-typed (ack int64) ((m int64) (n int64)) \
           (if (= m 0) (+ n 1) \
               (if (= n 0) (ack (- m 1) 1) \
                   (ack (- m 1) (ack m (- n 1))))))"]);
    assert_eq!(agree(&j, "ack", &[i(2), i(3)]), i(9));
    assert_eq!(agree(&j, "ack", &[i(3), i(3)]), i(61));
}

#[test]
fn deep_recursion_sum_to_n() {
    // Non-tail recursion this deep overflows the default 2 MB test stack (the
    // real REPL runs on a large stack via `with_large_stack`), so give it room.
    std::thread::Builder::new()
        .stack_size(64 * 1024 * 1024)
        .spawn(|| {
            let j = build(&[
                "(defun-typed (sum int64) ((n int64)) (if (= n 0) 0 (+ n (sum (- n 1)))))",
            ]);
            assert_eq!(agree(&j, "sum", &[i(5000)]), i(5000 * 5001 / 2));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn float_recursive_power() {
    let j = build(&[
        "(defun-typed (fpow float64) ((b float64) (e int64)) (if (= e 0) 1.0 (* b (fpow b (- e 1)))))",
    ]);
    assert_eq!(agree(&j, "fpow", &[fl(2.0), i(10)]), fl(1024.0));
    assert_eq!(agree(&j, "fpow", &[fl(0.5), i(3)]), fl(0.125));
}

// --- cross-function calls --------------------------------------------------

#[test]
fn cross_function_calls() {
    let j = build(&[
        "(defun-typed (dbl int64) ((x int64)) (* x 2))",
        "(defun-typed (quad int64) ((x int64)) (dbl (dbl x)))",
        "(defun-typed (sq int64) ((x int64)) (* x x))",
        "(defun-typed (sum-sq int64) ((a int64) (b int64)) (+ (sq a) (sq b)))",
    ]);
    assert_eq!(agree(&j, "quad", &[i(5)]), i(20));
    assert_eq!(agree(&j, "sum-sq", &[i(3), i(4)]), i(25));
}

#[test]
fn call_across_types() {
    let j = build(&[
        "(defun-typed (is-even bool) ((n int64)) (= (mod n 2) 0))",
        "(defun-typed (classify int64) ((n int64)) (if (is-even n) 0 1))",
    ]);
    assert_eq!(agree(&j, "classify", &[i(4)]), i(0));
    assert_eq!(agree(&j, "classify", &[i(7)]), i(1));
}

// --- mutual recursion via forward declaration ------------------------------

#[test]
fn mutual_recursion_even_odd() {
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    j.declare("EVEN?", &[("N", Ty::Int64)], Ty::Bool);
    j.declare("ODD?", &[("N", Ty::Int64)], Ty::Bool);
    j.define(
        &read(
            "(defun-typed (even? bool) ((n int64)) (if (= n 0) true (odd? (- n 1))))",
            &env,
        )
        .unwrap(),
    )
    .unwrap();
    j.define(
        &read(
            "(defun-typed (odd? bool) ((n int64)) (if (= n 0) false (even? (- n 1))))",
            &env,
        )
        .unwrap(),
    )
    .unwrap();

    assert_eq!(agree(&j, "even?", &[i(10)]), bo(true));
    assert_eq!(agree(&j, "even?", &[i(7)]), bo(false));
    assert_eq!(agree(&j, "odd?", &[i(7)]), bo(true));
    assert_eq!(agree(&j, "odd?", &[i(0)]), bo(false));
}

/// Issue #133 Tier 2a: `even?`/`odd?` tail-call *each other* (not
/// themselves), so Tier 1's in-function loop never fires for these — before
/// Tier 2a, each mutual call was an ordinary recursive `Ctx::call`, growing
/// the native Rust stack by one frame per iteration and segfaulting at
/// depth. This must now run at a depth deep enough to blow any bounded
/// per-call-frame stack, on the interpreter/closure path *and* the native
/// Cranelift path, on the default thread stack (no `with_large_stack`).
#[test]
fn tier2a_mutual_tail_recursion_runs_on_default_stack() {
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    j.declare("EVEN2?", &[("N", Ty::Int64)], Ty::Bool);
    j.declare("ODD2?", &[("N", Ty::Int64)], Ty::Bool);
    j.define(
        &read(
            "(defun-typed (even2? bool) ((n int64)) (if (= n 0) true (odd2? (- n 1))))",
            &env,
        )
        .unwrap(),
    )
    .unwrap();
    j.define(
        &read(
            "(defun-typed (odd2? bool) ((n int64)) (if (= n 0) false (even2? (- n 1))))",
            &env,
        )
        .unwrap(),
    )
    .unwrap();

    // Native Cranelift path.
    j.compile_all();
    assert_eq!(j.call("EVEN2?", &[i(50_000_000)]).unwrap(), bo(true));
    assert_eq!(j.call("EVEN2?", &[i(50_000_001)]).unwrap(), bo(false));
    assert_eq!(j.call("ODD2?", &[i(50_000_001)]).unwrap(), bo(true));

    // Interpreter (eval_core) path specifically.
    j.deoptimize_all();
    assert_eq!(j.call("EVEN2?", &[i(10_000_000)]).unwrap(), bo(true));
    assert_eq!(j.call("ODD2?", &[i(10_000_001)]).unwrap(), bo(true));
}

/// A three-function tail cycle (not just a two-function ping-pong), to
/// confirm the trampoline generalizes beyond a single mutual pair: `a`
/// tail-calls `b`, `b` tail-calls `c`, `c` tail-calls `a`, decrementing once
/// per full cycle.
#[test]
fn tier2a_three_function_tail_cycle() {
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    j.declare("CYCLE-A", &[("N", Ty::Int64)], Ty::Int64);
    j.declare("CYCLE-B", &[("N", Ty::Int64)], Ty::Int64);
    j.declare("CYCLE-C", &[("N", Ty::Int64)], Ty::Int64);
    for src in [
        "(defun-typed (cycle-a int64) ((n int64)) (if (= n 0) 111 (cycle-b (- n 1))))",
        "(defun-typed (cycle-b int64) ((n int64)) (cycle-c n))",
        "(defun-typed (cycle-c int64) ((n int64)) (cycle-a n))",
    ] {
        j.define(&read(src, &env).unwrap()).unwrap();
    }
    assert_eq!(agree(&j, "cycle-a", &[i(3_000_000)]), i(111));
}

/// `Ctx.pending_tail` is one shared, single-slot flag reused across every
/// nested `invoke` call within a whole top-level call. This proves it can't
/// bleed from one mutual-tail chain into an unrelated one: `both` calls
/// `ev?` twice from a *non-tail* position (`and`'s operands), each call
/// independently draining a deep `ev?`/`od?` chain to completion before
/// `both`'s own body sees a real (never a stale/leftover) result.
#[test]
fn tier2a_pending_tail_does_not_leak_across_independent_non_tail_calls() {
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    j.declare("EV3?", &[("N", Ty::Int64)], Ty::Bool);
    j.declare("OD3?", &[("N", Ty::Int64)], Ty::Bool);
    for src in [
        "(defun-typed (ev3? bool) ((n int64)) (if (= n 0) true (od3? (- n 1))))",
        "(defun-typed (od3? bool) ((n int64)) (if (= n 0) false (ev3? (- n 1))))",
        "(defun-typed (both3 bool) ((n int64)) (and (ev3? n) (ev3? n)))",
    ] {
        j.define(&read(src, &env).unwrap()).unwrap();
    }
    assert_eq!(agree(&j, "both3", &[i(20_000_000)]), bo(true));
    assert_eq!(agree(&j, "both3", &[i(20_000_001)]), bo(false));
}

/// A compound (array) value threaded through a mutual-tail chain: each hop
/// allocates a fresh array in the shared call arena (never reclaimed until
/// the whole top-level call returns, so it must stay valid across every
/// subsequent hop, not just the one that allocated it) and the base case
/// reads it back.
#[test]
fn tier2a_array_survives_across_mutual_tail_hops() {
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    j.declare("MAKE-A", &[("N", Ty::Int64)], Ty::Int64);
    j.declare("MAKE-B", &[("N", Ty::Int64)], Ty::Int64);
    for src in [
        // Each hop allocates a fresh 3-element array in the shared call
        // arena (churn across hops) and tail-calls the other, decrementing;
        // the base case reads back an array built on this very last hop to
        // prove it wasn't corrupted by any earlier hop's allocation.
        "(defun-typed (make-a int64) ((n int64)) \
           (let-typed ((arr (array 3))) \
             (store arr 0 n) (store arr 1 n) (store arr 2 n) \
             (if (= n 0) (fetch arr 1) (make-b (- n 1)))))",
        "(defun-typed (make-b int64) ((n int64)) \
           (let-typed ((arr (array 3))) \
             (store arr 0 n) (store arr 1 n) (store arr 2 n) \
             (if (= n 0) (fetch arr 2) (make-a (- n 1)))))",
    ] {
        j.define(&read(src, &env).unwrap()).unwrap();
    }
    assert_eq!(agree(&j, "make-a", &[i(2_000_000)]), i(0));
}

// --- redefinition / the cell -----------------------------------------------

#[test]
fn redefine_changes_behavior() {
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    j.define(&read("(defun-typed (sq int64) ((x int64)) (* x x))", &env).unwrap())
        .unwrap();
    assert_eq!(j.call("SQ", &[i(7)]).unwrap(), i(49));
    // Redefine under the same name -> cube.
    j.define(&read("(defun-typed (sq int64) ((x int64)) (* x (* x x)))", &env).unwrap())
        .unwrap();
    assert_eq!(j.call("SQ", &[i(7)]).unwrap(), i(343));
}

#[test]
fn caller_sees_redefined_callee_through_the_cell() {
    // `use` is compiled once and never recompiled; redefining `sq` must change
    // `use`'s result because the call goes through the registry cell (policy a).
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    j.define(&read("(defun-typed (sq int64) ((x int64)) (* x x))", &env).unwrap())
        .unwrap();
    j.define(&read("(defun-typed (use int64) ((x int64)) (+ (sq x) 1))", &env).unwrap())
        .unwrap();
    assert_eq!(j.call("USE", &[i(5)]).unwrap(), i(26));

    j.define(&read("(defun-typed (sq int64) ((x int64)) (* x (* x x)))", &env).unwrap())
        .unwrap();
    assert_eq!(j.call("USE", &[i(5)]).unwrap(), i(126)); // 125 + 1, no recompile of USE
}

#[test]
fn redefinition_pins_in_flight_edition() {
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    let id = j
        .define(&read("(defun-typed (sq int64) ((x int64)) (* x x))", &env).unwrap())
        .unwrap();
    let f = &j.funcs[id];
    let g0 = f.generation();

    // An in-flight caller pins the current compiled edition.
    let pinned = f.compiled.borrow().clone().unwrap();
    let ctx = j.ctx();
    assert_eq!(pinned(&mut [from_i(6)], &ctx), from_i(36));

    // Redefine: swap in a new edition. The pinned old one stays valid.
    j.define(&read("(defun-typed (sq int64) ((x int64)) (* x (* x x)))", &env).unwrap())
        .unwrap();
    let f = &j.funcs[id];
    assert!(f.generation() > g0);
    let ctx = j.ctx();
    assert_eq!(pinned(&mut [from_i(6)], &ctx), from_i(36)); // old edition unchanged
    assert_eq!(j.call("SQ", &[i(6)]).unwrap(), i(216)); // new edition live
}

#[test]
fn mixed_compiled_and_interpreted_callees() {
    let j = build(&[
        "(defun-typed (sq int64) ((x int64)) (* x x))",
        "(defun-typed (use int64) ((x int64)) (+ (sq x) 1))",
    ]);
    // sq interpreted, use compiled: dispatch is per-function through the cell.
    j.get("SQ").unwrap().deoptimize();
    assert!(!j.get("SQ").unwrap().is_compiled());
    assert!(j.get("USE").unwrap().is_compiled());
    assert_eq!(j.call("USE", &[i(5)]).unwrap(), i(26));
}

// --- the boxed LispVal membrane --------------------------------------------

#[test]
fn membrane_lispval_roundtrip() {
    let j = build(&[
        "(defun-typed (add int64) ((x int64) (y int64)) (+ x y))",
        "(defun-typed (scale float64) ((x float64)) (* x 2.0))",
    ]);
    assert_eq!(
        j.call_lisp("ADD", &[LispVal::Number(20), LispVal::Number(22)])
            .unwrap(),
        LispVal::Number(42)
    );
    assert_eq!(
        j.call_lisp("SCALE", &[LispVal::Float(1.5)]).unwrap(),
        LispVal::Float(3.0)
    );
}

#[test]
fn membrane_rejects_wrong_value_type() {
    let j = build(&["(defun-typed (add int64) ((x int64) (y int64)) (+ x y))"]);
    let err = j.call("ADD", &[fl(1.0), i(2)]).unwrap_err();
    assert!(err.contains("does not match type"), "got: {err}");
}

#[test]
fn membrane_rejects_wrong_arity() {
    let j = build(&["(defun-typed (add int64) ((x int64) (y int64)) (+ x y))"]);
    let err = j.call("ADD", &[i(1)]).unwrap_err();
    assert!(err.contains("expected 2 args"), "got: {err}");
}

// --- type rejection (pre-runtime) ------------------------------------------

#[test]
fn reject_mixed_numeric_operands() {
    let err = def_err("(defun-typed (bad float64) ((x float64)) (+ x 1))");
    assert!(err.contains("operands disagree"), "got: {err}");
}

#[test]
fn reject_arithmetic_on_bool() {
    let err = def_err("(defun-typed (bad bool) ((p bool) (q bool)) (+ p q))");
    assert!(err.contains("numeric operands"), "got: {err}");
}

#[test]
fn reject_mod_on_float() {
    let err = def_err("(defun-typed (bad float64) ((x float64) (y float64)) (mod x y))");
    assert!(err.contains("int64-only"), "got: {err}");
}

#[test]
fn reject_not_on_int() {
    let err = def_err("(defun-typed (bad bool) ((x int64)) (not x))");
    assert!(err.contains("`not` expects bool"), "got: {err}");
}

#[test]
fn reject_and_on_int() {
    let err = def_err("(defun-typed (bad bool) ((x int64) (y int64)) (and x y))");
    assert!(err.contains("bool operands"), "got: {err}");
}

/// A non-bool operand buried in the *middle* of a 3+-operand AND/OR (issue
/// #202's variadic fold) must still be rejected — the `Ty::Bool` check has
/// to run at every level of the right-associative fold
/// (`(and p q r)` -> `And(p, And(q, r))`), not just on the first operand.
#[test]
fn reject_and_on_int_in_middle_of_variadic_chain() {
    let err = def_err("(defun-typed (bad bool) ((p bool) (q bool)) (and p 5 q))");
    assert!(err.contains("bool operands"), "got: {err}");
}

#[test]
fn reject_if_nonbool_condition() {
    let err = def_err("(defun-typed (bad int64) ((x int64)) (if x 1 2))");
    assert!(err.contains("condition must be bool"), "got: {err}");
}

#[test]
fn reject_if_branch_mismatch() {
    let err = def_err("(defun-typed (bad int64) ((x int64)) (if (< x 0) 1 2.0))");
    assert!(err.contains("branches disagree"), "got: {err}");
}

#[test]
fn reject_return_type_mismatch() {
    let err = def_err("(defun-typed (bad int64) ((x int64)) (< x 1))");
    assert!(err.contains("declared return"), "got: {err}");
}

#[test]
fn reject_unbound_variable() {
    let err = def_err("(defun-typed (bad int64) ((x int64)) (+ x y))");
    assert!(err.contains("unbound"), "got: {err}");
}

#[test]
fn reject_unknown_type() {
    let err = def_err("(defun-typed (bad widget) ((x int64)) x)");
    assert!(err.contains("unknown return type"), "got: {err}");
}

#[test]
fn reject_call_to_unknown_function() {
    let err = def_err("(defun-typed (bad int64) ((x int64)) (nope x))");
    assert!(err.contains("unknown function"), "got: {err}");
}

#[test]
fn reject_call_wrong_arity() {
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    j.define(&read("(defun-typed (sq int64) ((x int64)) (* x x))", &env).unwrap())
        .unwrap();
    let err = j
        .define(&read("(defun-typed (bad int64) ((x int64)) (sq x x))", &env).unwrap())
        .unwrap_err();
    assert!(err.contains("expects 1 args"), "got: {err}");
}

#[test]
fn reject_call_wrong_arg_type() {
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    j.define(&read("(defun-typed (sq int64) ((x int64)) (* x x))", &env).unwrap())
        .unwrap();
    let err = j
        .define(&read("(defun-typed (bad int64) ((x float64)) (sq x))", &env).unwrap())
        .unwrap_err();
    assert!(err.contains("expects int64"), "got: {err}");
}

#[test]
fn reject_arity_on_operator() {
    // 0-arg `-` has no identity and must error
    let err = def_err("(defun-typed (bad int64) () (-))");
    assert!(err.contains("requires at least 1"), "got: {err}");
}

// --- variadic arithmetic ---------------------------------------------------

#[test]
fn variadic_add_and_mul() {
    let j = build(&[
        "(defun-typed (sum3 int64) ((a int64) (b int64) (c int64)) (+ a b c))",
        "(defun-typed (prod4 int64) ((a int64) (b int64) (c int64) (d int64)) (* a b c d))",
        "(defun-typed (unary-add int64) ((x int64)) (+ x))",
        "(defun-typed (zero-add int64) () (+))",
        "(defun-typed (one-mul int64) () (*))",
    ]);
    assert_eq!(
        agree(&j, "SUM3", &[Value::Int(1), Value::Int(2), Value::Int(3)]),
        Value::Int(6)
    );
    assert_eq!(
        agree(
            &j,
            "PROD4",
            &[Value::Int(2), Value::Int(3), Value::Int(4), Value::Int(5)]
        ),
        Value::Int(120)
    );
    assert_eq!(agree(&j, "UNARY-ADD", &[Value::Int(7)]), Value::Int(7));
    assert_eq!(agree(&j, "ZERO-ADD", &[]), Value::Int(0));
    assert_eq!(agree(&j, "ONE-MUL", &[]), Value::Int(1));
}

// --- differential sweep ----------------------------------------------------

#[test]
fn differential_sweep_over_many_inputs() {
    let j = build(&[
        "(defun-typed (fib int64) ((n int64)) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2)))))",
        "(defun-typed (fact int64) ((n int64)) (if (<= n 1) 1 (* n (fact (- n 1)))))",
        "(defun-typed (gcd int64) ((a int64) (b int64)) (if (= b 0) a (gcd b (mod a b))))",
    ]);
    // Reference implementations in Rust, compared against both editions.
    fn rfib(n: i64) -> i64 {
        if n < 2 { n } else { rfib(n - 1) + rfib(n - 2) }
    }
    fn rfact(n: i64) -> i64 {
        if n <= 1 {
            1
        } else {
            n.wrapping_mul(rfact(n - 1))
        }
    }
    fn rgcd(a: i64, b: i64) -> i64 {
        if b == 0 { a } else { rgcd(b, a % b) }
    }
    for n in 0..20 {
        assert_eq!(agree(&j, "fib", &[i(n)]), i(rfib(n)), "fib({n})");
        assert_eq!(agree(&j, "fact", &[i(n)]), i(rfact(n)), "fact({n})");
    }
    for a in 1..40 {
        for b in 0..40 {
            assert_eq!(
                agree(&j, "gcd", &[i(a), i(b)]),
                i(rgcd(a, b)),
                "gcd({a},{b})"
            );
        }
    }
}

// --- stability of the underlying Vec / HashMap / slot storage --------------

#[test]
fn registry_growth_keeps_earlier_functions_callable() {
    // Build a long chain f0..f199 (f_i = f_{i-1}(x) + 1), forcing the registry
    // Vec and by-name HashMap to grow/reallocate many times. Earlier functions
    // must stay callable, and (under --features jit) the direct-call cell
    // addresses baked into callers must remain valid -- they point at heap-stable
    // entry cells, not Vec-slot addresses.
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    j.define(&read("(defun-typed (f0 int64) ((x int64)) x)", &env).unwrap())
        .unwrap();
    for k in 1..200 {
        let src = format!(
            "(defun-typed (f{k} int64) ((x int64)) (+ (f{} x) 1))",
            k - 1
        );
        j.define(&read(&src, &env).unwrap()).unwrap();
    }
    assert_eq!(j.call("F199", &[i(0)]).unwrap(), i(199));
    assert_eq!(j.call("F199", &[i(1000)]).unwrap(), i(1199));
    // Earliest function still works after all that growth.
    assert_eq!(j.call("F0", &[i(7)]).unwrap(), i(7));
    // A middle one, too.
    assert_eq!(j.call("F100", &[i(0)]).unwrap(), i(100));
}

#[test]
fn redefine_in_grown_registry_propagates_through_cells() {
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    j.define(&read("(defun-typed (g0 int64) ((x int64)) x)", &env).unwrap())
        .unwrap();
    for k in 1..60 {
        let src = format!(
            "(defun-typed (g{k} int64) ((x int64)) (+ (g{} x) 1))",
            k - 1
        );
        j.define(&read(&src, &env).unwrap()).unwrap();
    }
    assert_eq!(j.call("G50", &[i(0)]).unwrap(), i(50));
    // Redefine the base of the chain; callers must see it through the cell
    // without being recompiled themselves.
    j.define(&read("(defun-typed (g0 int64) ((x int64)) (+ x 100))", &env).unwrap())
        .unwrap();
    assert_eq!(j.call("G50", &[i(0)]).unwrap(), i(150));
}

#[test]
fn many_let_slots_indexed_stably() {
    // 40 sequential let bindings exercise high-index slot reads/writes.
    let mut s = String::from("(defun-typed (chainlet int64) ((x int64)) (let-typed (");
    s.push_str("(v0 int64 x) ");
    for k in 1..40 {
        s.push_str(&format!("(v{k} int64 (+ v{} 1)) ", k - 1));
    }
    s.push_str(") v39))");
    let j = build(&[&s]);
    assert_eq!(agree(&j, "chainlet", &[i(10)]), i(49)); // x + 39
    assert_eq!(agree(&j, "chainlet", &[i(-39)]), i(0));
}

#[test]
fn declared_but_undefined_call_errors_not_panics() {
    let mut j = Jit::new();
    j.declare("GHOST", &[("N", Ty::Int64)], Ty::Int64);
    let err = j.call("GHOST", &[i(1)]).unwrap_err();
    assert!(err.contains("not defined"), "got: {err}");
}

#[test]
fn forward_declared_mutual_recursion_rust_api() {
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    j.declare("EVENP", &[("N", Ty::Int64)], Ty::Bool);
    j.declare("ODDP", &[("N", Ty::Int64)], Ty::Bool);
    j.define(
        &read(
            "(defun-typed (evenp bool) ((n int64)) (if (= n 0) true (oddp (- n 1))))",
            &env,
        )
        .unwrap(),
    )
    .unwrap();
    j.define(
        &read(
            "(defun-typed (oddp bool) ((n int64)) (if (= n 0) false (evenp (- n 1))))",
            &env,
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(agree(&j, "evenp", &[i(64)]), bo(true));
    assert_eq!(agree(&j, "oddp", &[i(64)]), bo(false));
}

// --- char / u8 scalar (issue #136) -----------------------------------------

fn ch(b: u8) -> Value {
    Value::Char(b)
}

#[test]
fn char_code_roundtrip() {
    // code-char(char-code(c)) == c for any byte.
    let j = build(&["(defun-typed (idc char) ((c char)) (code-char (char-code c)))"]);
    assert_eq!(agree(&j, "idc", &[ch(65)]), ch(65));
    assert_eq!(agree(&j, "idc", &[ch(200)]), ch(200));
}

#[test]
fn char_code_widens_to_int() {
    let j = build(&["(defun-typed (code int64) ((c char)) (char-code c))"]);
    assert_eq!(agree(&j, "code", &[ch(65)]), i(65));
}

#[test]
fn code_char_in_range_narrows_out_of_range_errors() {
    // The typed island's `char` is a byte, so CODE-CHAR is only defined for
    // code points 0..=255. In range it narrows to the byte; out of range it
    // used to silently mask (`321 & 0xff == 65`, a #209-direction silent wrong
    // value) and now records the evaluator's range error instead (issue #281),
    // agreeing across every edition. The access stays memory-safe (the masked
    // byte is still produced inside the call) — only the membrane result flips
    // from a wrong char to an error.
    let j = build(&["(defun-typed (mk char) ((n int64)) (code-char n))"]);
    assert_eq!(agree(&j, "mk", &[i(66)]), ch(66));
    assert_eq!(agree(&j, "mk", &[i(0)]), ch(0));
    assert_eq!(agree(&j, "mk", &[i(255)]), ch(255));

    for (n, expected) in [
        (
            321,
            "CODE-CHAR: code point 321 is outside the typed char range 0-255",
        ),
        (
            256,
            "CODE-CHAR: code point 256 is outside the typed char range 0-255",
        ),
        (-1, "CODE-CHAR: expected a non-negative integer, got -1"),
    ] {
        j.compile_all();
        assert_eq!(
            j.call("mk", &[i(n)]).unwrap_err(),
            expected,
            "compiled code-char({n}) range error"
        );
        j.deoptimize_all();
        assert_eq!(
            j.call("mk", &[i(n)]).unwrap_err(),
            expected,
            "interpreted code-char({n}) range error"
        );
    }
    j.compile_all();
}

#[test]
fn char_comparison() {
    let j = build(&["(defun-typed (eqa bool) ((c char)) (= c (code-char 65)))"]);
    assert_eq!(agree(&j, "eqa", &[ch(65)]), bo(true));
    assert_eq!(agree(&j, "eqa", &[ch(66)]), bo(false));
    let j2 = build(&["(defun-typed (lt bool) ((a char) (b char)) (< a b))"]);
    assert_eq!(agree(&j2, "lt", &[ch(1), ch(2)]), bo(true));
    assert_eq!(agree(&j2, "lt", &[ch(9), ch(2)]), bo(false));
}

#[test]
fn byte_arith_via_widening() {
    // Uppercase an ASCII lowercase byte by widening, subtracting 32, narrowing.
    let j = build(&["(defun-typed (up char) ((c char)) (code-char (- (char-code c) 32)))"]);
    assert_eq!(agree(&j, "up", &[ch(97)]), ch(65)); // 'a' -> 'A'
}

#[test]
fn char_type_aliases_parse() {
    // u8 and byte are accepted spellings of char.
    let j = build(&["(defun-typed (f u8) ((c byte)) c)"]);
    assert_eq!(agree(&j, "f", &[ch(7)]), ch(7));
}

#[test]
fn char_rejects_arithmetic() {
    // Bare +/- is not defined on char: you must widen via char-code first.
    let e = def_err("(defun-typed (f char) ((a char) (b char)) (+ a b))");
    assert!(
        e.to_lowercase().contains("char") || e.contains("numeric"),
        "got: {e}"
    );
}

#[test]
fn char_membrane_boxes_to_char() {
    // From untyped Lisp: a Number flows into a char param (backward compat);
    // a Char also works natively. The char result comes back as LispVal::Char.
    let j = build(&["(defun-typed (up char) ((c char)) (code-char (- (char-code c) 32)))"]);
    j.compile_all();
    // LispVal::Number still coerces into char params for backward compat.
    let out = j
        .call_lisp("up", &[crate::LispVal::Number(97)])
        .expect("call_lisp");
    assert_eq!(out, crate::LispVal::Char(65));
    // LispVal::Char is the native input form.
    let out2 = j
        .call_lisp("up", &[crate::LispVal::Char(97)])
        .expect("call_lisp char");
    assert_eq!(out2, crate::LispVal::Char(65));
}

// --- debug trace + structural introspection --------------------------------

#[test]
fn trace_records_result_and_is_deterministic() {
    let j =
        build(&["(defun-typed (poly int64) ((x int64)) (let-typed ((y int64 (* x x))) (+ y x)))"]);
    let (val, log) = j.trace_call("POLY", &[i(4)]).unwrap();
    assert_eq!(val, i(20)); // 16 + 4
    assert!(!log.is_empty());
    // The final recorded step is the function body's result word.
    assert_eq!(log.last().unwrap().result, from_i(20));
    // Tracing again is byte-identical.
    let (val2, log2) = j.trace_call("POLY", &[i(4)]).unwrap();
    assert_eq!(val2, i(20));
    assert_eq!(log, log2);
}

#[test]
fn trace_agrees_with_interpreter_and_compiled() {
    let j = build(&["(defun-typed (f int64) ((a int64) (b int64)) (if (> a b) (- a b) (* a b)))"]);
    for (a, b) in [(7, 3), (2, 9), (5, 5)] {
        let compiled = {
            j.compile_all();
            j.call("F", &[i(a), i(b)]).unwrap()
        };
        let interpreted = {
            j.deoptimize_all();
            j.call("F", &[i(a), i(b)]).unwrap()
        };
        let (traced, _) = j.trace_call("F", &[i(a), i(b)]).unwrap();
        assert_eq!(compiled, interpreted);
        assert_eq!(interpreted, traced);
    }
}

#[test]
fn trace_short_circuits_leave_no_step() {
    // `(or true <rhs>)` must not evaluate <rhs>: no step is recorded for it.
    let j = build(&["(defun-typed (sc bool) ((p bool) (q bool)) (or p q))"]);
    let (val, log) = j.trace_call("SC", &[bo(true), bo(false)]).unwrap();
    assert_eq!(val, bo(true));
    // Steps: var(p) [short-circuits], or. The rhs var(q) is NOT evaluated.
    let var_steps = log.iter().filter(|s| s.op == "var").count();
    assert_eq!(
        var_steps, 1,
        "short-circuited rhs should not be traced: {log:?}"
    );
}

#[test]
fn verify_core_and_node_count_on_lowered_body() {
    let j = build(&["(defun-typed (g int64) ((x int64)) (let-typed ((y int64 (+ x 1))) (* y y)))"]);
    let tf = j.get("G").unwrap();
    let core = tf.core_clone().expect("defined");
    // Well-formed against its own frame and the (single-function) registry.
    verify_core(&core, tf.n_slots(), 1).unwrap();
    assert!(core_node_count(&core) >= 5); // let, +, x, 1, *, y, y ...
}

#[test]
fn verify_core_rejects_out_of_bounds_slot() {
    // A hand-built malformed core (slot index past the frame) must be caught.
    let bad = Core::Var(99);
    let err = verify_core(&bad, 1, 1).unwrap_err();
    assert!(err.contains("out of bounds"), "got: {err}");
    let bad_call = Core::Call(7, vec![]);
    let err2 = verify_core(&bad_call, 1, 1).unwrap_err();
    assert!(err2.contains("Call id"), "got: {err2}");
}

// --- issue #133 Tier 1: self tail-call -> loop -----------------------------

/// The acceptance-criterion example from the issue: a tail-recursive
/// accumulator loop must run in O(1) native stack, on the *default* test
/// stack (no spawned large-stack thread — contrast `deep_recursion_sum_to_n`
/// above, which is genuinely non-tail and still needs one).
#[test]
fn tco_self_tail_call_runs_on_default_stack() {
    let j = build(&[
        "(defun-typed (sum int64) ((n int64) (acc int64)) (if (= n 0) acc (sum (- n 1) (+ acc n))))",
    ]);
    // Sum 1..=100_000_000 = 5_000_000_050_000_000 — would blow any bounded
    // native stack (one frame per iteration) if this weren't a real loop.
    assert_eq!(
        j.call("SUM", &[i(100_000_000), i(0)]).unwrap(),
        i(5_000_000_050_000_000)
    );
}

/// Same shape via the closure-backend interpreter path specifically
/// (`compile_with_tco`/`eval_core`'s tail loop), not just the native
/// Cranelift edition — `agree()` runs both, but this asserts the depth
/// directly against each side after forcing it.
#[test]
fn tco_self_tail_call_agrees_native_vs_interpreted_and_both_are_o1_stack() {
    let j = build(&[
        "(defun-typed (count int64) ((n int64) (acc int64)) (if (= n 0) acc (count (- n 1) (+ acc 1))))",
    ]);
    assert_eq!(agree(&j, "count", &[i(2_000_000), i(0)]), i(2_000_000));
    // Force the interpreter-only (eval_core) path and re-check depth directly.
    j.deoptimize_all();
    assert_eq!(
        j.call("COUNT", &[i(50_000_000), i(0)]).unwrap(),
        i(50_000_000)
    );
}

/// Factorial-with-accumulator: another idiomatic tail-recursive loop shape,
/// deep enough to segfault pre-TCO.
#[test]
fn tco_factorial_with_accumulator() {
    let j = build(&[
        "(defun-typed (fact-acc int64) ((n int64) (acc int64)) (if (= n 0) acc (fact-acc (- n 1) (* acc n))))",
    ]);
    assert_eq!(agree(&j, "fact-acc", &[i(10), i(1)]), i(3_628_800));
}

/// Parallel-assignment correctness (the ticket's one real subtlety): a
/// two-accumulator tail loop (Fibonacci-by-iteration) where the new value of
/// one slot depends on the *old* value of another that is simultaneously
/// being overwritten. If the implementation stored new argument values one
/// at a time instead of computing all of them before storing any, this would
/// silently compute the wrong sequence instead of erroring.
#[test]
fn tco_parallel_assignment_swap_hazard() {
    let j = build(&[
        "(defun-typed (fib-iter int64) ((n int64) (a int64) (b int64)) \
           (if (= n 0) a (fib-iter (- n 1) b (+ a b))))",
    ]);
    // fib-iter(n, 0, 1) is the n-th Fibonacci number.
    assert_eq!(agree(&j, "fib-iter", &[i(20), i(0), i(1)]), i(6765));
    assert_eq!(agree(&j, "fib-iter", &[i(30), i(0), i(1)]), i(832040));
}

/// TCO must fire through nested `if` and `let-typed` tail positions, not
/// just a bare `(if cond (self-call ...) (self-call ...))` at the top of the
/// body.
#[test]
fn tco_fires_through_nested_if_and_let_typed() {
    let j = build(&["(defun-typed (loopy int64) ((n int64) (acc int64)) \
           (let-typed ((doubled (* acc 1))) \
             (if (> n 0) \
                 (if (= (mod n 2) 0) \
                     (loopy (- n 1) (+ doubled 1)) \
                     (loopy (- n 1) (+ doubled 1))) \
                 doubled)))"]);
    assert_eq!(
        j.call("LOOPY", &[i(3_000_000), i(0)]).unwrap(),
        i(3_000_000)
    );
}

/// A self-call that is *not* in tail position (an operand of `+`, exactly
/// like `fib`) must remain an ordinary, non-looping recursive call — the
/// same function also has a genuine tail call elsewhere, so this checks the
/// tail-position analysis distinguishes the two call sites correctly rather
/// than either always-looping (wrong answer) or never-looping (defeats the
/// point).
#[test]
fn tco_leaves_non_tail_self_calls_as_ordinary_recursion() {
    let j = build(&[
        // Non-tail self-call: `(+ n (almost-tail (- n 1)))`.
        "(defun-typed (almost-tail int64) ((n int64)) (if (= n 0) 0 (+ n (almost-tail (- n 1)))))",
    ]);
    assert_eq!(agree(&j, "almost-tail", &[i(100)]), i(5050));
}

/// A function whose every branch is a self tail call with no reachable base
/// case must still *compile* successfully (an infinite loop is a user-level
/// concern, not a compiler one) — this exercises `branch_merge`'s
/// both-branches-tail-loop case, where the shared merge block ends up with
/// no predecessors. Deliberately not called (it would spin forever).
#[test]
fn tco_both_branches_tail_looping_still_compiles() {
    build(&[
        "(defun-typed (spins int64) ((n int64)) (if (> n 0) (spins (- n 1)) (spins (+ n 1))))",
    ]);
}
