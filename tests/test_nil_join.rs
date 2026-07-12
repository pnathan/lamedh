//! #336: the checker's on-demand DERIVED-scheme path (#308) was committing
//! an `any`-vs-`nil` `if`/`cond` branch join to `(list a)` instead of
//! degrading to `any`, the same honesty rule the declared layer already
//! applies to nil-on-miss functions (lib/28-types.lisp rule 1). That
//! rejected the standard "parse and fall back on a miss" guard idiom:
//!
//!   (defun count-or-default (s)
//!     (let ((n (parse-integer s)))
//!       (if (numberp n) n 10)))
//!
//! because `parse-integer`'s own body — `(if test n nil)` — bound `n`'s
//! free result-type variable to `(list _)` against the literal `nil` miss
//! branch, so `count-or-default`'s own `if` then saw `(list _)` vs `int64`
//! and errored.

mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

#[test]
fn guard_idiom_checks_with_correct_scheme() {
    let e = env_with_stdlib();
    eval_line(
        "(defun count-or-default (s) (let ((n (parse-integer s))) (if (numberp n) n 10)))",
        &e,
    );
    assert_eq!(
        eval_line("(see-type 'count-or-default)", &e),
        "(CHECKED (FORALL (A) (-> (A) INT64)))"
    );
    // And it actually runs, both the hit and the miss path.
    assert_eq!(eval_line("(count-or-default \"42\")", &e), "42");
    assert_eq!(eval_line("(count-or-default \"xx\")", &e), "10");
}

#[test]
fn cond_guard_idiom_also_checks() {
    // The identical idiom spelled with `cond` hit the same defect through
    // `elab_cond`'s clause-join, not just `elab_if`.
    let e = env_with_stdlib();
    eval_line(
        "(defun count-or-default2 (s) \
           (let ((n (parse-integer s))) \
             (cond ((numberp n) n) (t 10))))",
        &e,
    );
    let out = eval_line("(see-type 'count-or-default2)", &e);
    assert!(out.starts_with("(CHECKED"), "got: {out}");
    assert!(out.contains("INT64"), "got: {out}");
    assert_eq!(eval_line("(count-or-default2 \"42\")", &e), "42");
    assert_eq!(eval_line("(count-or-default2 \"xx\")", &e), "10");
}

#[test]
fn nil_on_miss_functions_stay_gradual_and_still_run() {
    let e = env_with_stdlib();
    // `parse-integer` itself: no declared scheme (honesty rule 1), and its
    // on-demand derived scheme is now fully gradual on the result — not
    // `(-> (a) (list b))` (the pre-#336 bug).
    let out = eval_line("(see-type 'parse-integer)", &e);
    assert!(out.starts_with("(CHECKED"), "got: {out}");
    assert!(
        !out.contains("LIST"),
        "parse-integer must not be list-typed: {out}"
    );
    // Runtime behavior is untouched: hit is the integer, miss is NIL.
    assert_eq!(eval_line("(parse-integer \"42\")", &e), "42");
    assert_eq!(eval_line("(parse-integer \"nope\")", &e), "()");
}

#[test]
fn genuinely_list_returning_function_still_infers_list() {
    // Adversarial case: a function that ACTUALLY returns a list (or nil)
    // must keep its `(list _)` scheme — the #336 fix only degrades a
    // nil-vs-NON-list join; nil-vs-list still unifies as a list exactly as
    // before.
    let e = env_with_stdlib();
    eval_line("(defun lon (p x) (if p (list x) nil))", &e);
    assert_eq!(
        eval_line("(see-type 'lon)", &e),
        "(CHECKED (FORALL (A B) (-> (A B) (LIST B))))"
    );
    assert_eq!(eval_line("(lon t 5)", &e), "(5)");
    assert_eq!(eval_line("(lon () 5)", &e), "()");
}

#[test]
fn nil_vs_list_join_still_unifies_as_a_list() {
    // Both branches list-shaped (one via a literal nil, one via a real
    // list expression) must still unify to ONE list type, not degrade to
    // `any` — the honesty-rule degrade only fires when the non-nil side is
    // NOT a list.
    let e = env_with_stdlib();
    eval_line("(defun maybe-pair (p) (if p (list 1 2) nil))", &e);
    assert_eq!(
        eval_line("(see-type 'maybe-pair)", &e),
        "(CHECKED (FORALL (A) (-> (A) (LIST INT64))))"
    );
}

#[test]
fn cond_default_nil_clause_still_checks() {
    // `cond`'s common "fall through to nil" default clause, mixed with a
    // genuine list-returning clause, still unifies to a list.
    let e = env_with_stdlib();
    eval_line(
        "(defun cond-list-or-nil (p) (cond (p (list 1)) (t nil)))",
        &e,
    );
    let out = eval_line("(see-type 'cond-list-or-nil)", &e);
    assert!(out.contains("LIST"), "got: {out}");
    assert!(!out.contains("TYPE-ERROR"), "got: {out}");
}

#[test]
fn wrong_concrete_use_is_any_degraded_not_a_hard_error() {
    // A downstream misuse of a nil-on-miss result — passing it somewhere
    // that demands a concrete type — still type-checks (it is NOT rejected
    // as a hard error) because the derived result stays a genuinely free
    // type variable that then specializes at the call site, exactly like
    // any other ungeneralized nil-on-miss consumer (see the pre-existing
    // `(car (parse-integer s))` "CHECKED (VACUOUS)" case #336 names as a
    // related, explicitly out-of-scope imprecision). This is the
    // documented gradual-frontier behavior, not a soundness hole: nothing
    // here binds a CONCRETE ground type incorrectly against another
    // concrete ground type — a genuinely concrete conflict (e.g. passing a
    // literal int where a string is demanded) still errors, see
    // `genuinely_concrete_conflicts_still_error` below.
    let e = env_with_stdlib();
    eval_line("(defun bad (s) (concat (parse-integer s) \"x\"))", &e);
    let out = eval_line("(see-type 'bad)", &e);
    assert!(out.starts_with("(CHECKED"), "got: {out}");
}

#[test]
fn self_recursive_nil_on_miss_helper_stays_gradual_not_a_wrong_concrete_type() {
    // `$require-resolve-disk` (lib/06-require.lisp) is SELF-RECURSIVE and
    // hits a subtler variant of #336: its recursive call site unifies
    // against the checker's self-recursion "assumed return type"
    // placeholder *while the body is still being elaborated*, which can
    // concretize that placeholder (via the sibling `(cons (read-file …)
    // …)` branch) BEFORE the function's own top-level nil-vs-non-list `if`
    // join decides the honest answer is `any`. Left alone, that
    // concretization leaks past the honesty-rule degrade into the
    // generalized scheme, and since `$require-resolve` and `$require-load`
    // call it transitively, the wrong concrete type then breaks `$require-
    // load`'s own `(null resolved)` check — turning a previously-CHECKED,
    // working function into a hard TYPE-ERROR. `Cx::check_callee` now
    // detects exactly this (the body's own final type resolves to `any`
    // while the assumed return variable was already bound concretely) and
    // forces the assumption back to `any` before generalizing.
    let e = env_with_stdlib();
    let out = eval_line("(see-type '$require-load)", &e);
    assert!(out.starts_with("(CHECKED"), "got: {out}");
    let out = eval_line("(see-type '$require-resolve)", &e);
    assert!(out.starts_with("(CHECKED"), "got: {out}");
    // And REQUIRE actually still runs end to end.
    assert_eq!(eval_line("(require 'json)", &e), "JSON");
    assert_eq!(eval_line("(module-loaded-p 'json)", &e), "T");
}

#[test]
fn genuinely_concrete_conflicts_still_error() {
    let e = env_with_stdlib();
    // Two concrete, non-nil branches that truly disagree must still error.
    eval_line("(defun bad-if (x) (if x 1 2.0))", &e);
    let out = eval_line("(see-type 'bad-if)", &e);
    assert!(out.contains("TYPE-ERROR"), "got: {out}");
    assert!(out.contains("branches disagree"), "got: {out}");

    eval_line("(defun bad-cond (x) (cond (x 1) (t 2.0)))", &e);
    let out = eval_line("(see-type 'bad-cond)", &e);
    assert!(out.contains("TYPE-ERROR"), "got: {out}");
    assert!(out.contains("clauses disagree"), "got: {out}");

    // A literal concrete type passed where a nil-on-miss consumer expects
    // one thing still errors too.
    eval_line("(defun bad-concat (s) (concat 5 s))", &e);
    let out = eval_line("(see-type 'bad-concat)", &e);
    assert!(out.contains("TYPE-ERROR"), "got: {out}");
}
