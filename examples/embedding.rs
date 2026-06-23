//! End-to-end embedding example for Lamedh.
//!
//! Run with:  cargo run --example embedding
//!
//! Shows the four key embedding tasks:
//!   1. Create an environment with the standard library loaded.
//!   2. Register a host Rust function callable from Lisp.
//!   3. Evaluate a multi-expression Lisp script and read back typed values.
//!   4. Handle a Lisp error gracefully.

use lamedh::{
    LispError, LispVal,
    environment::Environment,
    eval_all, eval_str,
};
use std::convert::TryFrom;

fn main() {
    // Lamedh uses large stack frames. Run everything on a dedicated thread.
    lamedh::with_large_stack(run);
}

fn run() {
    // ------------------------------------------------------------------
    // 1. Create an environment with the full standard library available.
    // ------------------------------------------------------------------
    let env = Environment::with_stdlib();

    // ------------------------------------------------------------------
    // 2. Register a host function.
    //    Arguments arrive as &[LispVal]; return Ok(LispVal) or Err.
    // ------------------------------------------------------------------
    env.register_fn("rust-add", |args, _env| {
        if args.len() != 2 {
            return Err(LispError::Generic(
                "rust-add requires exactly 2 arguments".to_string(),
            ));
        }
        let a = args[0].as_number()?;
        let b = args[1].as_number()?;
        Ok(LispVal::from(a + b))
    });

    println!("--- 1. Calling a registered host function from Lisp ---");
    let result = eval_str("(rust-add 10 32)", &env).expect("eval failed");
    let n = i64::try_from(result).expect("expected integer");
    println!("(rust-add 10 32) => {n}");   // 42

    // ------------------------------------------------------------------
    // 3. Evaluate a multi-expression script; read back typed values.
    // ------------------------------------------------------------------
    println!("\n--- 2. Evaluating a Lisp script ---");
    let script = r#"
        (def *greeting* "Hello from Lamedh!")
        (defun double (x) (* x 2))
        (double 21)
    "#;

    let results = eval_all(script, &env).expect("script failed");
    for val in &results {
        println!("  => {val:?}");
    }

    // The last expression evaluated to 42.
    let last = results.last().expect("no results");
    let doubled: i64 = i64::try_from(last.clone()).expect("expected integer");
    println!("doubled 21 = {doubled}");    // 42

    // Read a variable we defined in the script.
    let greeting = eval_str("*greeting*", &env).expect("eval failed");
    let s = String::try_from(greeting).expect("expected string");
    println!("greeting = {s}");

    // ------------------------------------------------------------------
    // 4. Handle a Lisp error.
    // ------------------------------------------------------------------
    println!("\n--- 3. Handling a Lisp error ---");
    match eval_str("(/ 1 0)", &env) {
        Ok(v)  => println!("unexpected success: {v:?}"),
        Err(LispError::Generic(msg)) => println!("caught error: {msg}"),
        Err(e) => println!("other error: {e}"),
    }

    // Undefined variable.
    match eval_str("undefined-variable", &env) {
        Ok(v)  => println!("unexpected success: {v:?}"),
        Err(LispError::Generic(msg)) => println!("caught error: {msg}"),
        Err(e) => println!("other error: {e}"),
    }

    // ------------------------------------------------------------------
    // 5. Use capability flags to restrict what scripts can do.
    // ------------------------------------------------------------------
    println!("\n--- 4. Capability flags ---");
    // SHELL is disabled by default; enable it only when you trust the script.
    println!("SHELL enabled: {}", env.feature_enabled("SHELL"));
    env.enable_feature("SHELL");
    println!("SHELL enabled after grant: {}", env.feature_enabled("SHELL"));
    env.disable_feature("SHELL");

    println!("\nAll done.");
}
