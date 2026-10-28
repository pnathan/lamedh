//! The non-tail call depth cap for typed (compiled) code is a per-thread
//! host setting, not a constant. A host whose native stack is far smaller than
//! the 512 MiB `with_large_stack` thread (wasm32 in a browser) must be able to
//! lower it so the recursion-limit error fires before the stack overflows.
//! It is separate from `set_eval_depth_limit`, which the typed code never sees.

use lamedh::environment::Environment;
use lamedh::printer::print;
use lamedh::{
    DEFAULT_TYPED_CALL_DEPTH_LIMIT, Shared, eval_str, set_eval_depth_limit,
    set_typed_call_depth_limit, typed_call_depth_limit, with_large_stack,
};

fn ok(src: &str, e: &Shared<Environment>) -> String {
    print(&eval_str(src, e).unwrap_or_else(|err| panic!("{src}: {err}")))
}

fn err_of(src: &str, e: &Shared<Environment>) -> String {
    match eval_str(src, e) {
        Ok(v) => panic!("{src}: expected an error, got {}", print(&v)),
        Err(err) => format!("{err}"),
    }
}

/// Define `consume`, a non-tail recursion that descends once per unit of the
/// budget held in slot 0 of `g`, and check it compiled.
fn define_consume(e: &Shared<Environment>) {
    eval_str(
        "(defun consume (a) \
           (if (= (fetch a 0) 0) 0 \
               (progn (store a 0 (- (fetch a 0) 1)) (+ 1 (consume a)))))",
        e,
    )
    .unwrap();
    assert!(ok("(see-type 'consume)", e).ends_with("COMPILED)"));
    eval_str("(setq g (typed-array 1 'int64))", e).unwrap();
}

fn budget(n: i64, e: &Shared<Environment>) {
    eval_str(&format!("(store g 0 {n})"), e).unwrap();
}

#[test]
fn the_default_is_the_documented_constant() {
    with_large_stack(|| {
        assert_eq!(typed_call_depth_limit(), DEFAULT_TYPED_CALL_DEPTH_LIMIT);
    });
}

#[test]
fn a_lowered_limit_fires_at_that_depth_and_a_restored_one_does_not() {
    with_large_stack(|| {
        let e = Environment::with_stdlib();
        define_consume(&e);

        set_typed_call_depth_limit(30);
        assert_eq!(typed_call_depth_limit(), 30);
        budget(20, &e);
        assert_eq!(ok("(consume g)", &e), "20");
        budget(100, &e);
        let msg = err_of("(consume g)", &e);
        assert!(
            msg.contains("recursion limit exceeded (30 non-tail typed calls)"),
            "{msg}"
        );

        set_typed_call_depth_limit(DEFAULT_TYPED_CALL_DEPTH_LIMIT);
        budget(100, &e);
        assert_eq!(ok("(consume g)", &e), "100");
    });
}

#[test]
fn the_eval_depth_limit_does_not_move_the_typed_limit() {
    with_large_stack(|| {
        let e = Environment::with_stdlib();
        define_consume(&e);

        set_eval_depth_limit(50);
        assert_eq!(typed_call_depth_limit(), DEFAULT_TYPED_CALL_DEPTH_LIMIT);
        budget(200, &e);
        assert_eq!(ok("(consume g)", &e), "200");
    });
}
