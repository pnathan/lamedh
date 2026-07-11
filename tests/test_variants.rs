//! Sum types (#312): DEFVARIANT — a closed set of branded record
//! constructors plus a checker-level union — with exhaustive VARIANT-CASE,
//! #S record patterns in MATCH, and Option/Result defined as ordinary
//! variants in the stdlib.

mod test_helpers;

use lamedh::eval_line;
use test_helpers::env_with_stdlib;

fn env() -> lamedh::Shared<lamedh::environment::Environment> {
    let e = env_with_stdlib();
    eval_line(
        "(defvariant shape (circle (r int64)) (rect (w int64) (h int64)))",
        &e,
    );
    e
}

#[test]
fn constructors_are_bare_named_branded_records() {
    let e = env();
    assert_eq!(eval_line("(circle 3)", &e), "#S(CIRCLE 3)");
    assert_eq!(eval_line("(rect 2 4)", &e), "#S(RECT 2 4)");
    assert_eq!(eval_line("(circle-r (circle 3))", &e), "3");
    assert_eq!(eval_line("(circle-p (circle 3))", &e), "T");
    assert_eq!(eval_line("(circle-p (rect 1 1))", &e), "()");
    // The union predicate covers every constructor and nothing else.
    assert_eq!(eval_line("(shape-p (rect 1 1))", &e), "T");
    assert_eq!(eval_line("(shape-p 5)", &e), "()");
}

#[test]
fn variant_case_dispatches_and_binds_fields() {
    let e = env();
    eval_line(
        "(defun area (s) (variant-case s (circle (r) (* 3 (* r r))) (rect (w h) (* w h))))",
        &e,
    );
    assert_eq!(eval_line("(area (circle 3))", &e), "27");
    assert_eq!(eval_line("(area (rect 2 4))", &e), "8");
}

#[test]
fn variant_case_is_exhaustive() {
    let e = env();
    let out = eval_line(
        "(handler-case (variant-case (circle 3) (circle (r) r)) (error (er) (error-message er)))",
        &e,
    );
    assert!(
        out.contains("not exhaustive") && out.contains("RECT"),
        "got: {out}"
    );
    // An ELSE clause satisfies exhaustiveness.
    assert_eq!(
        eval_line("(variant-case (rect 1 2) (circle (r) r) (else 'other))", &e),
        "OTHER"
    );
}

#[test]
fn match_destructures_with_record_patterns() {
    let e = env();
    assert_eq!(
        eval_line(
            "(match (rect 2 4) (#S(CIRCLE ?r) (list 'circ ?r)) (#S(RECT ?w ?h) (list 'rect ?w ?h)))",
            &e
        ),
        "(RECT 2 4)"
    );
    // Nested: a record pattern inside a record pattern.
    eval_line("(defvariant wrap (boxed (inner any)))", &e);
    assert_eq!(
        eval_line(
            "(match (boxed (circle 9)) (#S(BOXED #S(CIRCLE ?r)) ?r))",
            &e
        ),
        "9"
    );
}

#[test]
fn constructors_absorb_into_the_variant_statically() {
    let e = env();
    eval_line(
        "(defun area (s) (variant-case s (circle (r) (* 3 (* r r))) (rect (w h) (* w h))))",
        &e,
    );
    eval_line("(declare-type! 'area '(-> (shape) int64))", &e);
    assert_eq!(eval_line("(check-type (area (circle 3)))", &e), "\"int64\"");
    let out = eval_line("(check-type (area 5))", &e);
    assert!(out.contains("type error"), "got: {out}");
    // A constructor of a DIFFERENT variant is rejected nominally.
    eval_line("(defvariant coin-flip (heads) (tails))", &e);
    let out = eval_line("(check-type (area (heads)))", &e);
    assert!(
        out.contains("HEADS is not a constructor of variant SHAPE"),
        "got: {out}"
    );
}

#[test]
fn option_and_result_work() {
    let e = env();
    assert_eq!(eval_line("(unwrap-or (some 5) 0)", &e), "5");
    assert_eq!(eval_line("(unwrap-or (none) 0)", &e), "0");
    assert_eq!(eval_line("(option-map #'1+ (some 4))", &e), "#S(SOME 5)");
    assert_eq!(eval_line("(option-of ())", &e), "#S(NONE)");
    assert_eq!(eval_line("(option-of 3)", &e), "#S(SOME 3)");
    assert_eq!(eval_line("(result-or (ok 1) 99)", &e), "1");
    assert_eq!(eval_line("(result-or (err \"bad\") 99)", &e), "99");
    // then/map are FUNCTION FIRST, like every CL-convention HOF.
    assert_eq!(
        eval_line(
            "(unwrap-result (result-then (lambda (v) (ok (* v 10))) (ok 2)))",
            &e
        ),
        "20"
    );
    assert_eq!(
        eval_line("(option-then (lambda (v) (some (* v 2))) (some 21))", &e),
        "#S(SOME 42)"
    );
    // try-call bridges the condition system into Result.
    assert_eq!(eval_line("(err-p (try-call #'car 5))", &e), "T");
    assert_eq!(eval_line("(try-call #'car (list 1 2))", &e), "#S(OK 1)");
}

#[test]
fn nullary_constructors_are_singleton_like_and_round_trip() {
    let e = env();
    assert_eq!(eval_line("(none)", &e), "#S(NONE)");
    assert_eq!(eval_line("(equal (none) (none))", &e), "T");
    assert_eq!(
        eval_line(
            "(equal (none) (read-from-string (prin1-to-string (none))))",
            &e
        ),
        "T"
    );
}

// ---- variant-case in the checker (#350) -----------------------------------
// The checker has a native eliminator rule for `variant-case`: the scrutinee
// unifies with the clause ctors' owning variant, clause vars bind to field
// types, and clause bodies join to one result. Before #350 clauses were
// misread as constructor applications and every variant-consuming function
// carried a false TYPE-ERROR.

#[test]
fn variant_case_consumers_check_clean() {
    let e = env();
    eval_line(
        "(defun area3 (s) (variant-case s (circle (r) (* r r)) (rect (w h) (* w h))))",
        &e,
    );
    assert_eq!(
        eval_line("(see-type 'area3)", &e),
        "(CHECKED (-> (SHAPE) INT64))"
    );
    assert_eq!(eval_line("(area3 (rect 2 4))", &e), "8");
    // The scrutinee type flows to call sites: a non-member ctor is rejected.
    eval_line("(defvariant temp3 (hot3 (h int64)) (cold3 (c int64)))", &e);
    let out = eval_line("(check-type (area3 (hot3 1)))", &e);
    assert!(out.contains("not a constructor of variant SHAPE"), "{out}");
}

#[test]
fn variant_case_checks_parametric_variants() {
    let e = env();
    // Fully generic consumer: the payload stays polymorphic.
    eval_line(
        "(defun opt-len3 (o) (variant-case o (some (v) 1) (none () 0)))",
        &e,
    );
    assert_eq!(
        eval_line("(see-type 'opt-len3)", &e),
        "(CHECKED (FORALL (A) (-> ((OPTION A)) INT64)))"
    );
    // A clause body using the payload pins the parameter.
    eval_line(
        "(defun unwrap-or0 (o) (variant-case o (some (v) v) (none () 0)))",
        &e,
    );
    assert_eq!(
        eval_line("(see-type 'unwrap-or0)", &e),
        "(CHECKED (-> ((OPTION INT64)) INT64))"
    );
    assert_eq!(eval_line("(unwrap-or0 (some 42))", &e), "42");
}

#[test]
fn variant_case_clause_misuse_is_still_an_error() {
    let e = env();
    // A genuine type error inside a clause body still surfaces.
    eval_line(
        "(defun bad-body3 (s) (variant-case s (circle (r) (concat r \"x\")) (rect (w h) \"y\")))",
        &e,
    );
    let out = eval_line("(see-type 'bad-body3)", &e);
    assert!(out.contains("TYPE-ERROR"), "{out}");
    // Binding the wrong number of fields is caught statically.
    eval_line(
        "(defun bad-arity3 (s) (variant-case s (circle (r x) r) (rect (w h) w)))",
        &e,
    );
    let out = eval_line("(see-type 'bad-arity3)", &e);
    assert!(out.contains("binds 2 var(s)"), "{out}");
    // Clause bodies must join: int64 vs string disagrees.
    eval_line(
        "(defun disagree3 (s) (variant-case s (circle (r) r) (rect (w h) \"str\")))",
        &e,
    );
    let out = eval_line("(see-type 'disagree3)", &e);
    assert!(out.contains("clauses disagree"), "{out}");
    // Mixing two variants' ctors in one case clashes at the scrutinee.
    eval_line("(defvariant temp4 (hot4 (h int64)) (cold4 (c int64)))", &e);
    eval_line(
        "(defun mixed3 (x) (variant-case x (circle (r) r) (hot4 (h) h) (else 0)))",
        &e,
    );
    let out = eval_line("(see-type 'mixed3)", &e);
    assert!(out.contains("TYPE-ERROR"), "{out}");
}

#[test]
fn variant_case_else_clause_joins() {
    let e = env();
    eval_line(
        "(defun radius-or (s) (variant-case s (circle (r) r) (else 0)))",
        &e,
    );
    assert_eq!(
        eval_line("(see-type 'radius-or)", &e),
        "(CHECKED (-> (SHAPE) INT64))"
    );
    assert_eq!(eval_line("(radius-or (rect 2 4))", &e), "0");
}
