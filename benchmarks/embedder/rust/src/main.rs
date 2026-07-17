//! The "Call Me Maybe" embedder benchmark (epic #427).
//!
//! Measures what a host program pays, per call, to talk to an embedded Lamedh
//! image — each rung of the ladder an embedder can stand on today, against
//! the pure-Rust floor. Three scenarios:
//!
//! A. **Call overhead** — a trivial tick function invoked per frame, the
//!    game-loop pattern: `eval_str` (string build + parse + eval, what
//!    project-sword-and-sourcery does today) vs a pre-parsed form vs the
//!    typed-registry `jit_call` vs a plain Rust function call.
//! B. **Scalar kernel** — a sphere SDF evaluated 1M times: interpreted
//!    Lisp loop, per-sample `jit_call` into a NATIVE `defun*`, one
//!    `jit_call` running the whole loop natively, pure Rust.
//! C. **Array batch** — dot product over 100k f64: interpreted list code is
//!    omitted (minutes); typed-array `defun*` NATIVE vs Rust iterator.
//!
//! Issues #423 (fast-call API) and #424 (raw native entry points) exist to
//! collapse rungs: #423 should bring rung A2/A3 cost to roughly symbol-lookup
//! + apply; #424 should bring B2 to within a hair of B4.

use std::hint::black_box;
use std::time::Instant;

use lamedh::environment::Environment;
use lamedh::jit::Value;
use lamedh::{
    LispVal, call_function, eval_all, eval_str, evaluator, fn_handle, native_entry, reader,
};

fn time<F: FnMut()>(label: &str, iters: u64, mut f: F) -> f64 {
    // One warmup pass, then the measured run.
    f();
    let start = Instant::now();
    for _ in 0..iters {
        f();
    }
    let total = start.elapsed().as_secs_f64();
    let ns = total * 1e9 / iters as f64;
    println!("{label:<58} {ns:>12.1} ns/call   ({iters} iters)");
    ns
}

#[inline(never)]
fn rust_sd_sphere(x: f64, y: f64, z: f64) -> f64 {
    (x * x + y * y + z * z).sqrt() - 1.0
}

fn main() {
    lamedh::with_large_stack(|| {
        let env = Environment::with_stdlib_fresh();

        // --- definitions -----------------------------------------------------
        eval_all(
            r#"
            (setq *t* 0.0)
            (defun tick (dt) (setq *t* (+ *t* dt)))

            (defun sd-sphere (x y z)
              (- (sqrt (+ (* x x) (* y y) (* z z))) 1.0))

            (defun kernel-loop (n)
              (let ((acc 0.0) (i 0))
                (while (< i n)
                  (setq acc (+ acc (sd-sphere (* 0.001 i) 0.5 0.25)))
                  (setq i (+ i 1)))
                acc))

            (defun* tick-t (dt float64) float64 (+ dt 1.0))

            (defun* sd-sphere-t (x float64) (y float64) (z float64) float64
              (- (sqrt (+ (* x x) (* y y) (* z z))) 1.0))

            (defun* kernel-loop-t (n int64) float64
              (let ((acc 0.0) (i 0))
                (while (< i n)
                  (setq acc (+ acc (sd-sphere-t (* 0.001 (float i)) 0.5 0.25)))
                  (setq i (+ i 1)))
                acc))

            (defun* dot-t (a (array float64)) (b (array float64)) (n int64) float64
              (let ((acc 0.0) (i 0))
                (while (< i n)
                  (setq acc (+ acc (* (aref a i) (aref b i))))
                  (setq i (+ i 1)))
                acc))

            (defun* fill-t (a (array float64)) (b (array float64)) (n int64) int64
              (let ((i 0))
                (while (< i n)
                  (aset a i (* 0.5 (float i)))
                  (aset b i 2.0)
                  (setq i (+ i 1)))
                i))
            "#,
            &env,
        )
        .expect("definitions failed");

        for name in ["TICK-T", "SD-SPHERE-T", "KERNEL-LOOP-T", "DOT-T", "FILL-T"] {
            let tier = eval_str(&format!("(compiled-p '{name})"), &env)
                .map(|v| lamedh::printer::print(&v))
                .unwrap_or_else(|e| format!("err: {e}"));
            let why = eval_str(&format!("(why-not-typed '{name})"), &env)
                .map(|v| lamedh::printer::print(&v))
                .unwrap_or_default();
            println!("; {name}: tier {tier}  why-not: {why}");
            assert_eq!(tier, "NATIVE", "{name} must compile NATIVE — {why}");
        }
        println!();

        // --- A: call overhead -------------------------------------------------
        println!("A. call overhead (trivial tick fn, per frame-style call)");
        let a1 = time("A1 eval_str (string+parse+eval — the game today)", 200_000, || {
            black_box(eval_str("(tick 0.016666)", &env).unwrap());
        });
        let form = reader::read("(tick 0.016666)", &env).unwrap();
        let a2 = time("A2 pre-parsed form, evaluator::eval", 200_000, || {
            black_box(evaluator::eval(&form, &env).unwrap());
        });
        // A2.5 (issue #423): the fast-call API — no reader, no printer, works
        // for any callable (not just a typed/NATIVE `defun*`). This calls the
        // same plain interpreted TICK as A1/A2, so the gap to A2 measures
        // exactly the string-build + parse cost `call_function` skips.
        let a2_5 = time(
            "A2.5 call_function (fast-call API, name lookup + apply)",
            500_000,
            || {
                black_box(call_function("TICK", &[LispVal::Float(0.016_666)], &env).unwrap());
            },
        );
        let tick_handle = fn_handle("TICK", &env).unwrap();
        let a2_6 = time(
            "A2.6 FnHandle::call (fast-call API, pinned symbol)",
            500_000,
            || {
                black_box(
                    tick_handle
                        .call(&[LispVal::Float(0.016_666)], &env)
                        .unwrap(),
                );
            },
        );
        let a3 = time("A3 typed registry jit_call (NATIVE defun*)", 2_000_000, || {
            black_box(env.jit_call("TICK-T", &[Value::Float(0.016_666)]).unwrap().unwrap());
        });
        let mut acc = 0.0f64;
        let a4 = time("A4 pure Rust fn", 50_000_000, || {
            acc = black_box(acc + 0.016_666);
        });
        println!(
            "   ratios vs Rust: eval_str {:.0}x, preparsed {:.0}x, call_function {:.0}x, FnHandle {:.0}x, jit_call {:.0}x\n",
            a1 / a4,
            a2 / a4,
            a2_5 / a4,
            a2_6 / a4,
            a3 / a4
        );

        // --- B: scalar kernel -------------------------------------------------
        println!("B. sphere-SDF kernel, 1,000,000 evaluations");
        let n = 1_000_000u64;
        let t0 = Instant::now();
        let v1 = eval_str("(kernel-loop 1000000)", &env).unwrap();
        let b1 = t0.elapsed().as_secs_f64() * 1e9 / n as f64;
        println!("B1 interpreted lisp loop + interpreted kernel      {b1:>12.1} ns/eval   ({v1:?})");

        let t0 = Instant::now();
        let mut s = 0.0;
        for i in 0..n {
            if let Value::Float(f) = env
                .jit_call(
                    "SD-SPHERE-T",
                    &[
                        Value::Float(0.001 * i as f64),
                        Value::Float(0.5),
                        Value::Float(0.25),
                    ],
                )
                .unwrap()
                .unwrap()
            {
                s += f;
            }
        }
        let b2 = t0.elapsed().as_secs_f64() * 1e9 / n as f64;
        println!("B2 per-sample jit_call into NATIVE defun*          {b2:>12.1} ns/eval   (sum {s:.1})");

        // B2.5 (issue #424): a raw native entry point — extract the compiled
        // SD-SPHERE-T once, then call its machine code directly per sample,
        // with no boxing, no membrane, no dispatch. The per-sample rung a
        // marching-cubes host loop actually wants.
        let sdf = native_entry("SD-SPHERE-T", &env).expect("extract SD-SPHERE-T");
        let t0 = Instant::now();
        let mut s25 = 0.0;
        for i in 0..n {
            s25 += sdf.call_f3(black_box(0.001 * i as f64), 0.5, 0.25);
        }
        let b2_5 = t0.elapsed().as_secs_f64() * 1e9 / n as f64;
        println!("B2.5 per-sample raw native entry (#424)            {b2_5:>12.1} ns/eval   (sum {s25:.1})");

        let t0 = Instant::now();
        let v3 = env.jit_call("KERNEL-LOOP-T", &[Value::Int(n as i64)]).unwrap().unwrap();
        let b3 = t0.elapsed().as_secs_f64() * 1e9 / n as f64;
        println!("B3 whole loop NATIVE (one jit_call)                {b3:>12.1} ns/eval   ({v3:?})");

        let t0 = Instant::now();
        let mut s4 = 0.0;
        for i in 0..n {
            s4 += rust_sd_sphere(black_box(0.001 * i as f64), 0.5, 0.25);
        }
        let b4 = t0.elapsed().as_secs_f64() * 1e9 / n as f64;
        println!("B4 pure Rust loop                                  {b4:>12.1} ns/eval   (sum {s4:.1})");
        println!(
            "   ratios vs Rust: interp {:.0}x, per-sample-call {:.0}x, raw-entry {:.1}x, native-loop {:.1}x\n",
            b1 / b4,
            b2 / b4,
            b2_5 / b4,
            b3 / b4
        );

        // --- C: array batch ---------------------------------------------------
        println!("C. dot product, 100,000 f64 (typed arrays, zero-copy membrane)");
        let m = 100_000usize;
        eval_str(
            "(progn (setq *a* (typed-array 100000 'float64)) \
                    (setq *b* (typed-array 100000 'float64)) \
                    (fill-t *a* *b* 100000))",
            &env,
        )
        .expect("typed-array setup failed");
        let t0 = Instant::now();
        let vd = eval_str("(dot-t *a* *b* 100000)", &env).unwrap();
        let c1 = t0.elapsed().as_secs_f64() * 1e9 / m as f64;
        println!("C1 defun* NATIVE dot over typed arrays             {c1:>12.1} ns/elem   ({})",
                 lamedh::printer::print(&vd));

        let a: Vec<f64> = (0..m).map(|i| 0.5 * i as f64).collect();
        let b: Vec<f64> = vec![2.0; m];
        let t0 = Instant::now();
        let dot: f64 = a.iter().zip(&b).map(|(x, y)| x * y).sum();
        let c2 = t0.elapsed().as_secs_f64() * 1e9 / m as f64;
        println!("C2 pure Rust iterator dot                          {c2:>12.1} ns/elem   ({dot:.1})");
        println!("   ratio vs Rust: {:.1}x", c1 / c2);
    });
}
