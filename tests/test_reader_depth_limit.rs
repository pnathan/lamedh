//! Integration tests for issue #270: unbounded reader recursion used to crash
//! the process (native stack overflow) on pathologically deep input such as
//! tens of thousands of nested `(` or a long `'''...` quote chain. The reader
//! now bounds recursion at `reader::MAX_READER_DEPTH` and reports a normal
//! parse error instead.
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

// (a) 20k-deep nested parens: must return a parse error, not crash the
// process. Run directly on the default test-harness thread (no
// with_large_stack) since the reader must be safe there too — the whole
// point of the fix is that callers who parse without an oversized stack
// (e.g. a host embedding the library and calling `reader::read` directly)
// are protected.
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
        msg.contains(&lamedh::reader::MAX_READER_DEPTH.to_string()),
        "expected the message to carry the configured limit, got: {msg}"
    );
}

// (b) A long quote chain ('''''...) recurses through a different set of
// reader productions (parse_quoted -> parse_expr) than nested parens; make
// sure it is bounded too.
#[test]
fn deep_quote_chain_error_not_crash() {
    let env = Shared::new(Environment::new());
    let input = quote_chain(20_000);
    let result = lamedh::reader::read(&input, &env);
    assert!(result.is_err(), "expected a parse error, got {result:?}");
    let msg = result.unwrap_err();
    assert!(
        msg.contains("nesting too deep"),
        "expected a 'nesting too deep' message, got: {msg}"
    );
}

// Same as (a)/(b) but also exercised from a with_large_stack (512 MiB)
// thread, since file loading and the REPL run there in the CLI — the depth
// limit must trigger well before either stack context is exhausted.
#[test]
fn deeply_nested_parens_error_not_crash_on_large_stack() {
    with_large_stack(|| {
        let env = Shared::new(Environment::new());
        let input = nested_parens(20_000);
        assert!(lamedh::reader::read(&input, &env).is_err());

        let input = quote_chain(20_000);
        assert!(lamedh::reader::read(&input, &env).is_err());
    });
}

// (c) `read-from-string` on deeply nested input must surface as a catchable
// Lisp error (via handler-case), not as a host-level panic/crash, since it is
// reachable at runtime from ordinary Lisp code (issue #270).
#[test]
fn read_from_string_deep_input_is_catchable_lisp_error() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        let deep = nested_parens(20_000);
        let program = format!("(handler-case (read-from-string \"{deep}\") (error (e) 'caught))");
        let result = lamedh::eval_line(&program, &env);
        assert_eq!(
            result, "CAUGHT",
            "expected the reader error to be caught as a Lisp condition, got: {result}"
        );
    });
}

// (d) Reasonable nesting well under the limit still parses normally.
#[test]
fn reasonable_nesting_still_parses() {
    let env = Shared::new(Environment::new());
    let depth = 400; // comfortably below MAX_READER_DEPTH (512)
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
        // lib/*.lisp; none of them come close to MAX_READER_DEPTH.
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
