//! Integration tests for the PORTABLE Hindley-Milner checker
//! (`lib/45-hm-check.lisp`, issue #451).
//!
//! The checker is a port of this host's own checker — `src/jit/infer.rs`'s
//! inference substrate, `src/jit/types.rs`'s type vocabulary,
//! `src/jit/elaboration.rs`'s elaborator in its CHECKING mode, and
//! `src/jit/registry.rs`'s declaration plane — into portable Lamedh source, so
//! every Lamedh host runs the same checking logic instead of reimplementing it
//! natively (the SBCL port, #449, is the immediate second consumer).
//!
//! What these tests hold it to, in order of importance:
//!
//! 1. **Wiring.** `defun`'s `$defun-auto-compile` hook, `defrecord`,
//!    `defvariant` and `definstance` all feed the portable registry, from
//!    their existing call sites, with no separate declaration step.
//! 2. **Coverage.** Nothing the standard library declares is dropped for want
//!    of a portable spelling (`hm-dropped-declarations` is empty after a full
//!    stdlib load), and the checker runs over every stdlib `defun` without
//!    crashing.
//! 3. **Fidelity.** Where both checkers can see the same thing, they say the
//!    same thing.
//! 4. **Honesty.** Anything outside coverage reports `DYNAMIC`/`ANY`, never a
//!    fabricated `CHECKED`.

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line};

fn env() -> Shared<Environment> {
    Environment::with_stdlib()
}

fn ev(e: &Shared<Environment>, src: &str) -> String {
    eval_line(src, e)
}

/// Do types `a` and `b` unify, in a fresh checker state? `st` is in scope for
/// both operands, so either may name a fresh row tail with `(hm-fresh st)`.
fn unifies(e: &Shared<Environment>, a: &str, b: &str) -> String {
    ev(
        e,
        &format!("(let ((st (hm-new-state))) (hm-unifies-p st {a} {b}))"),
    )
}

// ---------------------------------------------------------------------------
// The inference core.
// ---------------------------------------------------------------------------

#[test]
fn identity_generalizes_to_a_polymorphic_scheme() {
    let e = env();
    assert_eq!(
        ev(&e, "(hm-check-lambda '(x) '(x))"),
        "(CHECKED (FORALL (A) (-> (A) A)))"
    );
}

#[test]
fn arithmetic_constrains_operands_and_rejects_strings() {
    let e = env();
    assert_eq!(
        ev(&e, "(hm-check-lambda '(x) '((+ x 1)))"),
        "(CHECKED (-> (INT64) INT64))"
    );
    assert_eq!(
        ev(&e, "(car (hm-check-expr '(+ \"a\" \"b\")))"),
        "TYPE-ERROR"
    );
}

#[test]
fn division_arity_matches_the_evaluator() {
    // `/` and `mod` are strictly binary in the evaluator; the checker must not
    // invent a unary or variadic meaning it does not have.
    let e = env();
    assert_eq!(ev(&e, "(car (hm-check-expr '(/ 1)))"), "TYPE-ERROR");
    assert_eq!(ev(&e, "(car (hm-check-expr '(/ 1 2 3)))"), "TYPE-ERROR");
    assert_eq!(ev(&e, "(hm-check-expr '(/ 6 2))"), "(CHECKED INT64)");
}

#[test]
fn list_rules_infer_element_types() {
    let e = env();
    assert_eq!(
        ev(&e, "(hm-check-expr '(list 1 2 3))"),
        "(CHECKED (LIST INT64))"
    );
    assert_eq!(
        ev(&e, "(car (hm-check-expr '(list 1 \"s\")))"),
        "TYPE-ERROR"
    );
    assert_eq!(
        ev(&e, "(hm-check-expr '(car (list 1 2)))"),
        "(CHECKED INT64)"
    );
    // A cons onto a known non-list ground type is a dotted pair.
    assert_eq!(
        ev(&e, "(hm-check-expr '(cons 1 \"s\"))"),
        "(CHECKED (PAIR INT64 STRING))"
    );
}

#[test]
fn the_nil_branch_honesty_rule_degrades_instead_of_biasing() {
    let e = env();
    // A literal-nil branch meeting a non-list branch degrades to ANY rather
    // than forcing `(list _)` onto the other branch.
    assert_eq!(ev(&e, "(hm-check-expr '(if 1 2 nil))"), "(CHECKED ANY)");
    // ... but a nil branch meeting a genuine list branch still unifies.
    assert_eq!(
        ev(&e, "(hm-check-expr '(if 1 (list 1 2) nil))"),
        "(CHECKED (LIST INT64))"
    );
}

#[test]
fn occurs_check_rejects_an_infinite_type() {
    let e = env();
    // Two DISTINCT fresh variables unify happily...
    assert_eq!(
        unifies(&e, "(hm-fresh st)", "(list 'list (hm-fresh st))"),
        "T"
    );
    // ... but binding one INTO a type that contains it is the infinite type
    // the occurs-check exists to reject.
    let cyclic = "(let ((a (hm-fresh st))) (hm-unifies-p st a (list 'list a)))";
    assert_eq!(
        ev(&e, &format!("(let ((st (hm-new-state))) {cyclic})")),
        "()"
    );
}

#[test]
fn variadic_parameter_lists_are_reported_dynamic_not_checked() {
    // The honesty rule: a parameter grammar the checker does not model is
    // DYNAMIC, never a type-safety claim in either direction.
    let e = env();
    assert_eq!(
        ev(&e, "(hm-check-lambda '(x &rest r) '(x))"),
        "(DYNAMIC \"variadic parameter list\")"
    );
    assert_eq!(ev(&e, "(car (hm-check-lambda '(&key k) '(k)))"), "DYNAMIC");
}

// ---------------------------------------------------------------------------
// Row polymorphism — what lib/20-condensation.lisp's accessors depend on.
// ---------------------------------------------------------------------------

#[test]
fn record_ref_derives_an_open_row_with_no_axioms() {
    let e = env();
    assert_eq!(
        ev(&e, "(hm-check-lambda '(r) '((record-ref r 'x)))"),
        "(CHECKED (FORALL (A B) (-> ((RECORD ((X A)) B)) A)))"
    );
}

#[test]
fn one_row_typed_reader_accepts_every_conforming_brand() {
    let e = env();
    ev(&e, "(defrecord RowA (x int64) (y int64))");
    ev(&e, "(defrecord RowB (x int64) (z string))");
    assert_eq!(
        ev(&e, "(car (hm-check-expr '(record-ref (make-RowA 1 2) 'x)))"),
        "CHECKED"
    );
    assert_eq!(
        ev(
            &e,
            "(car (hm-check-expr '(record-ref (make-RowB 1 \"s\") 'x)))"
        ),
        "CHECKED"
    );
    // A field neither brand has is a static error, not a silent pass.
    assert_eq!(
        ev(
            &e,
            "(car (hm-check-expr '(record-ref (make-RowA 1 2) 'nope)))"
        ),
        "TYPE-ERROR"
    );
}

// ---------------------------------------------------------------------------
// Wiring 1: defrecord.
// ---------------------------------------------------------------------------

#[test]
fn defrecord_registers_the_brand_in_the_portable_registry() {
    let e = env();
    ev(&e, "(defrecord Pt (x int64) (y int64))");
    assert_eq!(ev(&e, "(hm-struct-p 'Pt)"), "T");
    assert_eq!(ev(&e, "(hm-struct-def 'Pt)"), "((X . INT64) (Y . INT64))");
    // A DYNAMIC-tier record (one non-native field) additionally generates a
    // per-accessor axiom, which lands in the portable declared table too,
    // rendered exactly as the native checker renders it.
    ev(&e, "(defrecord Label (n int64) (s string))");
    assert_eq!(
        ev(&e, "(hm-struct-def 'Label)"),
        "((N . INT64) (S . STRING))"
    );
    assert_eq!(
        ev(&e, "(hm-see-type 'Label-s)"),
        "(DECLARED (-> (LABEL) STRING))"
    );
}

#[test]
fn defrecord_brands_are_nominal_but_row_subsumable() {
    let e = env();
    ev(&e, "(defrecord Chest (w int64))");
    ev(&e, "(defrecord Crate (w int64))");
    // Same shape, different brand: nominally distinct.
    assert_eq!(unifies(&e, "'(struct Chest)", "'(struct Crate)"), "()");
    // Both subsume into an open row naming only W.
    for brand in ["Chest", "Crate"] {
        let row = "(list 'record (list (cons 'w 'int64)) (hm-fresh st))";
        assert_eq!(unifies(&e, &format!("'(struct {brand})"), row), "T");
    }
    // A CLOSED row must name every field of the brand.
    assert_eq!(
        unifies(&e, "'(struct Chest)", "(list 'record (list) nil)"),
        "()"
    );
}

#[test]
fn defrecord_compiled_tier_also_reaches_the_portable_registry() {
    // A record whose fields are all natively storable takes the compiled
    // tier, which expands to the host-only `defstruct-typed` special form —
    // the one registration channel with no portable entry point. The
    // condensation layer therefore declares it to the portable registry
    // explicitly, so BOTH tiers are covered.
    let e = env();
    ev(&e, "(defrecord Native2 (a int64) (b float64))");
    assert_eq!(ev(&e, "(record-compiled-p 'Native2)"), "T");
    assert_eq!(ev(&e, "(hm-struct-p 'Native2)"), "T");
    assert_eq!(
        ev(&e, "(hm-struct-def 'Native2)"),
        "((A . INT64) (B . FLOAT64))"
    );
}

// ---------------------------------------------------------------------------
// Wiring 2: defvariant.
// ---------------------------------------------------------------------------

#[test]
fn defvariant_registers_the_union_and_its_constructors() {
    let e = env();
    ev(
        &e,
        "(defvariant Shape2 (Circle2 (r int64)) (Rect2 (w int64) (h int64)))",
    );
    assert_eq!(ev(&e, "(hm-variant-p 'Shape2)"), "T");
    assert_eq!(ev(&e, "(hm-variant-ctors 'Shape2)"), "(CIRCLE2 RECT2)");
    assert_eq!(ev(&e, "(hm-struct-def 'Circle2)"), "((R . INT64))");
    // A constructor brand absorbs into its variant, both argument orders.
    assert_eq!(unifies(&e, "'(struct Circle2)", "'(variant Shape2)"), "T");
    assert_eq!(unifies(&e, "'(variant Shape2)", "'(struct Rect2)"), "T");
}

#[test]
fn variant_case_binds_fields_and_joins_clause_bodies() {
    let e = env();
    ev(&e, "(defvariant Box2 (Full2 (v int64)) (Empty2))");
    assert_eq!(
        ev(
            &e,
            "(hm-check-lambda '(b) '((variant-case b (Full2 (v) v) (Empty2 () 0))))"
        ),
        "(CHECKED (-> (BOX2) INT64))"
    );
    // Disagreeing clause bodies are a static error.
    assert_eq!(
        ev(
            &e,
            "(car (hm-check-lambda '(b) '((variant-case b (Full2 (v) v) (Empty2 () \"s\")))))"
        ),
        "TYPE-ERROR"
    );
}

#[test]
fn parametric_variants_from_the_stdlib_are_registered_generically() {
    // OPTION and RESULT are ordinary parametric variants declared in
    // lib/25-variants.lisp; the portable registry learned them from the same
    // expansion the native one did.
    let e = env();
    assert_eq!(ev(&e, "(hm-generic-p 'option)"), "T");
    assert_eq!(ev(&e, "(hm-generic-arity (hm-generic-def 'option))"), "1");
    // A constructor application absorbs into its variant's application,
    // arguments pairwise.
    assert_eq!(
        unifies(&e, "'(app some (int64))", "'(app option (int64))"),
        "T"
    );
    assert_eq!(
        unifies(&e, "'(app some (int64))", "'(app option (string))"),
        "()"
    );
}

// ---------------------------------------------------------------------------
// Wiring 3: protocol instances.
// ---------------------------------------------------------------------------

#[test]
fn protocol_instances_reach_the_portable_registry_and_dispatch() {
    let e = env();
    assert_eq!(ev(&e, "(hm-protocol-p 'length)"), "T");
    assert_eq!(
        ev(&e, "(< 0 (length (hm-protocol-instances 'length)))"),
        "T"
    );
    // Every LENGTH instance returns int64, so even an unresolved dispatch
    // argument still yields the shared ground result.
    assert_eq!(
        ev(&e, "(hm-check-lambda '(x) '((length x)))"),
        "(CHECKED (FORALL (A) (-> (A) INT64)))"
    );
    // A resolved dispatch argument selects the matching instance.
    assert_eq!(
        ev(&e, "(hm-check-expr '(length (list 1 2)))"),
        "(CHECKED INT64)"
    );
}

#[test]
fn a_new_definstance_is_visible_to_the_portable_checker() {
    let e = env();
    ev(&e, "(defrecord Metre (n int64))");
    ev(&e, "(defprotocol scale2)");
    ev(&e, "(definstance scale2 ((m Metre) (k int64)) Metre m)");
    assert_eq!(ev(&e, "(hm-protocol-p 'scale2)"), "T");
    assert_eq!(ev(&e, "(length (hm-protocol-instances 'scale2))"), "1");
    assert_eq!(
        ev(&e, "(hm-check-expr '(scale2 (make-Metre 1) 2))"),
        "(CHECKED METRE)"
    );
}

#[test]
fn protocol_dispatch_position_is_registered_portably() {
    let e = env();
    ev(&e, "(defprotocol fnfirst2 (:dispatch 1))");
    assert_eq!(ev(&e, "(hm-protocol-dispatch-index 'fnfirst2)"), "1");
    assert_eq!(ev(&e, "(hm-protocol-dispatch-index 'length)"), "0");
}

// ---------------------------------------------------------------------------
// Wiring 4: declare-type! axioms, and the defun hook.
// ---------------------------------------------------------------------------

#[test]
fn declared_axioms_are_registered_and_consumed_at_call_sites() {
    let e = env();
    ev(
        &e,
        "(declare-type! 'axiom-demo '(forall (a) (-> ((list a)) a)))",
    );
    assert_eq!(
        ev(&e, "(hm-see-type 'axiom-demo)"),
        "(DECLARED (FORALL (A) (-> ((LIST A)) A)))"
    );
    assert_eq!(
        ev(&e, "(hm-check-expr '(axiom-demo (list 1 2)))"),
        "(CHECKED INT64)"
    );
    assert_eq!(
        ev(&e, "(car (hm-check-expr '(axiom-demo 1)))"),
        "TYPE-ERROR"
    );
}

#[test]
fn the_defun_hook_drives_the_portable_checker_on_every_definition() {
    let e = env();
    ev(&e, "(hm-check-policy! 'eager)");
    ev(&e, "(defun hook-demo (a b) (+ a b))");
    // The verdict was computed through `$defun-auto-compile` — the one door
    // every DEFUN in the language routes through — and cached.
    assert_eq!(ev(&e, "(car (hm-verdict 'hook-demo))"), "CHECKED");
    // Redefinition invalidates it, so the verdict always tracks the body.
    ev(&e, "(defun hook-demo (a b) (concat a b))");
    assert_eq!(
        ev(&e, "(hm-verdict 'hook-demo)"),
        "(CHECKED (-> (STRING STRING) STRING))"
    );
    ev(&e, "(hm-check-policy! 'lazy)");
}

#[test]
fn lazy_is_the_default_policy_and_still_invalidates() {
    // Bodies chosen so the one-door `defun` leaves them INTERPRETED (string
    // and list types are outside the compileable lattice): a natively
    // compiled function is a membrane, not a plain lambda, and the portable
    // checker reports that honestly as DYNAMIC — see
    // `the_condensation_layer_runs_off_the_portable_checker`.
    let e = env();
    assert_eq!(ev(&e, "(hm-check-policy)"), "LAZY");
    ev(&e, "(defun lazy-demo (x) (concat x \"!\"))");
    assert_eq!(
        ev(&e, "(hm-verdict 'lazy-demo)"),
        "(CHECKED (-> (STRING) STRING))"
    );
    ev(&e, "(defun lazy-demo (x) (car x))");
    assert_eq!(
        ev(&e, "(hm-verdict 'lazy-demo)"),
        "(CHECKED (FORALL (A) (-> ((LIST A)) A)))"
    );
}

// ---------------------------------------------------------------------------
// Coverage and fidelity against the REAL standard library.
// ---------------------------------------------------------------------------

#[test]
fn no_stdlib_declaration_is_dropped_by_the_portable_registry() {
    // The portable registry is fed by wrapping the same entry points the
    // native one uses, so a full stdlib load must leave it knowing every
    // declaration the stdlib makes. A non-empty list here means either a
    // parser gap or a registration channel this file does not cover — the one
    // thing that could make the checker quietly under-report.
    let e = env();
    assert_eq!(ev(&e, "(hm-dropped-declarations)"), "()");
}

#[test]
fn the_portable_registry_learned_the_stdlibs_own_records_and_variants() {
    // Spot-check the registrations that come from real stdlib code rather
    // than from a test fixture: OPTION/RESULT (lib/25-variants.lisp) and the
    // protocol table (lib/29-protocols.lisp).
    let e = env();
    assert_eq!(ev(&e, "(hm-generic-p 'option)"), "T");
    assert_eq!(ev(&e, "(hm-generic-p 'result)"), "T");
    assert_eq!(ev(&e, "(hm-struct-p 'some)"), "()"); // parametric, not plain
    assert_eq!(ev(&e, "(hm-protocol-p 'length)"), "T");
    // And the declared-axiom table picked up lib/28-types.lisp's entries.
    assert_eq!(
        ev(&e, "(hm-see-type 'string-upcase)"),
        "(DECLARED (-> (STRING) STRING))"
    );
}

#[test]
fn the_checker_runs_over_real_stdlib_definitions_without_crashing() {
    // The scale test. `$cg-pending` accumulates the name of every function
    // DEFUN ever defined, so it IS the whole standard library; this walks a
    // strided sample across all of it (every 10th name, so every source file
    // is represented) and requires every result to be one of the four honest
    // verdicts — no crash, no hang, no fabricated status.
    //
    // Strided rather than exhaustive purely for test runtime: checking is a
    // tree-walked analysis whose cost is the callee closure it has to derive,
    // and the expensive tail is this checker checking *itself* (one ~60-
    // function mutually recursive graph). The full sweep is a maintenance
    // exercise, not a per-commit one.
    let e = env();
    ev(
        &e,
        "(defun stride (n xs) \
           (cond ((null xs) nil) \
                 ((= n 0) (cons (car xs) (stride 9 (cdr xs)))) \
                 (t (stride (- n 1) (cdr xs)))))",
    );
    let out = ev(
        &e,
        "(let ((names (stride 0 (remove-duplicates $cg-pending)))) \
           (list (length names) \
                 (every (lambda (n) \
                          (member (car (hm-verdict n)) \
                                  '(checked declared type-error dynamic))) \
                        names)))",
    );
    // (count T) — a large sample and a clean sweep.
    let count: i64 = out
        .trim_start_matches('(')
        .split(' ')
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    assert!(
        count > 40,
        "expected a stdlib-scale sample, got {count} names"
    );
    assert!(out.ends_with(" T)"), "not every verdict was honest: {out}");
}

#[test]
fn portable_and_native_agree_wherever_both_can_see_the_same_thing() {
    // Fidelity, measured against the real standard library rather than
    // fixtures. The two checkers are comparable exactly where the native one
    // reports DECLARED (an axiom — both read the same table) — every such
    // scheme must render identically, since `condense-classify` and the
    // condensation layer's honesty guarantees are defined on that rendering.
    let e = env();
    let out = ev(
        &e,
        "(let* ((names (remove-duplicates $cg-pending)) \
                (dec (filter (lambda (n) (eq (car (see-type n)) 'declared)) names)) \
                (bad (filter (lambda (n) (not (equal (see-type n) (hm-see-type n)))) \
                             dec))) \
           (list (length dec) bad))",
    );
    assert!(
        out.ends_with(" ())"),
        "portable and native disagree on a DECLARED scheme: {out}"
    );
    let count: i64 = out
        .trim_start_matches('(')
        .split(' ')
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);
    assert!(
        count > 20,
        "expected the stdlib's declared axioms to be compared, got {count}"
    );
}

#[test]
fn portable_and_native_agree_on_every_stdlib_scheme_they_both_derive() {
    // The maintenance sweep, over the WHOLE standard library rather than a
    // sample: wherever both checkers actually derived a scheme for the same
    // name — native CHECKED and portable CHECKED — the rendered schemes must
    // be identical, since that rendering is what `condense-classify` and the
    // condensation layer's honesty guarantees are defined on.
    //
    // Names where the two legitimately differ are excluded by construction,
    // not by exception: native TYPED reports a codegen fact the portable
    // checker cannot see (and answers DYNAMIC for), and native DECLARED is
    // compared separately above.
    let e = env();
    let out = ev(
        &e,
        "(let* ((names (remove-duplicates $cg-pending)) \
                (both (filter (lambda (n) \
                                (and (eq (car (see-type n)) 'checked) \
                                     (eq (car (hm-see-type n)) 'checked))) \
                              names)) \
                (bad (filter (lambda (n) \
                               (not (equal (cadr (see-type n)) \
                                           (cadr (hm-see-type n))))) \
                             both))) \
           (list (length names) (length both) bad))",
    );
    assert!(
        out.ends_with(" ())"),
        "portable and native derived different schemes for: {out}"
    );
    // Guard the guard: if the overlap ever collapsed to nothing this test
    // would pass vacuously.
    let counts: Vec<i64> = out
        .trim_start_matches('(')
        .split(' ')
        .take(2)
        .filter_map(|t| t.parse().ok())
        .collect();
    assert_eq!(counts.len(), 2, "unexpected shape: {out}");
    assert!(counts[0] > 400, "expected the whole stdlib, got {out}");
    assert!(
        counts[1] > 40,
        "expected a real overlap to compare, got {out}"
    );
}

// ---------------------------------------------------------------------------
// Interop with the condensation layer.
// ---------------------------------------------------------------------------

#[test]
fn portable_verdicts_classify_through_condense_classify() {
    let e = env();
    ev(&e, "(defun classify-demo (x) (concat x \"!\"))");
    assert_eq!(
        ev(&e, "(condense-classify (hm-verdict 'classify-demo))"),
        "CHECKED"
    );
    assert_eq!(
        ev(&e, "(condense-classify (hm-see-type 'string-upcase))"),
        "DECLARED"
    );
    // A rendered CHECKED scheme is the exact sexpr CONDENSE-VACUOUS-P
    // classifies.
    assert_eq!(
        ev(
            &e,
            "(condense-vacuous-p (cadr (hm-check-lambda '(x) '(x))))"
        ),
        "()"
    );
    assert_eq!(
        ev(&e, "(condense-vacuous-p '(forall (a b) (-> (a) b)))"),
        "T"
    );
}

#[test]
fn the_condensation_layer_runs_off_the_portable_checker() {
    // CONDENSE-VERDICT is the single seam, and the PORTABLE checker is what
    // answers through it — so `condense-classify`, the dynamic frontier and
    // EDIT!'s type barrier are all driven by this port, on every host. That
    // is what #451 asked for.
    let e = env();
    ev(&e, "(defun cverdict-demo (x) (concat x \"!\"))");
    assert_eq!(
        ev(&e, "(condense-verdict 'cverdict-demo)"),
        ev(&e, "(hm-see-type 'cverdict-demo)")
    );
    // A declared axiom comes back DECLARED through the same seam.
    assert_eq!(
        ev(&e, "(condense-classify (condense-verdict 'string-upcase))"),
        "DECLARED"
    );
    // The one thing the native checker is still consulted for is TYPED — a
    // codegen fact (a natively compiled function's signature and execution
    // tier) that no portable checker can observe, so the seam defers to the
    // host for exactly that verdict and nothing else.
    //
    // A natively compiled function's live binding is an opaque membrane, so
    // the portable checker cannot see a body and says DYNAMIC — honestly, and
    // exactly as the host's own `checker_lambda_source` does. The seam is what
    // supplies the TYPED answer for those names.
    ev(&e, "(defun ctyped-demo (n) (+ n 1))");
    assert_eq!(ev(&e, "(car (see-type 'ctyped-demo))"), "TYPED");
    assert_eq!(ev(&e, "(car (hm-see-type 'ctyped-demo))"), "DYNAMIC");
    assert_eq!(ev(&e, "(car (condense-verdict 'ctyped-demo))"), "TYPED");
}

// ---------------------------------------------------------------------------
// Honesty of the verdict layer (#451 review findings F1–F4).
// ---------------------------------------------------------------------------

#[test]
fn a_callers_verdict_tracks_its_callees_current_body() {
    // F1. A verdict is derived from the whole world, including the CALLEE
    // bodies the checker reads on demand. The cache this layer used to keep
    // was invalidated only for the redefined name, so every CALLER went on
    // reporting a confident scheme computed from the callee's OLD body. There
    // is no cache now, and this is the regression guard.
    let e = env();
    ev(&e, "(defun cal (x) (concat x \"!\"))");
    ev(&e, "(defun cer (y) (cal y))");
    assert_eq!(
        ev(&e, "(hm-verdict 'cer)"),
        "(CHECKED (-> (STRING) STRING))"
    );
    ev(&e, "(defun cal (x) (car x))");
    assert_eq!(
        ev(&e, "(hm-verdict 'cer)"),
        "(CHECKED (FORALL (A) (-> ((LIST A)) A)))"
    );
    // HM-VERDICT and HM-SEE-TYPE cannot drift, because the former is the
    // latter.
    assert_eq!(ev(&e, "(hm-verdict 'cer)"), ev(&e, "(hm-see-type 'cer)"));
}

#[test]
fn a_rebinding_that_bypasses_defun_cannot_fabricate_a_verdict() {
    // F2. `jit-optimize` records a `source-form` property for every function
    // it natively compiles, and a later `def`/`set`/`setq` rebinding never
    // clears it — so asking SEE-SOURCE about the SYMBOL returns the old body
    // and would report a confident CHECKED scheme for code the name no longer
    // runs. The checker asks about the live VALUE instead.
    let e = env();
    ev(&e, "(defun sf (n) (+ n 1))");
    assert_eq!(ev(&e, "(car (see-type 'sf))"), "TYPED");
    ev(&e, "(set 'sf (lambda (s) (concat s \"!\")))");
    // What the name actually runs now:
    assert_eq!(ev(&e, "(funcall sf \"a\")"), "\"a!\"");
    // ... and what the checker says about it.
    assert_eq!(
        ev(&e, "(hm-see-type 'sf)"),
        "(CHECKED (-> (STRING) STRING))"
    );
    assert_eq!(
        ev(&e, "(condense-verdict 'sf)"),
        "(CHECKED (-> (STRING) STRING))"
    );
}

#[test]
fn a_wrong_arity_self_call_is_a_type_error() {
    // F3. The function under check is reached through the native checker's
    // provisional registry entry, which rejects a wrong-arity call outright
    // rather than conceding the gradual frontier.
    let e = env();
    assert_eq!(
        ev(&e, "(car (hm-check-named 'sa '(x) '((if x (sa x 1) 0))))"),
        "TYPE-ERROR"
    );
    assert_eq!(
        ev(&e, "(car (hm-check-named 'sb '(x) '((if x (sb x) 0))))"),
        "CHECKED"
    );
}

#[test]
fn let_typed_annotations_use_the_native_annotation_grammar() {
    // F4. LET-TYPED annotations are `src/jit/parse.rs`'s `parse_ty` grammar —
    // scalars (with `u8`/`byte` naming the byte scalar), struct names, bare
    // `array`, `(array T)` — and NOT the larger DECLARE-TYPE! grammar.
    let e = env();
    assert_eq!(
        ev(&e, "(hm-check-expr '(let ((a int64 1)) a))"),
        "(CHECKED INT64)"
    );
    assert_eq!(
        ev(&e, "(hm-check-expr '(let ((a (array int64) (array 3))) a))"),
        "(CHECKED (ARRAY INT64))"
    );
    assert_eq!(ev(&e, "(hm-parse-annotation (hm-new-state) 'u8)"), "CHAR");
    assert_eq!(ev(&e, "(hm-parse-annotation (hm-new-state) 'byte)"), "CHAR");
    // Accepted by DECLARE-TYPE!, rejected as an annotation — both directions
    // of the disagreement this fixes.
    assert_eq!(
        ev(
            &e,
            "(car (hm-check-expr '(let ((a (list int64) (list 1))) a)))"
        ),
        "TYPE-ERROR"
    );
    assert_eq!(
        ev(&e, "(car (hm-check-expr '(let ((a string \"s\")) a)))"),
        "TYPE-ERROR"
    );
}

#[test]
fn the_defun_hook_records_a_dated_note_never_a_cache() {
    // Under EAGER the hook checks the definition on the spot and records the
    // verdict. That record is explicitly a dated note, not an answer: nothing
    // reads it back as current, which is what keeps F1 fixed.
    let e = env();
    ev(&e, "(defun noted (x) (concat x \"!\"))");
    ev(&e, "(hm-check-policy! 'eager)");
    ev(&e, "($hm-on-defun 'noted)");
    assert_eq!(
        ev(&e, "(hm-definition-verdict 'noted)"),
        "(CHECKED (-> (STRING) STRING))"
    );
    // Redefining by a path the hook never sees leaves the note stale — and
    // the live answer correct.
    ev(&e, "(set 'noted (lambda (x) (car x)))");
    assert_eq!(
        ev(&e, "(hm-definition-verdict 'noted)"),
        "(CHECKED (-> (STRING) STRING))"
    );
    assert_eq!(
        ev(&e, "(hm-see-type 'noted)"),
        "(CHECKED (FORALL (A) (-> ((LIST A)) A)))"
    );
    ev(&e, "(hm-check-policy! 'lazy)");
}

#[test]
fn the_no_compile_declaration_still_reaches_the_checker_hook() {
    // `(declare (no-compile))` pins a definition away from the COMPILER, not
    // from the checker: it takes a different branch of the `defun` expansion,
    // and that branch calls the hook too.
    let e = env();
    ev(&e, "(hm-check-policy! 'eager)");
    ev(
        &e,
        "(defun pinned (x) (declare (no-compile)) (concat x \"!\"))",
    );
    assert_eq!(
        ev(&e, "(hm-definition-verdict 'pinned)"),
        "(CHECKED (-> (STRING) STRING))"
    );
    ev(&e, "(hm-check-policy! 'lazy)");
}
