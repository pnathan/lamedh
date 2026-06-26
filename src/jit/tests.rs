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

// --- arithmetic: int -------------------------------------------------------

#[test]
fn int_add_sub_mul() {
    let j = build(&["(deffun-typed (f int64) ((a int64) (b int64)) (+ (* a b) (- a b)))"]);
    assert_eq!(agree(&j, "f", &[i(6), i(4)]), i(24 + 2));
    assert_eq!(agree(&j, "f", &[i(-3), i(7)]), i(-21 + -10));
}

#[test]
fn int_div_and_mod() {
    let j = build(&[
        "(deffun-typed (d int64) ((a int64) (b int64)) (/ a b))",
        "(deffun-typed (m int64) ((a int64) (b int64)) (mod a b))",
    ]);
    assert_eq!(agree(&j, "d", &[i(17), i(5)]), i(3));
    assert_eq!(agree(&j, "m", &[i(17), i(5)]), i(2));
    assert_eq!(agree(&j, "d", &[i(-17), i(5)]), i(-3)); // truncating
}

#[test]
fn int_div_by_zero_is_zero_not_panic() {
    let j = build(&["(deffun-typed (d int64) ((a int64) (b int64)) (/ a b))"]);
    assert_eq!(agree(&j, "d", &[i(5), i(0)]), i(0));
}

#[test]
fn int_div_mod_overflow_min_by_neg1_is_zero_not_trap() {
    // `i64::MIN / -1` and `i64::MIN % -1` overflow signed division and trap
    // (SIGFPE) on hardware `sdiv`/`srem`. The reference editions use
    // `checked_div`/`checked_rem(...).unwrap_or(0)` ⇒ `0`; every edition
    // (interpreter, closure, and the native Cranelift backend under
    // `--features jit`) must match without faulting.
    let j = build(&[
        "(deffun-typed (d int64) ((a int64) (b int64)) (/ a b))",
        "(deffun-typed (m int64) ((a int64) (b int64)) (mod a b))",
    ]);
    assert_eq!(agree(&j, "d", &[i(i64::MIN), i(-1)]), i(0));
    assert_eq!(agree(&j, "m", &[i(i64::MIN), i(-1)]), i(0));
    // Sanity: ordinary division around the boundary still works.
    assert_eq!(agree(&j, "d", &[i(i64::MIN), i(1)]), i(i64::MIN));
    assert_eq!(agree(&j, "d", &[i(i64::MIN), i(2)]), i(i64::MIN / 2));
}

#[test]
fn int_overflow_wraps() {
    let j = build(&["(deffun-typed (g int64) ((x int64)) (* x x))"]);
    let big = 5_000_000_000i64;
    assert_eq!(agree(&j, "g", &[i(big)]), i(big.wrapping_mul(big)));
}

// --- arithmetic: float -----------------------------------------------------

#[test]
fn float_arithmetic_roundtrips_bits() {
    let j = build(&["(deffun-typed (avg float64) ((x float64) (y float64)) (/ (+ x y) 2.0))"]);
    assert_eq!(agree(&j, "avg", &[fl(3.0), fl(5.0)]), fl(4.0));
    assert_eq!(agree(&j, "avg", &[fl(-1.5), fl(2.5)]), fl(0.5));
}

#[test]
fn float_sub_mul_div() {
    let j = build(&["(deffun-typed (f float64) ((x float64) (y float64)) (- (* x y) (/ x y)))"]);
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
    let j = build(&["(deffun-typed (id float64) ((x float64)) x)"]);
    for v in [-0.0, 0.25, -123.456, 1e300, -1e-300] {
        assert_eq!(agree(&j, "id", &[fl(v)]), fl(v));
    }
}

// --- comparisons -----------------------------------------------------------

#[test]
fn all_int_comparisons() {
    let j = build(&[
        "(deffun-typed (lt bool) ((a int64) (b int64)) (< a b))",
        "(deffun-typed (gt bool) ((a int64) (b int64)) (> a b))",
        "(deffun-typed (le bool) ((a int64) (b int64)) (<= a b))",
        "(deffun-typed (ge bool) ((a int64) (b int64)) (>= a b))",
        "(deffun-typed (eq bool) ((a int64) (b int64)) (= a b))",
        "(deffun-typed (ne bool) ((a int64) (b int64)) (/= a b))",
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
    let j = build(&["(deffun-typed (le bool) ((a float64) (b float64)) (<= a b))"]);
    assert_eq!(agree(&j, "le", &[fl(0.5), fl(0.5)]), bo(true));
    assert_eq!(agree(&j, "le", &[fl(0.5), fl(0.25)]), bo(false));
}

// --- boolean logic ---------------------------------------------------------

#[test]
fn and_or_not_truth_tables() {
    let j = build(&[
        "(deffun-typed (a2 bool) ((p bool) (q bool)) (and p q))",
        "(deffun-typed (o2 bool) ((p bool) (q bool)) (or p q))",
        "(deffun-typed (n1 bool) ((p bool)) (not p))",
    ]);
    for (p, q) in [(false, false), (false, true), (true, false), (true, true)] {
        assert_eq!(agree(&j, "a2", &[bo(p), bo(q)]), bo(p && q));
        assert_eq!(agree(&j, "o2", &[bo(p), bo(q)]), bo(p || q));
    }
    assert_eq!(agree(&j, "n1", &[bo(true)]), bo(false));
    assert_eq!(agree(&j, "n1", &[bo(false)]), bo(true));
}

#[test]
fn range_predicates_compose_logic_and_comparison() {
    let j = build(&[
        "(deffun-typed (between bool) ((x int64) (lo int64) (hi int64)) (and (>= x lo) (<= x hi)))",
        "(deffun-typed (outside bool) ((x int64) (lo int64) (hi int64)) (or (< x lo) (> x hi)))",
    ]);
    assert_eq!(agree(&j, "between", &[i(5), i(1), i(10)]), bo(true));
    assert_eq!(agree(&j, "between", &[i(11), i(1), i(10)]), bo(false));
    assert_eq!(agree(&j, "outside", &[i(11), i(1), i(10)]), bo(true));
    assert_eq!(agree(&j, "outside", &[i(5), i(1), i(10)]), bo(false));
}

#[test]
fn bool_literals() {
    let j = build(&["(deffun-typed (pick bool) ((p bool)) (if p false true))"]);
    assert_eq!(agree(&j, "pick", &[bo(true)]), bo(false));
    assert_eq!(agree(&j, "pick", &[bo(false)]), bo(true));
}

// --- if --------------------------------------------------------------------

#[test]
fn if_branches() {
    let j = build(&[
        "(deffun-typed (absish int64) ((x int64)) (if (< x 0) (- 0 x) x))",
        "(deffun-typed (maxi int64) ((a int64) (b int64)) (if (> a b) a b))",
        "(deffun-typed (sign int64) ((x int64)) (if (> x 0) 1 (if (< x 0) (- 0 1) 0)))",
    ]);
    assert_eq!(agree(&j, "absish", &[i(-9)]), i(9));
    assert_eq!(agree(&j, "absish", &[i(9)]), i(9));
    assert_eq!(agree(&j, "maxi", &[i(3), i(8)]), i(8));
    assert_eq!(agree(&j, "sign", &[i(-4)]), i(-1));
    assert_eq!(agree(&j, "sign", &[i(0)]), i(0));
    assert_eq!(agree(&j, "sign", &[i(4)]), i(1));
}

// --- let-typed -------------------------------------------------------------

#[test]
fn let_single_and_sequential() {
    let j = build(&[
        "(deffun-typed (poly int64) ((x int64)) (let-typed ((y int64 (* x x))) (+ y x)))",
        "(deffun-typed (seq int64) ((x int64)) \
           (let-typed ((y int64 (+ x 1)) (z int64 (* y 2))) (+ y z)))",
    ]);
    assert_eq!(agree(&j, "poly", &[i(3)]), i(12));
    // x=4 -> y=5 -> z=10 -> 15
    assert_eq!(agree(&j, "seq", &[i(4)]), i(15));
}

#[test]
fn let_shadowing_uses_innermost() {
    let j = build(&["(deffun-typed (sh int64) ((x int64)) \
           (let-typed ((x int64 (+ x 1))) (let-typed ((x int64 (* x 10))) x)))"]);
    // x=5 -> 6 -> 60
    assert_eq!(agree(&j, "sh", &[i(5)]), i(60));
}

#[test]
fn nested_let_in_branch_does_not_corrupt_outer_slot() {
    let src = "(deffun-typed (f int64) ((c int64)) \
               (let-typed ((a int64 (if (> c 0) (let-typed ((tmp int64 1)) tmp) 0))) \
                 (+ a 100)))";
    let j = build(&[src]);
    assert_eq!(agree(&j, "f", &[i(5)]), i(101));
    assert_eq!(agree(&j, "f", &[i(-5)]), i(100));
}

#[test]
fn let_with_float() {
    let j = build(&["(deffun-typed (hyp float64) ((a float64) (b float64)) \
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
        "(deffun-typed (poly int64) ((x int64)) (let-typed ((y (* x x))) (+ y x)))",
        "(deffun-typed (hyp float64) ((a float64) (b float64)) \
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
    let j = build(&["(deffun-typed (f int64) ((n int64)) \
           (let-typed ((c (code-char n))) (char-code c)))"]);
    assert_eq!(agree(&j, "f", &[i(65)]), i(65));
    assert_eq!(agree(&j, "f", &[i(321)]), i(65)); // 321 & 0xff = 65
}

#[test]
fn let_inferred_and_explicit_mix() {
    let j = build(&["(deffun-typed (g int64) ((x int64)) \
           (let-typed ((y (+ x 1)) (z int64 (* y 2))) (+ y z)))"]);
    // x=4 -> y=5 -> z=10 -> 15
    assert_eq!(agree(&j, "g", &[i(4)]), i(15));
}

#[test]
fn explicit_annotation_agreeing_with_inference_is_accepted() {
    // The pin matches what inference would derive — accepted.
    let j = build(&["(deffun-typed (h float64) ((x float64)) \
           (let-typed ((y float64 (* x x))) y))"]);
    assert_eq!(agree(&j, "h", &[fl(2.5)]), fl(6.25));
}

#[test]
fn explicit_annotation_conflicting_with_inference_is_rejected() {
    // The initializer is `int64` but the binding is pinned `float64`: the pin
    // and the inferred type fail to unify.
    let err = def_err(
        "(deffun-typed (f int64) ((x int64)) \
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
        "(deffun-typed (f float64) ((a float64)) \
           (let-typed ((y (+ 1 2))) (+ y a)))",
    );
    assert!(err.contains("operands disagree"), "got: {err}");
}

// --- self-recursion --------------------------------------------------------

#[test]
fn factorial() {
    let j =
        build(&["(deffun-typed (fact int64) ((n int64)) (if (<= n 1) 1 (* n (fact (- n 1)))))"]);
    assert_eq!(agree(&j, "fact", &[i(5)]), i(120));
    assert_eq!(agree(&j, "fact", &[i(10)]), i(3_628_800));
    assert_eq!(agree(&j, "fact", &[i(20)]), i(2_432_902_008_176_640_000));
}

#[test]
fn fibonacci() {
    let j = build(&[
        "(deffun-typed (fib int64) ((n int64)) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2)))))",
    ]);
    for (n, want) in [(0, 0), (1, 1), (10, 55), (20, 6765), (25, 75025)] {
        assert_eq!(agree(&j, "fib", &[i(n)]), i(want), "fib({n})");
    }
}

#[test]
fn gcd_euclid() {
    let j = build(&[
        "(deffun-typed (gcd int64) ((a int64) (b int64)) (if (= b 0) a (gcd b (mod a b))))",
    ]);
    assert_eq!(agree(&j, "gcd", &[i(48), i(36)]), i(12));
    assert_eq!(agree(&j, "gcd", &[i(17), i(5)]), i(1));
    assert_eq!(agree(&j, "gcd", &[i(1071), i(462)]), i(21));
}

#[test]
fn integer_power() {
    let j = build(&[
        "(deffun-typed (pw int64) ((base int64) (e int64)) (if (= e 0) 1 (* base (pw base (- e 1)))))",
    ]);
    assert_eq!(agree(&j, "pw", &[i(2), i(10)]), i(1024));
    assert_eq!(agree(&j, "pw", &[i(3), i(5)]), i(243));
}

#[test]
fn ackermann() {
    let j = build(&["(deffun-typed (ack int64) ((m int64) (n int64)) \
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
                "(deffun-typed (sum int64) ((n int64)) (if (= n 0) 0 (+ n (sum (- n 1)))))",
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
        "(deffun-typed (fpow float64) ((b float64) (e int64)) (if (= e 0) 1.0 (* b (fpow b (- e 1)))))",
    ]);
    assert_eq!(agree(&j, "fpow", &[fl(2.0), i(10)]), fl(1024.0));
    assert_eq!(agree(&j, "fpow", &[fl(0.5), i(3)]), fl(0.125));
}

// --- cross-function calls --------------------------------------------------

#[test]
fn cross_function_calls() {
    let j = build(&[
        "(deffun-typed (dbl int64) ((x int64)) (* x 2))",
        "(deffun-typed (quad int64) ((x int64)) (dbl (dbl x)))",
        "(deffun-typed (sq int64) ((x int64)) (* x x))",
        "(deffun-typed (sum-sq int64) ((a int64) (b int64)) (+ (sq a) (sq b)))",
    ]);
    assert_eq!(agree(&j, "quad", &[i(5)]), i(20));
    assert_eq!(agree(&j, "sum-sq", &[i(3), i(4)]), i(25));
}

#[test]
fn call_across_types() {
    let j = build(&[
        "(deffun-typed (is-even bool) ((n int64)) (= (mod n 2) 0))",
        "(deffun-typed (classify int64) ((n int64)) (if (is-even n) 0 1))",
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
            "(deffun-typed (even? bool) ((n int64)) (if (= n 0) true (odd? (- n 1))))",
            &env,
        )
        .unwrap(),
    )
    .unwrap();
    j.define(
        &read(
            "(deffun-typed (odd? bool) ((n int64)) (if (= n 0) false (even? (- n 1))))",
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

// --- redefinition / the cell -----------------------------------------------

#[test]
fn redefine_changes_behavior() {
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    j.define(&read("(deffun-typed (sq int64) ((x int64)) (* x x))", &env).unwrap())
        .unwrap();
    assert_eq!(j.call("SQ", &[i(7)]).unwrap(), i(49));
    // Redefine under the same name -> cube.
    j.define(&read("(deffun-typed (sq int64) ((x int64)) (* x (* x x)))", &env).unwrap())
        .unwrap();
    assert_eq!(j.call("SQ", &[i(7)]).unwrap(), i(343));
}

#[test]
fn caller_sees_redefined_callee_through_the_cell() {
    // `use` is compiled once and never recompiled; redefining `sq` must change
    // `use`'s result because the call goes through the registry cell (policy a).
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    j.define(&read("(deffun-typed (sq int64) ((x int64)) (* x x))", &env).unwrap())
        .unwrap();
    j.define(&read("(deffun-typed (use int64) ((x int64)) (+ (sq x) 1))", &env).unwrap())
        .unwrap();
    assert_eq!(j.call("USE", &[i(5)]).unwrap(), i(26));

    j.define(&read("(deffun-typed (sq int64) ((x int64)) (* x (* x x)))", &env).unwrap())
        .unwrap();
    assert_eq!(j.call("USE", &[i(5)]).unwrap(), i(126)); // 125 + 1, no recompile of USE
}

#[test]
fn redefinition_pins_in_flight_edition() {
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    let id = j
        .define(&read("(deffun-typed (sq int64) ((x int64)) (* x x))", &env).unwrap())
        .unwrap();
    let f = &j.funcs[id];
    let g0 = f.generation();

    // An in-flight caller pins the current compiled edition.
    let pinned = f.compiled.borrow().clone().unwrap();
    let ctx = j.ctx();
    assert_eq!(pinned(&mut [from_i(6)], &ctx), from_i(36));

    // Redefine: swap in a new edition. The pinned old one stays valid.
    j.define(&read("(deffun-typed (sq int64) ((x int64)) (* x (* x x)))", &env).unwrap())
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
        "(deffun-typed (sq int64) ((x int64)) (* x x))",
        "(deffun-typed (use int64) ((x int64)) (+ (sq x) 1))",
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
        "(deffun-typed (add int64) ((x int64) (y int64)) (+ x y))",
        "(deffun-typed (scale float64) ((x float64)) (* x 2.0))",
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
    let j = build(&["(deffun-typed (add int64) ((x int64) (y int64)) (+ x y))"]);
    let err = j.call("ADD", &[fl(1.0), i(2)]).unwrap_err();
    assert!(err.contains("does not match type"), "got: {err}");
}

#[test]
fn membrane_rejects_wrong_arity() {
    let j = build(&["(deffun-typed (add int64) ((x int64) (y int64)) (+ x y))"]);
    let err = j.call("ADD", &[i(1)]).unwrap_err();
    assert!(err.contains("expected 2 args"), "got: {err}");
}

// --- type rejection (pre-runtime) ------------------------------------------

#[test]
fn reject_mixed_numeric_operands() {
    let err = def_err("(deffun-typed (bad float64) ((x float64)) (+ x 1))");
    assert!(err.contains("operands disagree"), "got: {err}");
}

#[test]
fn reject_arithmetic_on_bool() {
    let err = def_err("(deffun-typed (bad bool) ((p bool) (q bool)) (+ p q))");
    assert!(err.contains("numeric operands"), "got: {err}");
}

#[test]
fn reject_mod_on_float() {
    let err = def_err("(deffun-typed (bad float64) ((x float64) (y float64)) (mod x y))");
    assert!(err.contains("int64-only"), "got: {err}");
}

#[test]
fn reject_not_on_int() {
    let err = def_err("(deffun-typed (bad bool) ((x int64)) (not x))");
    assert!(err.contains("`not` expects bool"), "got: {err}");
}

#[test]
fn reject_and_on_int() {
    let err = def_err("(deffun-typed (bad bool) ((x int64) (y int64)) (and x y))");
    assert!(err.contains("bool operands"), "got: {err}");
}

#[test]
fn reject_if_nonbool_condition() {
    let err = def_err("(deffun-typed (bad int64) ((x int64)) (if x 1 2))");
    assert!(err.contains("condition must be bool"), "got: {err}");
}

#[test]
fn reject_if_branch_mismatch() {
    let err = def_err("(deffun-typed (bad int64) ((x int64)) (if (< x 0) 1 2.0))");
    assert!(err.contains("branches disagree"), "got: {err}");
}

#[test]
fn reject_return_type_mismatch() {
    let err = def_err("(deffun-typed (bad int64) ((x int64)) (< x 1))");
    assert!(err.contains("declared return"), "got: {err}");
}

#[test]
fn reject_unbound_variable() {
    let err = def_err("(deffun-typed (bad int64) ((x int64)) (+ x y))");
    assert!(err.contains("unbound"), "got: {err}");
}

#[test]
fn reject_unknown_type() {
    let err = def_err("(deffun-typed (bad widget) ((x int64)) x)");
    assert!(err.contains("unknown return type"), "got: {err}");
}

#[test]
fn reject_call_to_unknown_function() {
    let err = def_err("(deffun-typed (bad int64) ((x int64)) (nope x))");
    assert!(err.contains("unknown function"), "got: {err}");
}

#[test]
fn reject_call_wrong_arity() {
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    j.define(&read("(deffun-typed (sq int64) ((x int64)) (* x x))", &env).unwrap())
        .unwrap();
    let err = j
        .define(&read("(deffun-typed (bad int64) ((x int64)) (sq x x))", &env).unwrap())
        .unwrap_err();
    assert!(err.contains("expects 1 args"), "got: {err}");
}

#[test]
fn reject_call_wrong_arg_type() {
    let env = Environment::new_with_builtins();
    let mut j = Jit::new();
    j.define(&read("(deffun-typed (sq int64) ((x int64)) (* x x))", &env).unwrap())
        .unwrap();
    let err = j
        .define(&read("(deffun-typed (bad int64) ((x float64)) (sq x))", &env).unwrap())
        .unwrap_err();
    assert!(err.contains("expects Int64"), "got: {err}");
}

#[test]
fn reject_arity_on_operator() {
    let err = def_err("(deffun-typed (bad int64) ((x int64)) (+ x))");
    assert!(err.contains("expects 2 args"), "got: {err}");
}

// --- differential sweep ----------------------------------------------------

#[test]
fn differential_sweep_over_many_inputs() {
    let j = build(&[
        "(deffun-typed (fib int64) ((n int64)) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2)))))",
        "(deffun-typed (fact int64) ((n int64)) (if (<= n 1) 1 (* n (fact (- n 1)))))",
        "(deffun-typed (gcd int64) ((a int64) (b int64)) (if (= b 0) a (gcd b (mod a b))))",
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
    j.define(&read("(deffun-typed (f0 int64) ((x int64)) x)", &env).unwrap())
        .unwrap();
    for k in 1..200 {
        let src = format!(
            "(deffun-typed (f{k} int64) ((x int64)) (+ (f{} x) 1))",
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
    j.define(&read("(deffun-typed (g0 int64) ((x int64)) x)", &env).unwrap())
        .unwrap();
    for k in 1..60 {
        let src = format!(
            "(deffun-typed (g{k} int64) ((x int64)) (+ (g{} x) 1))",
            k - 1
        );
        j.define(&read(&src, &env).unwrap()).unwrap();
    }
    assert_eq!(j.call("G50", &[i(0)]).unwrap(), i(50));
    // Redefine the base of the chain; callers must see it through the cell
    // without being recompiled themselves.
    j.define(&read("(deffun-typed (g0 int64) ((x int64)) (+ x 100))", &env).unwrap())
        .unwrap();
    assert_eq!(j.call("G50", &[i(0)]).unwrap(), i(150));
}

#[test]
fn many_let_slots_indexed_stably() {
    // 40 sequential let bindings exercise high-index slot reads/writes.
    let mut s = String::from("(deffun-typed (chainlet int64) ((x int64)) (let-typed (");
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
            "(deffun-typed (evenp bool) ((n int64)) (if (= n 0) true (oddp (- n 1))))",
            &env,
        )
        .unwrap(),
    )
    .unwrap();
    j.define(
        &read(
            "(deffun-typed (oddp bool) ((n int64)) (if (= n 0) false (evenp (- n 1))))",
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
    let j = build(&["(deffun-typed (idc char) ((c char)) (code-char (char-code c)))"]);
    assert_eq!(agree(&j, "idc", &[ch(65)]), ch(65));
    assert_eq!(agree(&j, "idc", &[ch(200)]), ch(200));
}

#[test]
fn char_code_widens_to_int() {
    let j = build(&["(deffun-typed (code int64) ((c char)) (char-code c))"]);
    assert_eq!(agree(&j, "code", &[ch(65)]), i(65));
}

#[test]
fn code_char_narrows_and_masks() {
    let j = build(&["(deffun-typed (mk char) ((n int64)) (code-char n))"]);
    assert_eq!(agree(&j, "mk", &[i(66)]), ch(66));
    // narrowing masks to a byte: 321 & 0xff == 65
    assert_eq!(agree(&j, "mk", &[i(321)]), ch(65));
}

#[test]
fn char_comparison() {
    let j = build(&["(deffun-typed (eqa bool) ((c char)) (= c (code-char 65)))"]);
    assert_eq!(agree(&j, "eqa", &[ch(65)]), bo(true));
    assert_eq!(agree(&j, "eqa", &[ch(66)]), bo(false));
    let j2 = build(&["(deffun-typed (lt bool) ((a char) (b char)) (< a b))"]);
    assert_eq!(agree(&j2, "lt", &[ch(1), ch(2)]), bo(true));
    assert_eq!(agree(&j2, "lt", &[ch(9), ch(2)]), bo(false));
}

#[test]
fn byte_arith_via_widening() {
    // Uppercase an ASCII lowercase byte by widening, subtracting 32, narrowing.
    let j = build(&["(deffun-typed (up char) ((c char)) (code-char (- (char-code c) 32)))"]);
    assert_eq!(agree(&j, "up", &[ch(97)]), ch(65)); // 'a' -> 'A'
}

#[test]
fn char_type_aliases_parse() {
    // u8 and byte are accepted spellings of char.
    let j = build(&["(deffun-typed (f u8) ((c byte)) c)"]);
    assert_eq!(agree(&j, "f", &[ch(7)]), ch(7));
}

#[test]
fn char_rejects_arithmetic() {
    // Bare +/- is not defined on char: you must widen via char-code first.
    let e = def_err("(deffun-typed (f char) ((a char) (b char)) (+ a b))");
    assert!(
        e.to_lowercase().contains("char") || e.contains("numeric"),
        "got: {e}"
    );
}

#[test]
fn char_membrane_boxes_to_char() {
    // From untyped Lisp: a Number flows into a char param (backward compat);
    // a Char also works natively. The char result comes back as LispVal::Char.
    let j = build(&["(deffun-typed (up char) ((c char)) (code-char (- (char-code c) 32)))"]);
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
        build(&["(deffun-typed (poly int64) ((x int64)) (let-typed ((y int64 (* x x))) (+ y x)))"]);
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
    let j = build(&["(deffun-typed (f int64) ((a int64) (b int64)) (if (> a b) (- a b) (* a b)))"]);
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
    let j = build(&["(deffun-typed (sc bool) ((p bool) (q bool)) (or p q))"]);
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
    let j =
        build(&["(deffun-typed (g int64) ((x int64)) (let-typed ((y int64 (+ x 1))) (* y y)))"]);
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
