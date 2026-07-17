//! Raw native entry points for NATIVE leaf `defun*` kernels (issue #424).
//!
//! Covers extraction success (leaf float64/int64 kernels, including a
//! call-graph the inliner collapses into a leaf), signature-shape validation,
//! non-leaf rejection (naming the surviving callees), snapshot-across-
//! redefinition semantics + `generation()`, a multi-thread hammer proving the
//! handle is safe to call concurrently, `last_error()` reporting, and result
//! parity against a Rust reference for 1000 random inputs.
#![cfg(feature = "jit")]

use lamedh::environment::Environment;
use lamedh::{LispVal, eval_all, native_entry};

fn eval_i(src: &str, env: &lamedh::Shared<Environment>) -> i64 {
    match lamedh::eval_str(src, env).expect(src) {
        LispVal::Number(n) => n,
        other => panic!("{src}: expected integer, got {other:?}"),
    }
}

// A tiny deterministic xorshift RNG so the parity sweep needs no dev-dep.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn f(&mut self, lo: f64, hi: f64) -> f64 {
        let u = (self.next() >> 11) as f64 / (1u64 << 53) as f64; // [0,1)
        lo + u * (hi - lo)
    }
}

// -------------------------------------------------------------------------
// Extraction success.
// -------------------------------------------------------------------------

#[test]
fn extract_float3_leaf_kernel() {
    lamedh::with_large_stack(|| {
        let env = Environment::with_stdlib();
        eval_all(
            "(defun-typed (sdf float64) ((x float64) (y float64) (z float64))\n\
               (- (sqrt (+ (* x x) (+ (* y y) (* z z)))) 1.0))",
            &env,
        )
        .unwrap();
        let h = native_entry("sdf", &env).expect("extract sdf");
        assert_eq!(
            h.params(),
            &[
                lamedh::jit::entry::NativeTy::Float64,
                lamedh::jit::entry::NativeTy::Float64,
                lamedh::jit::entry::NativeTy::Float64
            ]
        );
        assert_eq!(h.ret(), lamedh::jit::entry::NativeTy::Float64);
        // 3-4-5 style: sqrt(9+0+16) - 1 = 4.
        assert_eq!(h.call_f3(3.0, 0.0, 4.0), 4.0);
        assert!(h.last_error().is_none());
    });
}

#[test]
fn extract_int_leaf_kernels() {
    lamedh::with_large_stack(|| {
        let env = Environment::with_stdlib();
        eval_all(
            "(defun-typed (mul2 int64) ((a int64) (b int64)) (* a b))\n\
             (defun-typed (dbl int64) ((x int64)) (+ x x))",
            &env,
        )
        .unwrap();
        let m = native_entry("mul2", &env).expect("extract mul2");
        assert_eq!(m.call_i2(6, 7), 42);
        let d = native_entry("dbl", &env).expect("extract dbl");
        assert_eq!(d.call_i1(21), 42);
        // Generic escape hatch agrees with the typed fast path.
        assert_eq!(m.call_words(&[6, 7]), 42);
    });
}

#[test]
fn callgraph_that_inlines_to_a_leaf_is_extractable() {
    lamedh::with_large_stack(|| {
        let env = Environment::with_stdlib();
        // `sq` is tiny, so the Core inliner splices it into `sumsq`, leaving
        // no residual Call — `sumsq` is a leaf and extractable.
        eval_all(
            "(defun-typed (sq int64) ((x int64)) (* x x))\n\
             (defun-typed (sumsq int64) ((a int64) (b int64)) (+ (sq a) (sq b)))",
            &env,
        )
        .unwrap();
        let h = native_entry("sumsq", &env).expect("sumsq inlines to a leaf");
        assert_eq!(h.call_i2(3, 4), 25);
    });
}

// -------------------------------------------------------------------------
// Validation errors.
// -------------------------------------------------------------------------

#[test]
fn unknown_function_is_rejected() {
    lamedh::with_large_stack(|| {
        let env = Environment::with_stdlib();
        let err = native_entry("nope", &env).unwrap_err();
        assert!(err.contains("no typed function"), "{err}");
    });
}

#[test]
fn non_scalar_signature_is_rejected() {
    lamedh::with_large_stack(|| {
        let env = Environment::with_stdlib();
        // Array parameter — cannot cross a raw entry.
        eval_all(
            "(defun-typed (asum int64) ((xs (array int64))) (array-sum xs))",
            &env,
        )
        .unwrap();
        let err = native_entry("asum", &env).unwrap_err();
        assert!(
            err.contains("not a raw scalar") && err.to_uppercase().contains("XS"),
            "shape error should name the offending param: {err}"
        );
    });
}

#[test]
fn non_leaf_rejection_names_the_callees() {
    lamedh::with_large_stack(|| {
        let env = Environment::with_stdlib();
        // `bigh` is deliberately over the inline node budget, so it survives
        // as a real cross-function call inside `usebig` — which is therefore
        // not a leaf.
        eval_all(
            "(defun-typed (bigh int64) ((a int64) (b int64))\n\
               (+ (* a b) (+ (* a a) (+ (* b b) (+ (- a b) (+ (* a 2)\n\
                  (+ (* b 3) (+ (* a 4) (+ (* b 5) (+ (* a 6) (* b 7)))))))))))\n\
             (defun-typed (usebig int64) ((x int64)) (+ x (bigh x 2)))",
            &env,
        )
        .unwrap();
        // Sanity: usebig really did go native (so the rejection is about
        // leaf-ness, not a missing native edition).
        assert_eq!(
            lamedh::printer::print(&lamedh::eval_str("(compiled-p 'usebig)", &env).unwrap()),
            "NATIVE"
        );
        let err = native_entry("usebig", &env).unwrap_err();
        assert!(
            err.contains("not a leaf") && err.to_uppercase().contains("BIGH"),
            "non-leaf error should name the callee: {err}"
        );
    });
}

// -------------------------------------------------------------------------
// last_error(): raw entries report, but do not propagate, conditions.
// -------------------------------------------------------------------------

#[test]
fn division_by_zero_is_reported_via_last_error() {
    lamedh::with_large_stack(|| {
        let env = Environment::with_stdlib();
        eval_all(
            "(defun-typed (dv int64) ((a int64) (b int64)) (/ a b))",
            &env,
        )
        .unwrap();
        let h = native_entry("dv", &env).expect("extract dv");
        assert_eq!(h.call_i2(12, 3), 4);
        assert!(h.last_error().is_none(), "clean call clears last_error");
        // Division by zero: memory-safe substitute returned, error recorded.
        let _ = h.call_i2(1, 0);
        let err = h.last_error().expect("div by zero recorded");
        assert!(err.contains("division by zero"), "{err}");
        // A subsequent clean call clears it again.
        assert_eq!(h.call_i2(9, 3), 3);
        assert!(h.last_error().is_none());
    });
}

// -------------------------------------------------------------------------
// Snapshot semantics + generation().
// -------------------------------------------------------------------------

#[test]
fn handle_pins_a_snapshot_across_redefinition() {
    lamedh::with_large_stack(|| {
        let env = Environment::with_stdlib();
        eval_all("(defun-typed (k int64) ((x int64)) (+ x 1))", &env).unwrap();
        let h = native_entry("k", &env).expect("extract k");
        assert_eq!(h.call_i1(10), 11);
        let gen0 = h.generation();
        assert_eq!(env.jit_generation("k"), Some(gen0));

        // Redefine to a different formula. The live function changes; the
        // pinned handle keeps running its captured edition.
        eval_all("(defun-typed (k int64) ((x int64)) (* x 100))", &env).unwrap();
        assert_eq!(eval_i("(k 10)", &env), 1000, "live function is redefined");
        assert_eq!(h.call_i1(10), 11, "handle still runs the old snapshot");

        // generation() lets the host detect the staleness.
        let gen1 = env.jit_generation("k").expect("k still defined");
        assert!(
            gen1 > gen0,
            "redefinition bumps generation ({gen0} -> {gen1})"
        );
        assert_eq!(h.generation(), gen0, "handle keeps its snapshot generation");

        // Re-extracting picks up the new edition.
        let h2 = native_entry("k", &env).expect("re-extract k");
        assert_eq!(h2.call_i1(10), 1000);
        assert_eq!(h2.generation(), gen1);
    });
}

// -------------------------------------------------------------------------
// Thread-safety: hammer one handle from many threads on disjoint inputs.
// -------------------------------------------------------------------------

#[test]
fn concurrent_calls_are_sound_and_correct() {
    lamedh::with_large_stack(|| {
        let env = Environment::with_stdlib();
        eval_all(
            "(defun-typed (sdf float64) ((x float64) (y float64) (z float64))\n\
               (- (sqrt (+ (* x x) (+ (* y y) (* z z)))) 1.0))",
            &env,
        )
        .unwrap();
        let h = native_entry("sdf", &env).expect("extract sdf");

        // Reference computed serially in Rust. Note the associativity must
        // match the Lisp body `(+ (* x x) (+ (* y y) (* z z)))` exactly —
        // float addition is not associative, so a left-associated sum would
        // differ by a ULP.
        let reference = |x: f64, y: f64, z: f64| (x * x + (y * y + z * z)).sqrt() - 1.0;

        const THREADS: usize = 6;
        const PER: usize = 4000;
        let serial: Vec<f64> = (0..THREADS * PER)
            .map(|i| {
                let x = i as f64 * 0.5;
                let y = i as f64 * 0.25;
                let z = i as f64 * 0.125;
                reference(x, y, z)
            })
            .collect();

        let concurrent: Vec<f64> = std::thread::scope(|s| {
            let handles: Vec<_> = (0..THREADS)
                .map(|t| {
                    let h = &h;
                    s.spawn(move || {
                        (t * PER..(t + 1) * PER)
                            .map(|i| {
                                let x = i as f64 * 0.5;
                                let y = i as f64 * 0.25;
                                let z = i as f64 * 0.125;
                                (i, h.call_f3(x, y, z))
                            })
                            .collect::<Vec<(usize, f64)>>()
                    })
                })
                .collect();
            let mut out = vec![0.0; THREADS * PER];
            for jh in handles {
                for (i, v) in jh.join().unwrap() {
                    out[i] = v;
                }
            }
            out
        });

        assert_eq!(serial.len(), concurrent.len());
        for (i, (a, b)) in serial.iter().zip(concurrent.iter()).enumerate() {
            assert_eq!(a.to_bits(), b.to_bits(), "mismatch at sample {i}");
        }
    });
}

// -------------------------------------------------------------------------
// Result parity vs a Rust reference for 1000 random inputs.
// -------------------------------------------------------------------------

#[test]
fn parity_against_reference_1000_random_inputs() {
    lamedh::with_large_stack(|| {
        let env = Environment::with_stdlib();
        eval_all(
            "(defun-typed (sdf float64) ((x float64) (y float64) (z float64))\n\
               (- (sqrt (+ (* x x) (+ (* y y) (* z z)))) 1.0))\n\
             (defun-typed (poly int64) ((a int64) (b int64))\n\
               (+ (* a a) (- (* 3 b) (* a b))))",
            &env,
        )
        .unwrap();
        let sdf = native_entry("sdf", &env).expect("extract sdf");
        let poly = native_entry("poly", &env).expect("extract poly");

        let mut rng = Rng(0x9E3779B97F4A7C15);
        for _ in 0..1000 {
            let x = rng.f(-100.0, 100.0);
            let y = rng.f(-100.0, 100.0);
            let z = rng.f(-100.0, 100.0);
            let got = sdf.call_f3(x, y, z);
            // Associativity matches the Lisp body (see the note in the
            // concurrency test): x*x + (y*y + z*z).
            let want = (x * x + (y * y + z * z)).sqrt() - 1.0;
            assert_eq!(got.to_bits(), want.to_bits(), "sdf({x},{y},{z})");

            let a = (rng.next() % 2000) as i64 - 1000;
            let b = (rng.next() % 2000) as i64 - 1000;
            let got = poly.call_i2(a, b);
            let want = a
                .wrapping_mul(a)
                .wrapping_add(3i64.wrapping_mul(b).wrapping_sub(a.wrapping_mul(b)));
            assert_eq!(got, want, "poly({a},{b})");
        }
    });
}
