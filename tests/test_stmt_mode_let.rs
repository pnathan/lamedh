//! #513: statement mode reaches through `let`/`let*`/`progn`. A
//! `cond`/`when`/`unless`/`case` that is the last form of a `let` or `progn`
//! whose own value is discarded (a loop body) desugars in statement mode, so
//! it compiles instead of failing with "`if` branches disagree".
//! Differential: every compiled result equals the interpreter's result for
//! the same body, and the portable codegen gate agrees on admission.

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line};

fn env() -> Shared<Environment> {
    Environment::with_stdlib()
}

/// (name, body over `n`, inputs). Every case is `(-> (INT64) INT64)`.
const CASES: &[(&str, &str, &[i64])] = &[
    (
        "sm-dotimes-let-when",
        "(let ((a 0)) (dotimes (j n) (let ((c j)) (when (<= c 3) (setq a (+ a c))))) a)",
        &[0, 2, 9],
    ),
    (
        "sm-dotimes-progn-unless",
        "(let ((a 0)) (dotimes (j n) (progn (setq a (+ a 1)) \
         (unless (< j 2) (setq a (+ a j))))) a)",
        &[0, 1, 7],
    ),
    (
        "sm-dotimes-let-case",
        "(let ((a 0)) (dotimes (j n) (let ((m (mod j 4))) \
         (case m (0 (setq a (+ a 10))) ((1 2) (setq a (+ a 1)))))) a)",
        &[0, 3, 11],
    ),
    (
        "sm-for-let-cond",
        "(let ((a 0)) (for (i 1 n) (let ((m (mod i 3))) \
         (cond ((= m 0) (setq a (+ a i))) ((= m 1) (setq a (- a 1)))))) a)",
        &[0, 1, 13],
    ),
    (
        "sm-for-progn-case",
        "(let ((a 0)) (for (i 1 n) (progn \
         (case (mod i 4) (0 (setq a (+ a 10))) ((1 2) (setq a (+ a 1)))))) a)",
        &[0, 4, 10],
    ),
    (
        "sm-for-progn-when",
        "(let ((a 0)) (for (i 1 n) (progn (when (> i 2) (setq a (+ a i))))) a)",
        &[0, 2, 6],
    ),
    (
        "sm-while-let-when",
        "(let ((a 0) (i 0)) (while (< i n) (setq i (+ i 1)) \
         (let ((d (* i 2))) (when (> d 5) (setq a (+ a d))))) a)",
        &[0, 2, 8],
    ),
    (
        "sm-while-progn-cond",
        "(let ((a 0) (i 0)) (while (< i n) (setq i (+ i 1)) \
         (progn (cond ((< i 3) (setq a (+ a 1))) (t (setq a (+ a 2)))))) a)",
        &[0, 2, 10],
    ),
    (
        "sm-while-let-unless",
        "(let ((a 0) (i 0)) (while (< i n) (setq i (+ i 1)) \
         (let ((q (mod i 2))) (unless (= q 0) (setq a (+ a i))))) a)",
        &[0, 1, 9],
    ),
    (
        "sm-while-let-case",
        "(let ((a 0) (i 0)) (while (< i n) (setq i (+ i 1)) \
         (let ((m (mod i 3))) (case m (0 (setq a (+ a 5))) (t (setq a (- a 1)))))) a)",
        &[0, 3, 7],
    ),
    (
        "sm-nested-let-progn-let",
        "(let ((a 0)) (dotimes (j n) (let ((c j)) (progn \
         (let ((d (+ c 1))) (unless (> d 4) (setq a (+ a d))))))) a)",
        &[0, 3, 8],
    ),
    (
        "sm-let-star",
        "(let ((a 0)) (dotimes (j n) (let* ((c j) (d (* c c))) \
         (when (> d 4) (setq a (+ a d))))) a)",
        &[0, 3, 6],
    ),
    (
        "sm-nested-loops",
        "(let ((a 0)) (dotimes (i n) (for (j 0 i) (let ((s (+ i j))) \
         (when (= (mod s 2) 0) (setq a (+ a s)))))) a)",
        &[0, 1, 5],
    ),
];

#[test]
fn statement_branches_in_let_and_progn_compile_and_match_the_interpreter() {
    let e = env();
    for (name, body, inputs) in CASES {
        eval_line(&format!("(defun {name} (n) {body})"), &e);
        let ex = eval_line(&format!("(explain-compile '{name})"), &e);
        assert!(ex.starts_with("((TIER . COMPILED)"), "{name}: {ex}");
        for n in *inputs {
            let want = eval_line(&format!("(let ((n {n})) {body})"), &e);
            assert_eq!(eval_line(&format!("({name} {n})"), &e), want, "{name} {n}");
        }
    }
}

#[test]
fn the_portable_gate_agrees() {
    let e = env();
    for (name, body, _) in CASES {
        assert_eq!(
            eval_line(&format!("(hm-compile-lambda '{name} '(n) '({body}))"), &e),
            "(COMPILEABLE (-> (INT64) INT64))",
            "{name}"
        );
    }
}

/// The issue's repro: a statement-position `when` inside a `let` in a
/// `dotimes` body, storing into an array.
const REPRO_BODY: &str = "(dotimes (j n) (let ((c j)) (when (<= c 3) (store a j 0)))) 0";

#[test]
fn the_issue_repro_compiles_under_defun_typed_and_defun_star() {
    let e = env();
    let r = eval_line(
        &format!("(defun-typed (sm-w int64) ((a (array int64)) (n int64)) {REPRO_BODY})"),
        &e,
    );
    assert_eq!(r, "SM-W", "{r}");
    assert_eq!(
        eval_line("(see-type 'sm-w)", &e),
        "(TYPED (-> ((ARRAY INT64) INT64) INT64) COMPILED)"
    );
    eval_line(&format!("(defun* sm-w2 (a n) {REPRO_BODY})"), &e);
    let ex = eval_line("(explain-compile 'sm-w2)", &e);
    assert!(ex.starts_with("((TIER . COMPILED)"), "{ex}");
    // Differential: the compiled stores match the tree-walker's.
    for (f, arr) in [("sm-w", "*sm-a1*"), ("sm-w2", "*sm-a2*")] {
        eval_line(
            &format!("(defvar {arr} (let ((v (make-array 6))) (dotimes (k 6) (store v k 7)) v))"),
            &e,
        );
        assert_eq!(eval_line(&format!("({f} {arr} 6)"), &e), "0");
    }
    eval_line(
        "(defvar *sm-a3* (let ((v (make-array 6))) (dotimes (k 6) (store v k 7)) v))",
        &e,
    );
    eval_line(&format!("(let ((a *sm-a3*) (n 6)) {REPRO_BODY})"), &e);
    let want = eval_line("*sm-a3*", &e);
    assert_eq!(eval_line("*sm-a1*", &e), want);
    assert_eq!(eval_line("*sm-a2*", &e), want);
}

#[test]
fn a_value_position_let_still_yields_nil_on_a_miss() {
    // Statement mode must not reach a `let` whose value is used: a
    // non-bool `when` there can yield NIL, which native code cannot carry.
    let e = env();
    eval_line(
        "(defun sm-val (x) (let ((c x)) (when (> c 0) (+ c 1))))",
        &e,
    );
    assert!(!eval_line("(explain-compile 'sm-val)", &e).contains("COMPILED"));
    assert_eq!(eval_line("(sm-val -1)", &e), "()");
    assert_eq!(eval_line("(sm-val 1)", &e), "2");
}

#[test]
fn let_star_compiles_and_both_checkers_agree() {
    // `let*` binds sequentially, like the elaborator's `let`; value position.
    let e = env();
    let body = "(let* ((c n) (d (+ c 1))) (* c d))";
    assert_eq!(
        eval_line(
            &format!("(defun-typed (sm-ls int64) ((n int64)) {body})"),
            &e
        ),
        "SM-LS"
    );
    assert_eq!(
        eval_line("(see-type 'sm-ls)", &e),
        "(TYPED (-> (INT64) INT64) COMPILED)"
    );
    for n in [0, 3, -4] {
        let want = eval_line(&format!("(let ((n {n})) {body})"), &e);
        assert_eq!(eval_line(&format!("(sm-ls {n})"), &e), want, "{n}");
    }
    assert_eq!(
        eval_line(&format!("(hm-check-lambda '(n) '({body}))"), &e),
        "(CHECKED (-> (INT64) INT64))"
    );
    assert_eq!(
        eval_line(&format!("(hm-compile-lambda 'sm-ls '(n) '({body}))"), &e),
        "(COMPILEABLE (-> (INT64) INT64))"
    );
}
