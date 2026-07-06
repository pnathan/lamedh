//! Integration tests for issue #270: unbounded reader recursion used to crash
//! the process (native stack overflow) on pathologically deep input such as
//! tens of thousands of nested `(` or a long `'''...` quote chain.
//!
//! The reader now bounds recursion at a caller-chosen depth limit and reports
//! a normal parse error instead. The contract:
//! - Plain `reader::read`/`read_all`/`read_next` use
//!   `reader::DEFAULT_READER_DEPTH` (512), sized for stacks of ~4 MiB or
//!   more. Callers on smaller stacks must pass a lower limit through the
//!   `_with_depth_limit` variants (see `small_stack_needs_lower_limit`).
//! - Evaluator-facing entry points (`load_file`, `eval_str`, and the
//!   `read-from-string`/`read` builtins) use
//!   `Environment::reader_depth_limit()`, which `with_stdlib` raises to
//!   50,000 for the 512 MiB `with_large_stack` thread they are documented
//!   to run on.
mod test_helpers;
use lamedh::environment::Environment;
use lamedh::{Shared, with_large_stack};
use test_helpers::env_with_stdlib;

fn nested_parens(depth: usize) -> String {
    let mut s = String::with_capacity(depth * 2 + 1);
    for _ in 0..depth {
        s.push('(');
    }
    s.push('1');
    for _ in 0..depth {
        s.push(')');
    }
    s
}

fn quote_chain(depth: usize) -> String {
    let mut s = String::with_capacity(depth + 1);
    for _ in 0..depth {
        s.push('\'');
    }
    s.push('1');
    s
}

/// A chain of `prefix` repeated `depth` times in front of `1`, e.g. `,,,,1`.
fn prefix_chain(prefix: &str, depth: usize) -> String {
    let mut s = String::with_capacity(prefix.len() * depth + 1);
    for _ in 0..depth {
        s.push_str(prefix);
    }
    s.push('1');
    s
}

/// Right-nested dotted tails: `(1 . (1 . (1 . ... x)))` — recurses through
/// the dotted-tail branch of `parse_list_contents` rather than the plain
/// element branch.
fn dotted_chain(depth: usize) -> String {
    let mut s = String::with_capacity(depth * 5 + 1);
    for _ in 0..depth {
        s.push_str("(1 . ");
    }
    s.push('x');
    for _ in 0..depth {
        s.push(')');
    }
    s
}

// (a) 20k-deep nested parens through the plain `reader::read` entry point
// (default limit 512): must return a parse error, not crash the process.
//
// NOTE on stack context: `.cargo/config.toml` sets RUST_MIN_STACK=128 MiB, so
// this test thread is NOT a small stack — it validates the error contract of
// the default limit, not small-stack safety (that is what
// `small_stack_needs_lower_limit` below is for).
#[test]
fn deeply_nested_parens_error_not_crash() {
    let env = Shared::new(Environment::new());
    let input = nested_parens(20_000);
    let result = lamedh::reader::read(&input, &env);
    assert!(result.is_err(), "expected a parse error, got {result:?}");
    let msg = result.unwrap_err();
    assert!(
        msg.contains("nesting too deep"),
        "expected a 'nesting too deep' message, got: {msg}"
    );
    assert!(
        msg.contains(&lamedh::reader::DEFAULT_READER_DEPTH.to_string()),
        "expected the message to carry the configured limit, got: {msg}"
    );
}

// (b) Every other recursive reader production is bounded too, not just
// lists: quote, unquote, unquote-splicing, function shorthand, and dotted
// tails each recurse through a distinct branch of the reader.
#[test]
fn all_recursive_productions_are_bounded() {
    let env = Shared::new(Environment::new());
    for (name, input) in [
        ("quote chain", quote_chain(20_000)),
        ("unquote chain", prefix_chain(",", 20_000)),
        ("unquote-splicing chain", prefix_chain(",@", 20_000)),
        ("function-shorthand chain", prefix_chain("#'", 20_000)),
        ("dotted-tail chain", dotted_chain(20_000)),
    ] {
        let result = lamedh::reader::read(&input, &env);
        assert!(
            result.is_err(),
            "{name}: expected a parse error, got {result:?}"
        );
        let msg = result.unwrap_err();
        assert!(
            msg.contains("nesting too deep"),
            "{name}: expected a 'nesting too deep' message, got: {msg}"
        );
    }
}

// Same shapes from a with_large_stack (512 MiB) thread: the *default-limit*
// entry point must trigger its limit long before that stack is exhausted.
#[test]
fn deeply_nested_parens_error_not_crash_on_large_stack() {
    with_large_stack(|| {
        let env = Shared::new(Environment::new());
        assert!(lamedh::reader::read(&nested_parens(20_000), &env).is_err());
        assert!(lamedh::reader::read(&quote_chain(20_000), &env).is_err());
    });
}

// (c) `read-from-string` past the environment's configured limit (50,000
// for a stdlib environment) must surface as a catchable Lisp error, not a
// host-level crash, since it is reachable at runtime from ordinary Lisp code.
#[test]
fn read_from_string_deep_input_is_catchable_lisp_error() {
    with_large_stack(|| {
        // Environment::with_stdlib() (not the test helper, which builds its
        // environment by hand) so the test exercises the real 50,000 default.
        let env = Environment::with_stdlib();
        let deep = nested_parens(60_000); // past the 50,000 stdlib-env limit
        let program = format!("(handler-case (read-from-string \"{deep}\") (error (e) 'caught))");
        let result = lamedh::eval_line(&program, &env);
        assert_eq!(
            result, "CAUGHT",
            "expected the reader error to be caught as a Lisp condition, got: {result}"
        );
    });
}

// (c2) The flip side of the configurable limit: a stdlib environment on the
// large stack can read machine-generated data far deeper than the 512
// library default — depth 5,000 here — restoring the pre-limit capability
// for the documented `with_large_stack` path (issue #270 review follow-up).
#[test]
fn stdlib_env_reads_deep_data_on_large_stack() {
    with_large_stack(|| {
        let env = Environment::with_stdlib();
        let deep = nested_parens(5_000);
        let program = format!("(progn (read-from-string \"{deep}\") 'parsed)");
        let result = lamedh::eval_line(&program, &env);
        assert_eq!(
            result, "PARSED",
            "expected a 5,000-deep form to parse under the 50,000 stdlib limit, got: {result}"
        );
    });
}

// The default 512 limit is sized for stacks >= ~4 MiB (each nesting level
// costs roughly 6.4 KB of native stack in a debug build). On a genuinely
// small stack the caller must lower the limit via the `_with_depth_limit`
// API — this is what actually protects a 2 MiB embedder thread. Limit 150
// bounds worst-case parser stack at ~1 MiB, half the 2 MiB thread.
#[test]
fn small_stack_needs_lower_limit() {
    let handle = std::thread::Builder::new()
        .stack_size(2 * 1024 * 1024)
        .spawn(|| {
            let env = Shared::new(Environment::new());
            // Comfortably under the lowered limit: parses fine.
            let ok = lamedh::reader::read_with_depth_limit(&nested_parens(100), &env, 150);
            assert!(ok.is_ok(), "depth 100 under limit 150 should parse: {ok:?}");
            // Pathological input: clean error, no stack overflow, on 2 MiB.
            let err = lamedh::reader::read_with_depth_limit(&nested_parens(20_000), &env, 150);
            assert!(err.is_err(), "expected a parse error, got {err:?}");
            assert!(err.unwrap_err().contains("nesting too deep"));
        })
        .expect("spawn 2 MiB thread");
    handle.join().expect("2 MiB reader thread must not crash");
}

// (d) Reasonable nesting well under the limit still parses normally.
#[test]
fn reasonable_nesting_still_parses() {
    let env = Shared::new(Environment::new());
    let depth = 400; // comfortably below DEFAULT_READER_DEPTH (512)
    let input = nested_parens(depth);
    let result = lamedh::reader::read(&input, &env);
    assert!(
        result.is_ok(),
        "expected depth {depth} nesting to parse fine, got: {result:?}"
    );

    let input = quote_chain(depth);
    let result = lamedh::reader::read(&input, &env);
    assert!(
        result.is_ok(),
        "expected a {depth}-deep quote chain to parse fine, got: {result:?}"
    );
}

// (e) Normal, non-pathological programs (including the full embedded
// standard library, which loads via the same reader path) are unaffected by
// the depth limit.
#[test]
fn normal_programs_unaffected() {
    with_large_stack(|| {
        // Loading the whole stdlib exercises the reader on every form in
        // lib/*.lisp; none of them come close to any depth limit.
        let env = env_with_stdlib();
        assert_eq!(lamedh::eval_line("(+ 1 2 3)", &env), "6");
        assert_eq!(lamedh::eval_line("(let ((x 1) (y 2)) (+ x y))", &env), "3");
        assert_eq!(
            lamedh::eval_line("(eval (read-from-string \"(+ 1 2)\"))", &env),
            "3"
        );
        // A typical, ordinarily-nested quasiquote form.
        assert_eq!(
            lamedh::eval_line("`(1 ,(+ 1 1) ,@(list 3 4))", &env),
            "(1 2 3 4)"
        );
    });
}
