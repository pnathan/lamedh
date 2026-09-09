//! Per-call-site macro expansion caching (issue #460).
//!
//! Only compiled call sites (`Code::Call`) participate: a macro call inside
//! a `defun`/`lambda` body is compiled once, and every test here therefore
//! calls the macro *through a defined function*, calling that function
//! repeatedly to exercise the same cached call site multiple times. A bare
//! top-level form (evaluated once by the tree-walker) never benefits from —
//! or is affected by — the cache, which is also exercised below to contrast
//! the two paths. `VAU`/fexpr dispatch is untouched by this feature and
//! stays fresh every call; that is pinned incidentally by the mixed-operative
//! tests already in `tests/test_compiled_m2.rs` and elsewhere, not repeated
//! here.

mod test_helpers;
use lamedh::environment::Environment;
use lamedh::{eval_line, with_large_stack};
use test_helpers::env_with_stdlib;

/// Baseline: a cached call site (through a `defun`, called many times) must
/// produce exactly the same result as the uncached tree-walker path
/// (the same macro call written out at top level, once per iteration).
#[test]
fn cached_path_matches_uncached_path_output() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        eval_line(
            "(defmacro sq-form (x) (list '* x x))",
            &env,
        );
        eval_line("(defun sq (x) (sq-form x))", &env);
        for n in 0..20i64 {
            let compiled = eval_line(&format!("(sq {n})"), &env);
            let interpreted = eval_line(&format!("(sq-form {n})"), &env);
            assert_eq!(
                compiled, interpreted,
                "cached call-site result diverged from the uncached tree-walker \
                 expansion for n={n}"
            );
            assert_eq!(compiled, (n * n).to_string());
        }
    });
}

/// The accepted semantic change from the issue, pinned directly: a macro
/// body that reads global/dynamic state now observes it **once**, at the
/// call site's first expansion — not on every call. This is a deliberate,
/// documented behavior change (KERNEL.md Part VI), not a bug.
#[test]
fn cached_macro_observes_global_state_once_per_call_site() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        eval_line("(defvar *tag* 'first)", &env);
        eval_line("(defmacro tagged () (list 'quote *tag*))", &env);
        eval_line("(defun get-tag () (tagged))", &env);

        // First call through the compiled call site expands (and caches)
        // while *tag* is FIRST.
        assert_eq!(eval_line("(get-tag)", &env), "FIRST");

        // Mutate the global *after* the call site has cached.
        eval_line("(setq *tag* 'second)", &env);

        // A cache hit must NOT re-observe the new value: this is the whole
        // point of caching, and the one accepted behavior change.
        assert_eq!(
            eval_line("(get-tag)", &env),
            "FIRST",
            "a cached macro call site must not re-observe global state \
             mutated after its first expansion"
        );

        // Contrast: the uncached tree-walker path (a bare top-level call,
        // never compiled into a Code::Call node) DOES see the live value.
        assert_eq!(eval_line("(tagged)", &env), "SECOND");

        // Contrast: macroexpand always performs one fresh expansion step
        // regardless of any call-site cache.
        assert_eq!(eval_line("(macroexpand '(tagged))", &env), "(QUOTE SECOND)");
    });
}

/// Redefining a macro (`defmacro` again under the same name) produces a new
/// `Shared<Macro>` identity, so a call site holding a cache keyed to the old
/// macro must miss and re-expand against the new definition on its very
/// next call.
#[test]
fn redefining_macro_invalidates_cache_on_next_call() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        eval_line("(defmacro pick (x y) x)", &env);
        eval_line("(defun run-pick (a b) (pick a b))", &env);

        // Populate the cache with the V1 expansion (returns the first arg).
        assert_eq!(eval_line("(run-pick 1 2)", &env), "1");
        assert_eq!(eval_line("(run-pick 3 4)", &env), "3");

        // Redefine the macro under the same name: new Shared<Macro> value.
        eval_line("(defmacro pick (x y) y)", &env);

        // The next call at the SAME compiled call site must observe the
        // redefinition, not the stale cached expansion.
        assert_eq!(
            eval_line("(run-pick 5 6)", &env),
            "6",
            "cache must invalidate on macro redefinition"
        );
        assert_eq!(eval_line("(run-pick 7 8)", &env), "8");
    });
}

/// A macro whose expansion errors must never be cached: the error is not a
/// valid expansion, and the next call at the same call site must retry
/// expansion from scratch (e.g. against a since-fixed environment) rather
/// than replaying the failure or, worse, caching garbage.
#[test]
fn erroring_expansion_is_never_cached_and_retries() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        eval_line("(defvar *ready* nil)", &env);
        // Expansion itself errors (via the macro body, not the expanded
        // code) while *ready* is NIL; once *ready* is T it expands cleanly.
        eval_line(
            "(defmacro maybe-ok () (if *ready* ''ok (error \"not ready\")))",
            &env,
        );
        eval_line("(defun try-it () (maybe-ok))", &env);

        let first = eval_line("(try-it)", &env);
        assert!(
            first.contains("not ready") || first.to_lowercase().contains("error"),
            "expected the first call to surface the expansion error, got: {first}"
        );

        // Fix the condition and retry at the SAME call site: this only
        // works if the failed expansion was never memoized.
        eval_line("(setq *ready* t)", &env);
        assert_eq!(
            eval_line("(try-it)", &env),
            "OK",
            "a call site must retry expansion after a prior expansion error, \
             never cache the failure"
        );

        // And once a good expansion IS cached, flipping *ready* back to NIL
        // must not un-cache it (ordinary cache-hit behavior).
        eval_line("(setq *ready* nil)", &env);
        assert_eq!(eval_line("(try-it)", &env), "OK");
    });
}

/// Nested/mutually-referential macro expansion: one macro's expansion
/// (once compiled) contains a call to a second macro. Each nesting level's
/// `Code::Call` node gets its own independent cache slot — nothing is
/// flattened together — so this must behave identically to the uncached
/// path across many calls with varying runtime argument values (the cached
/// operand *forms* never change; only the values they evaluate to do).
#[test]
fn nested_macro_expansion_caches_each_level_independently() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        eval_line(
            "(defmacro my-when (test &rest body) (list 'if test (cons 'progn body)))",
            &env,
        );
        eval_line(
            "(defmacro clamp-pos (x) (list 'my-when (list '> x 0) x))",
            &env,
        );
        eval_line("(defun classify (x) (clamp-pos x))", &env);

        // Exercise the same call site repeatedly with different runtime
        // values so a wrongly-flattened or wrongly-shared cache would show
        // up as stale output.
        for n in [-5, -1, 0, 1, 5, 100, -100, 2, -2, 0].into_iter() {
            let expected = if n > 0 {
                n.to_string()
            } else {
                "()".to_string()
            };
            assert_eq!(
                eval_line(&format!("(classify {n})"), &env),
                expected,
                "nested macro expansion diverged for n={n}"
            );
        }
    });
}

/// A recursive macro (each expansion step re-dispatches through the SAME
/// `Code::Call` node, since it's a self-tail-call inside the macro's own
/// compiled expansion) must still terminate and produce the right answer:
/// caching an expansion must not accidentally memoize across different
/// recursion depths.
#[test]
fn self_recursive_macro_terminates_and_caches_per_call_site() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        // Expands to itself with a decremented counter until zero, then to
        // the accumulated list length via a literal.
        eval_line(
            "(defmacro count-down (n) \
               (if (= n 0) 0 (list '+ 1 (list 'count-down (- n 1)))))",
            &env,
        );
        eval_line("(defun depth5 () (count-down 5))", &env);
        eval_line("(defun depth9 () (count-down 9))", &env);
        assert_eq!(eval_line("(depth5)", &env), "5");
        assert_eq!(eval_line("(depth9)", &env), "9");
        // Call again through the same compiled call sites: cache hits.
        assert_eq!(eval_line("(depth5)", &env), "5");
        assert_eq!(eval_line("(depth9)", &env), "9");
    });
}

/// A forked world (`Environment::fork_world`, underlying `with_stdlib`'s
/// deep-copy fork) must NOT inherit a prototype world's already-populated
/// call-site caches. A cached expansion's `code` and `macro_id` hold
/// prototype-world symbol cells; carrying it into the fork would be a
/// cross-world identity leak. `copy_code` resets the slot to `None`, so the
/// forked call site starts cold and re-expands against its own copied
/// macro binding on first use.
#[test]
fn forked_world_does_not_inherit_cached_expansion() {
    with_large_stack(|| {
        let proto = Environment::with_stdlib();
        eval_line("(defvar *label* 'from-proto)", &proto);
        eval_line("(defmacro labeled () (list 'quote *label*))", &proto);
        eval_line("(defun get-label () (labeled))", &proto);

        // Populate the prototype's call-site cache.
        assert_eq!(eval_line("(get-label)", &proto), "FROM-PROTO");

        // Fork AFTER the cache is populated.
        let forked = Environment::fork_world(&proto).expect("fork_world should succeed");

        // The fork must be fully isolated and functionally correct: if the
        // cached Code (holding prototype symbol cells) had leaked across,
        // this would at best silently read/write the wrong world's cells
        // and at worst panic on a dangling/foreign reference.
        assert_eq!(eval_line("(get-label)", &forked), "FROM-PROTO");

        // Prove isolation concretely: redefine the macro and mutate the
        // dynamic var in the PROTOTYPE only, after the fork was taken.
        eval_line("(setq *label* 'proto-mutated)", &proto);
        eval_line("(defmacro labeled () (list 'quote 'proto-redefined))", &proto);

        // The prototype's own cache is now stale (redefinition invalidates
        // it) and re-expands to the new definition.
        assert_eq!(eval_line("(get-label)", &proto), "PROTO-REDEFINED");

        // The forked world's call site must be entirely unaffected — both
        // by the prototype's mutation of *label* and by its macro
        // redefinition — because it never shared the prototype's cache OR
        // its macro/symbol identities.
        assert_eq!(
            eval_line("(get-label)", &forked),
            "FROM-PROTO",
            "forked world must not observe prototype-world mutations through \
             a shared cache slot"
        );
    });
}

/// `APPLY` on a macro value is documented to stay uncached (it never goes
/// through `Code::Call`): each `apply` re-expands, so it must observe live
/// global state exactly like the tree-walker path, even though a `defun`
/// wrapping an ordinary call to the same macro is caching that call site.
#[test]
fn apply_on_macro_stays_uncached() {
    with_large_stack(|| {
        let env = env_with_stdlib();
        eval_line("(defvar *v* 1)", &env);
        eval_line("(defmacro readv () (list 'quote *v*))", &env);
        eval_line("(defun via-call () (readv))", &env);

        // Cache the compiled call site.
        assert_eq!(eval_line("(via-call)", &env), "1");
        eval_line("(setq *v* 2)", &env);
        assert_eq!(eval_line("(via-call)", &env), "1"); // cache hit, stale by design

        // APPLY on the same macro value must see the live value.
        assert_eq!(
            eval_line("(apply 'readv '())", &env),
            "2",
            "APPLY on a macro must always re-expand fresh, never via the \
             Code::Call cache"
        );
    });
}
