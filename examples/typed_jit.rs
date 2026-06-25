//! Typed-JIT prototype demo.
//!
//! Defines `fib` two ways — once in lamedh's normal (boxed `LispVal`) evaluator
//! and once through the typed JIT — type-checks the typed version pre-runtime,
//! and times them head to head. This is the thesis in miniature: knowing the
//! types lets the runtime work on unboxed machine words instead of tagged,
//! `Rc`-counted `LispVal`s.
//!
//! Run with `cargo run --release --example typed_jit`.

use lamedh::LispVal;
use lamedh::environment::Environment;
use lamedh::evaluator::eval;
use lamedh::jit::{Jit, Value};
use lamedh::reader::read;
use std::time::{Duration, Instant};

fn main() {
    let env = Environment::new_with_builtins();

    // --- fib in the normal, boxed evaluator --------------------------------
    let fib_def = read(
        "(def fib (lambda (n) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2))))))",
        &env,
    )
    .unwrap();
    eval(&fib_def, &env).unwrap();
    let fib_call = read("(fib 28)", &env).unwrap();

    // --- fib in the typed JIT ----------------------------------------------
    let mut jit = Jit::new();
    let typed = read(
        "(deffun-typed (fib int64) ((n int64)) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2)))))",
        &env,
    )
    .unwrap();
    jit.define(&typed).expect("typed fib type-checks");

    // The membrane refuses an ill-typed definition before it can ever run.
    let bad = read("(deffun-typed (bad int64) ((x int64)) (if x 1 2))", &env).unwrap();
    println!(
        "define-time rejection: {:?}\n",
        jit.define(&bad).unwrap_err()
    );

    assert_eq!(eval(&fib_call, &env).unwrap(), LispVal::Number(317811));
    assert_eq!(
        jit.call("fib", &[Value::Int(28)]).unwrap(),
        Value::Int(317811)
    );
    println!("both compute fib(28) = 317811\n");

    let iters = 20u32;

    let mut boxed = Duration::ZERO;
    let start = Instant::now();
    for _ in 0..iters {
        std::hint::black_box(eval(&fib_call, &env).unwrap());
    }
    boxed += start.elapsed();

    jit.compile_all();
    let start = Instant::now();
    for _ in 0..iters {
        std::hint::black_box(jit.call("fib", &[Value::Int(28)]).unwrap());
    }
    let jit_compiled = start.elapsed();

    println!("fib(28) x{iters}:");
    println!("  boxed LispVal evaluator: {boxed:?}");
    println!("  typed JIT (unboxed):     {jit_compiled:?}");
    println!(
        "  speedup: {:.1}x",
        boxed.as_secs_f64() / jit_compiled.as_secs_f64()
    );
    println!(
        "\n(The JIT's own interpreter is already unboxed, so the closure backend ~ties it;\n \
         the decisive win over an unboxed tree-walk is what native Cranelift codegen buys —\n \
         the next stage behind the `jit` feature.)"
    );
}
