//! Integration tests for the typed-island FRONT END (`lib/47-typed-island.lisp`)
//! and the codegen-mode gate it is built on (`lib/46-hm-check.lisp`, section
//! 7b / 10).
//!
//! The front end decides, portably, what the kernel may compile: which
//! functions form a stable, closed, monomorphic group over the compileable
//! sub-lattice, under which signatures, with which frozen bodies. The kernel
//! (here: the Rust host's typed JIT, Cranelift or the closure tier) only
//! lowers. The two must agree, so every test below that touches the kernel
//! reads the kernel's own verdict back rather than trusting the island.
//!
//! What these tests hold the front end to:
//!
//! 1. **Gate fidelity.** On a single function the portable codegen verdict is
//!    the kernel's `explain-compile` verdict — same admission, same blocker
//!    wording, same signature — over the entire standard library.
//! 2. **Group power.** An island admits what the kernel's per-definition
//!    `jit-optimize` cannot on its own (mutual recursion, a helper whose
//!    parameter type only a caller pins) and the kernel then ACCEPTS every
//!    member when handed the group with its signatures declared first.
//! 3. **Closure and consistency.** A member that calls a rejected peer is
//!    rejected with it; no signature in the manifest comes from a round a
//!    rejected member took part in.
//! 4. **Optimizer validation.** `island-optimize` never changes a member's
//!    signature or set; a type-changing optimizer is reported, not obeyed.

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line};

fn env() -> Shared<Environment> {
    Environment::with_stdlib()
}

fn ev(e: &Shared<Environment>, src: &str) -> String {
    eval_line(src, e)
}

// ---------------------------------------------------------------------------
// The codegen-mode gate on single functions.
// ---------------------------------------------------------------------------

#[test]
fn the_gate_reports_the_kernels_own_blocker_on_single_functions() {
    let e = env();
    ev(&e, "(defun isl-sq (x) (* x x))");
    ev(&e, "(defun isl-greet (s) (concat s \"!\"))");
    // Ambiguous arithmetic: the kernel resolves the operand kind eagerly and
    // cannot; so does the portable gate, in the kernel's words.
    assert_eq!(
        ev(&e, "(hm-compile-verdict 'isl-sq)"),
        "(BLOCKED \"`*`: cannot infer operand type\")"
    );
    assert_eq!(
        ev(&e, "(cdr (assoc 'blocker (explain-compile 'isl-sq)))"),
        ev(&e, "(cadr (hm-compile-verdict 'isl-sq))")
    );
    // A checking-only head is an unknown call to codegen.
    assert_eq!(
        ev(&e, "(hm-compile-verdict 'isl-greet)"),
        "(BLOCKED \"call to unknown function `CONCAT`\")"
    );
    assert_eq!(
        ev(&e, "(cdr (assoc 'blocker (explain-compile 'isl-greet)))"),
        ev(&e, "(cadr (hm-compile-verdict 'isl-greet))")
    );
}

#[test]
fn codegen_mode_is_stricter_than_checking_exactly_where_the_kernel_is() {
    let e = env();
    // Lisp truthiness is fine for the checker; codegen needs a real bool: a
    // free parameter is PINNED to bool by the condition, a known int64 is
    // rejected.
    assert_eq!(
        ev(&e, "(hm-check-lambda '(x) '((if (+ x 1) 1 2)))"),
        "(CHECKED (-> (INT64) INT64))"
    );
    assert_eq!(
        ev(&e, "(hm-compile-lambda 'f '(x) '((if (+ x 1) 1 2)))"),
        "(BLOCKED \"`if` condition must be bool, got INT64\")"
    );
    assert_eq!(
        ev(&e, "(hm-compile-lambda 'f '(x) '((if x 1 2)))"),
        "(COMPILEABLE (-> (BOOL) INT64))"
    );
    // `and`/`or` are ANY to the checker and BOOL -> BOOL to codegen.
    assert_eq!(
        ev(&e, "(hm-compile-lambda 'f '(a b) '((and a b)))"),
        "(COMPILEABLE (-> (BOOL BOOL) BOOL))"
    );
    // Literals outside the unboxed scalars, and free symbols, are rejected.
    assert_eq!(
        ev(&e, "(hm-compile-lambda 'f '(x) '(\"s\"))"),
        "(BLOCKED \"typed core: unsupported literal s\")"
    );
    assert_eq!(
        ev(&e, "(hm-compile-lambda 'f '(x) '((+ x y)))"),
        "(BLOCKED \"unbound variable: Y\")"
    );
    // A polymorphic function is CHECKED but never compileable: no single
    // representation.
    assert_eq!(
        ev(&e, "(hm-check-lambda '(x) '(x))"),
        "(CHECKED (FORALL (A) (-> (A) A)))"
    );
    assert_eq!(ev(&e, "(car (hm-compile-lambda 'f '(x) '(x)))"), "BLOCKED");
}

#[test]
fn codegen_only_rules_admit_loops_and_intrinsics() {
    let e = env();
    assert_eq!(
        ev(
            &e,
            "(hm-compile-lambda 'f '(n) \
               '((let ((i 0) (acc 0)) \
                   (progn (while (< i n) (setq acc (+ acc i)) (setq i (+ i 1))) acc))))"
        ),
        "(COMPILEABLE (-> (INT64) INT64))"
    );
    assert_eq!(
        ev(
            &e,
            "(hm-compile-lambda 'f '(n) \
               '((let ((acc 0)) (progn (for (i 1 n) (setq acc (+ acc i))) acc))))"
        ),
        "(COMPILEABLE (-> (INT64) INT64))"
    );
    assert_eq!(
        ev(&e, "(hm-compile-lambda 'f '(x) '((sqrt x)))"),
        "(COMPILEABLE (-> (FLOAT64) FLOAT64))"
    );
    assert_eq!(
        ev(&e, "(hm-compile-lambda 'f '(x) '((floor x)))"),
        "(COMPILEABLE (-> (FLOAT64) INT64))"
    );
    assert_eq!(
        ev(&e, "(hm-compile-lambda 'f '(n) '((float (+ n 1))))"),
        "(COMPILEABLE (-> (INT64) FLOAT64))"
    );
    assert_eq!(
        ev(&e, "(hm-compile-lambda 'f '(a b) '((logand a b)))"),
        "(COMPILEABLE (-> (INT64 INT64) INT64))"
    );
    // A constant shift compiles; a runtime shift stays interpreted.
    assert_eq!(
        ev(&e, "(hm-compile-lambda 'f '(n) '((ash n 3)))"),
        "(COMPILEABLE (-> (INT64) INT64))"
    );
    assert_eq!(
        ev(&e, "(hm-compile-lambda 'f '(n k) '((ash n k)))"),
        "(BLOCKED \"`ash` shift must be a compile-time integer constant to compile\")"
    );
    // Binary min/max compile; other arities stay interpreted.
    assert_eq!(
        ev(&e, "(hm-compile-lambda 'f '(a b) '((min (+ a 1) b)))"),
        "(COMPILEABLE (-> (INT64 INT64) INT64))"
    );
    assert_eq!(
        ev(&e, "(car (hm-compile-lambda 'f '(a b c) '((min a b c))))"),
        "BLOCKED"
    );
    // A global setq is not compileable; only local slots are.
    assert_eq!(
        ev(&e, "(hm-compile-lambda 'f '(n) '((setq *g* n)))"),
        "(BLOCKED \"setq: variable *G* is not a local binding (only local setq is compileable)\")"
    );
}

#[test]
fn boxed_handles_are_inert_cargo_in_both_modes() {
    // Issue #476: `boxed` is never inferred, only pinned. Under a pin it
    // moves, compares with `equal`, hashes, and indexes a general array; it
    // never enters arithmetic or ordering.
    let e = env();
    assert_eq!(
        ev(
            &e,
            "(cdr (assoc 'bx (hm-compile-group '((bx (h) (hash-code h))) \
                                               (list (cons 'bx '(-> (boxed) int64))))))"
        ),
        "(COMPILEABLE (-> (BOXED) INT64))"
    );
    assert_eq!(
        ev(
            &e,
            "(cdr (assoc 'bx (hm-compile-group '((bx (h i) (fetch h i))) \
                                               (list (cons 'bx '(-> (boxed int64) boxed))))))"
        ),
        "(COMPILEABLE (-> (BOXED INT64) BOXED))"
    );
    assert_eq!(
        ev(
            &e,
            "(cdr (assoc 'bx (hm-compile-group '((bx (a b) (equal a b))) \
                                               (list (cons 'bx '(-> (boxed boxed) bool))))))"
        ),
        "(COMPILEABLE (-> (BOXED BOXED) BOOL))"
    );
    assert_eq!(
        ev(
            &e,
            "(cdr (assoc 'bx (hm-compile-group '((bx (h) (+ h 1))) \
                                               (list (cons 'bx '(-> (boxed) int64))))))"
        ),
        "(BLOCKED \"boxed values support only movement, equal, hash-code, and general-array access\")"
    );
    // The checker refuses the same operand, in the same words.
    assert_eq!(
        ev(
            &e,
            "(let ((st (hm-new-state))) \
               (handler-case (hm-elab st '((h . boxed)) '(< h 1)) \
                 (error (err) (error-message err))))"
        ),
        "\"boxed values support only movement, equal, hash-code, and general-array access\""
    );
    // `equal`/`hash-code` at a non-boxed operand keep their old call-path
    // meaning: unknown to codegen, and declared or gradual to the checker.
    assert_eq!(
        ev(&e, "(car (hm-compile-lambda 'f '(a b) '((equal a b))))"),
        "BLOCKED"
    );
}

// ---------------------------------------------------------------------------
// Freezing.
// ---------------------------------------------------------------------------

#[test]
fn freezing_expands_global_macros_to_a_fixpoint_and_skips_quoted_data() {
    let e = env();
    assert_eq!(ev(&e, "(island-freeze '(when p 1))"), "(IF P (PROGN 1) ())");
    assert_eq!(
        ev(&e, "(island-freeze '(quote (when p 1)))"),
        "(QUOTE (WHEN P 1))"
    );
    // `incf` inside `when`: two macros, both gone from the residue.
    assert_eq!(
        ev(&e, "(island-freeze '(when (> a 1) (incf a)))"),
        "(IF (> A 1) (PROGN (SETQ A (+ A 1))) ())"
    );
    // The frozen `when` carries a nil literal, so the gate blocks it for the
    // same reason the kernel would once handed the residue: honest, and the
    // reason names the residue rather than the macro.
    ev(&e, "(defun isl-wh (n) (when (> n 0) (+ n 1)))");
    assert_eq!(
        ev(&e, "(island-rejection (typed-island '(isl-wh)) 'isl-wh)"),
        "\"typed core: unsupported literal ()\""
    );
}

// ---------------------------------------------------------------------------
// Islands: group power, closure, consistency.
// ---------------------------------------------------------------------------

#[test]
fn an_island_admits_mutual_recursion_the_kernel_alone_cannot() {
    let e = env();
    ev(
        &e,
        "(defun isl-ev (n) (if (= n 0) (= 1 1) (isl-od (- n 1))))",
    );
    ev(
        &e,
        "(defun isl-od (n) (if (= n 0) (= 1 0) (isl-ev (- n 1))))",
    );
    // The kernel's per-definition path left both interpreted (CHECKED).
    assert_eq!(ev(&e, "(car (see-type 'isl-ev))"), "CHECKED");
    assert_eq!(ev(&e, "(car (see-type 'isl-od))"), "CHECKED");
    ev(&e, "(def isl (typed-island '(isl-ev isl-od)))");
    assert_eq!(ev(&e, "(island-member-names isl)"), "(ISL-EV ISL-OD)");
    assert_eq!(
        ev(&e, "(island-signature isl 'isl-ev)"),
        "(-> (INT64) BOOL)"
    );
    assert_eq!(ev(&e, "(island-rejected isl)"), "()");
}

#[test]
fn a_caller_pins_a_helper_that_blocks_alone() {
    let e = env();
    ev(&e, "(defun isl-addp (a b) (+ a b))");
    ev(&e, "(defun isl-usea (x) (isl-addp x 1.5))");
    assert_eq!(ev(&e, "(car (hm-compile-verdict 'isl-addp))"), "BLOCKED");
    ev(&e, "(def isl (typed-island '(isl-addp isl-usea)))");
    assert_eq!(
        ev(&e, "(island-signature isl 'isl-addp)"),
        "(-> (FLOAT64 FLOAT64) FLOAT64)"
    );
    assert_eq!(
        ev(&e, "(island-signature isl 'isl-usea)"),
        "(-> (FLOAT64) FLOAT64)"
    );
}

#[test]
fn the_island_is_closed_under_calls_and_names_the_reason() {
    let e = env();
    ev(&e, "(defun isl-bad (n) (car (list n)))");
    ev(&e, "(defun isl-mid (n) (if (> n 0) (isl-bad n) 1))");
    ev(&e, "(defun isl-top (n) (+ (isl-mid n) 1))");
    ev(&e, "(defun isl-leaf (n) (* n 2))");
    ev(
        &e,
        "(def isl (typed-island '(isl-top isl-mid isl-bad isl-leaf)))",
    );
    assert_eq!(ev(&e, "(island-member-names isl)"), "(ISL-LEAF)");
    assert_eq!(
        ev(&e, "(island-rejection isl 'isl-bad)"),
        "\"call to unknown function `CAR`\""
    );
    // Discovery admitted MID and TOP against BAD's provisional arrow; the
    // clean round, with BAD gone, drops them for the honest reason.
    assert_eq!(
        ev(&e, "(island-rejection isl 'isl-mid)"),
        "\"call to unknown function `ISL-BAD`\""
    );
    assert_eq!(
        ev(&e, "(island-rejection isl 'isl-top)"),
        "\"call to unknown function `ISL-MID`\""
    );
}

#[test]
fn a_name_without_source_is_rejected_not_guessed() {
    let e = env();
    ev(&e, "(defun isl-var (x &rest r) x)");
    ev(
        &e,
        "(def isl (typed-island '(isl-var length nope-not-bound)))",
    );
    assert_eq!(ev(&e, "(island-members isl)"), "()");
    assert_eq!(
        ev(&e, "(island-rejection isl 'isl-var)"),
        "\"no visible plain-lambda source\""
    );
    assert_eq!(
        ev(&e, "(island-rejection isl 'nope-not-bound)"),
        "\"no visible plain-lambda source\""
    );
}

// ---------------------------------------------------------------------------
// The kernel hand-off.
// ---------------------------------------------------------------------------

#[test]
fn island_forms_declare_every_member_before_defining_any() {
    let e = env();
    ev(
        &e,
        "(defun isl-ev (n) (if (= n 0) (= 1 1) (isl-od (- n 1))))",
    );
    ev(
        &e,
        "(defun isl-od (n) (if (= n 0) (= 1 0) (isl-ev (- n 1))))",
    );
    assert_eq!(
        ev(
            &e,
            "(mapcar #'car (island-forms (typed-island '(isl-ev isl-od))))"
        ),
        "(DECLARE-TYPED DECLARE-TYPED DEFUN-TYPED DEFUN-TYPED)"
    );
    assert_eq!(
        ev(&e, "(car (island-forms (typed-island '(isl-ev isl-od))))"),
        "(DECLARE-TYPED (ISL-EV BOOL) ((N INT64)))"
    );
    assert_eq!(
        ev(&e, "(caddr (island-forms (typed-island '(isl-ev isl-od))))"),
        "(DEFUN-TYPED (ISL-EV BOOL) ((N INT64)) (IF (= N 0) (= 1 1) (ISL-OD (- N 1))))"
    );
}

#[test]
fn the_kernel_agrees_on_every_member_and_results_are_preserved() {
    let e = env();
    ev(
        &e,
        "(defun isl-ev (n) (if (= n 0) (= 1 1) (isl-od (- n 1))))",
    );
    ev(
        &e,
        "(defun isl-od (n) (if (= n 0) (= 1 0) (isl-ev (- n 1))))",
    );
    ev(&e, "(defun isl-addp (a b) (+ a b))");
    ev(&e, "(defun isl-usea (x) (isl-addp x 1.5))");
    ev(
        &e,
        "(defun isl-cnt (n) (let ((i 0) (acc 0)) \
           (progn (while (< i n) (setq acc (+ acc i)) (setq i (+ i 1))) acc)))",
    );
    let before = ev(
        &e,
        "(list (isl-ev 10) (isl-od 10) (isl-usea 2.0) (isl-cnt 10))",
    );
    assert_eq!(before, "(T () 3.5 45)");
    ev(
        &e,
        "(def isl (typed-island '(isl-ev isl-od isl-addp isl-usea isl-cnt)))",
    );
    assert_eq!(ev(&e, "(length (island-members isl))"), "5");
    // STRICT here: this test is about the kernel's read-back, so the names
    // stay bound to the kernel's entries for SEE-TYPE to report.
    ev(&e, "(def rep (island-install! isl 'strict))");
    // Every member: the kernel compiled it and reports the island's signature.
    assert_eq!(
        ev(&e, "(island-agreement rep)"),
        "((ISL-EV ISL-OD ISL-ADDP ISL-USEA ISL-CNT))"
    );
    assert_eq!(ev(&e, "(car (see-type 'isl-ev))"), "TYPED");
    assert_eq!(
        ev(&e, "(cadr (see-type 'isl-addp))"),
        "(-> (FLOAT64 FLOAT64) FLOAT64)"
    );
    // The typed editions compute what the interpreted ones did.
    assert_eq!(
        ev(
            &e,
            "(list (isl-ev 10) (isl-od 10) (isl-usea 2.0) (isl-cnt 10))"
        ),
        before
    );
}

#[test]
fn a_guarded_install_never_turns_an_answer_into_a_membrane_error() {
    let e = env();
    ev(
        &e,
        "(defun isl-cnt (n) (let ((i 0) (acc 0)) \
           (progn (while (< i n) (setq acc (+ acc i)) (setq i (+ i 1))) acc)))",
    );
    // The dynamic definition answers a float argument.
    assert_eq!(ev(&e, "(isl-cnt 3.0)"), "3");
    ev(&e, "(def rep (island-install! (typed-island '(isl-cnt))))");
    assert_eq!(ev(&e, "(island-agreement rep)"), "((ISL-CNT))");
    // Guarded (the default): a fitting argument takes the typed entry, a
    // non-fitting one the original closure; the answer is unchanged.
    assert_eq!(ev(&e, "(isl-cnt 10)"), "45");
    assert_eq!(ev(&e, "(isl-cnt 3.0)"), "3");
    // The member stays visible to a later island through the guard record.
    assert_eq!(
        ev(&e, "(island-signature (typed-island '(isl-cnt)) 'isl-cnt)"),
        "(-> (INT64) INT64)"
    );
    // A rebinding by any path makes the record inert: nothing is guessed.
    ev(&e, "(def isl-cnt (lambda (s) (concat s \"!\")))");
    assert_eq!(
        ev(&e, "(island-rejection (typed-island '(isl-cnt)) 'isl-cnt)"),
        "\"call to unknown function `CONCAT`\""
    );
}

#[test]
fn a_strict_install_binds_the_kernels_entry_as_defun_typed_does() {
    let e = env();
    ev(
        &e,
        "(defun isl-cnt (n) (let ((i 0) (acc 0)) \
           (progn (while (< i n) (setq acc (+ acc i)) (setq i (+ i 1))) acc)))",
    );
    ev(
        &e,
        "(def rep (island-install! (typed-island '(isl-cnt)) 'strict))",
    );
    assert_eq!(ev(&e, "(island-agreement rep)"), "((ISL-CNT))");
    assert_eq!(ev(&e, "(car (see-type 'isl-cnt))"), "TYPED");
    assert_eq!(ev(&e, "(isl-cnt 10)"), "45");
    // Outside the signature the membrane refuses, as for any defun-typed.
    assert_eq!(
        ev(
            &e,
            "(handler-case (isl-cnt 3.0) (error (err) 'membrane-error))"
        ),
        "MEMBRANE-ERROR"
    );
    assert_eq!(
        ev(
            &e,
            "(handler-case (island-install! (typed-island '(isl-cnt)) 'loose) (error (err) (error-message err)))"
        ),
        "\"island-install!: mode must be GUARDED or STRICT\""
    );
}

#[test]
fn an_authors_annotation_is_a_pin_the_island_keeps() {
    // `boxed` is never inferred; a member the author typed with it is a
    // member of the island under that signature, not re-derived and lost.
    let e = env();
    ev(&e, "(defun-typed (isl-bh int64) ((v boxed)) (hash-code v))");
    ev(&e, "(defun* isl-half ((x float64)) (/ x 2.0))");
    assert_eq!(
        ev(&e, "(island-source-pin 'isl-bh)"),
        "(ANNOTATED (BOXED) INT64)"
    );
    assert_eq!(
        ev(&e, "(island-source-pin 'isl-half)"),
        "(ANNOTATED (FLOAT64) ?)"
    );
    ev(&e, "(def isl (typed-island '(isl-bh isl-half)))");
    assert_eq!(
        ev(&e, "(island-signature isl 'isl-bh)"),
        "(-> (BOXED) INT64)"
    );
    assert_eq!(
        ev(&e, "(island-signature isl 'isl-half)"),
        "(-> (FLOAT64) FLOAT64)"
    );
    // An unannotated defun* carries no pin.
    ev(&e, "(defun* isl-dbl (x) (* x 2))");
    assert_eq!(ev(&e, "(island-source-pin 'isl-dbl)"), "()");
}

// ---------------------------------------------------------------------------
// Optimizer validation.
// ---------------------------------------------------------------------------

#[test]
fn island_optimize_keeps_every_signature_and_reports_a_type_changing_optimizer() {
    let e = env();
    ev(
        &e,
        "(defun isl-ev (n) (if (= n 0) (= 1 1) (isl-od (- n 1))))",
    );
    ev(
        &e,
        "(defun isl-od (n) (if (= n 0) (= 1 0) (isl-ev (- n 1))))",
    );
    ev(&e, "(defun isl-k (n) (+ (* n 1) 0))");
    ev(&e, "(def isl (typed-island '(isl-ev isl-od isl-k)))");
    ev(&e, "(def opt (island-optimize isl))");
    assert_eq!(ev(&e, "(island-regressions opt)"), "()");
    assert_eq!(
        ev(&e, "(island-member-names opt)"),
        ev(&e, "(island-member-names isl)")
    );
    assert_eq!(
        ev(
            &e,
            "(every (lambda (n) (equal (island-signature opt n) (island-signature isl n))) \
                    (island-member-names isl))"
        ),
        "T"
    );
    // An optimizer that rewrites every body to a float constant changes the
    // types. Under the island's pinned signatures that is a return-type
    // mismatch: nothing is taken from it, every member is reported, the
    // original bodies stay.
    ev(&e, "(def optimize-form (lambda (f) '(lambda (n) 1.5)))");
    ev(&e, "(def broken (island-optimize isl))");
    assert_eq!(ev(&e, "(length (island-regressions broken))"), "3");
    assert_eq!(
        ev(&e, "(island-members broken)"),
        ev(&e, "(island-members isl)")
    );
    assert_eq!(
        ev(&e, "(cdr (assoc 'isl-k (island-regressions broken)))"),
        "\"optimized body no longer compiles: return type mismatch\""
    );
    // An optimizer that signals is a regression too, never a crash.
    ev(&e, "(def optimize-form (lambda (f) (error \"boom\")))");
    assert_eq!(
        ev(
            &e,
            "(cdr (assoc 'isl-k (island-regressions (island-optimize isl))))"
        ),
        "\"optimizer signalled: boom\""
    );
}

// ---------------------------------------------------------------------------
// The whole standard library.
// ---------------------------------------------------------------------------

#[test]
fn the_portable_gate_and_the_kernel_agree_on_the_whole_stdlib() {
    // Fidelity, measured against every function the standard library
    // defines. For a name the kernel has TYPED, the portable gate must
    // reproduce the kernel's signature: from the source alone when the kernel
    // inferred it (jit-optimize, bare-parameter defun*), and under the
    // kernel's signature as pins when the author annotated it (defun-typed,
    // annotated defun*) — the portable `declare-typed`. For every other
    // name, the kernel's explain-compile admits it iff the portable gate does.
    // A `(declare (no-compile))` pin is a policy, not a type fact, and is
    // skipped.
    let e = env();
    ev(
        &e,
        "(defun isl-native-admits-p (n) \
           (let* ((ec (explain-compile n)) (b (cdr (assoc 'blocker ec)))) \
             (and (stringp b) (starts-with-p b \"none\"))))",
    );
    ev(
        &e,
        "(defun isl-pinned-agrees-p (n src) \
           (let* ((pin (hm-host-arrow n)) \
                  (v (cdr (assoc n (hm-compile-group (list (cons n src)) \
                                                     (list (cons n pin))))))) \
             (and (eq (car v) 'compileable) (equal (cadr v) pin))))",
    );
    ev(
        &e,
        "(defun isl-parity-p (n) \
           (let* ((ec (explain-compile n)) \
                  (tier (cdr (assoc 'tier ec))) \
                  (b (cdr (assoc 'blocker ec))) \
                  (src (island-source n))) \
             (cond \
               ((and (stringp b) (starts-with-p b \"pinned\")) t) \
               ((member tier '(compiled typed-interpreted)) \
                (and src \
                     (isl-pinned-agrees-p n src) \
                     (if (island-annotated-source-p n) \
                         t \
                         (let ((v (hm-compile-lambda n (car src) (cdr src)))) \
                           (and (eq (car v) 'compileable) \
                                (equal (cadr v) (hm-host-arrow n))))))) \
               (t (eq (isl-native-admits-p n) \
                      (if src \
                          (eq (car (hm-compile-lambda n (car src) (cdr src))) 'compileable) \
                          nil))))))",
    );
    let out = ev(
        &e,
        "(let* ((names (remove-duplicates $cg-pending)) \
                (typed (filter (lambda (n) (eq (car (see-type n)) 'typed)) names)) \
                (bad (filter (lambda (n) (not (isl-parity-p n))) names))) \
           (list (length names) (length typed) bad))",
    );
    assert!(
        out.ends_with(" ())"),
        "portable gate and kernel disagree on: {out}"
    );
    let counts: Vec<i64> = out
        .trim_start_matches('(')
        .split(' ')
        .take(2)
        .filter_map(|t| t.parse().ok())
        .collect();
    assert_eq!(counts.len(), 2, "unexpected shape: {out}");
    assert!(counts[0] > 400, "expected the whole stdlib, got {out}");
    assert!(
        counts[1] >= 5,
        "expected natively typed stdlib functions to compare against, got {out}"
    );
}

#[test]
fn the_stdlib_island_installs_with_full_kernel_agreement() {
    // The end-to-end validation at scale: the island of every stdlib
    // function is handed to the kernel and every member comes back AGREE.
    // Everything the kernel had already compiled from a visible source is a
    // member (the island is never smaller than the kernel's own admission).
    let e = env();
    ev(&e, "(def names (remove-duplicates $cg-pending))");
    ev(&e, "(def isl (typed-island names))");
    let already = ev(
        &e,
        "(filter (lambda (n) (and (eq (car (see-type n)) 'typed) (island-source n))) names)",
    );
    assert_eq!(
        ev(
            &e,
            "(filter (lambda (n) (not (island-member isl n))) \
                     (filter (lambda (n) (and (eq (car (see-type n)) 'typed) (island-source n))) \
                             names))"
        ),
        "()",
        "a natively compiled stdlib function is missing from the island; natively typed: {already}"
    );
    let n: i64 = ev(&e, "(length (island-members isl))").parse().unwrap_or(0);
    assert!(n >= 5, "expected a stdlib-scale island, got {n} members");
    // Every rejection carries a reason string.
    assert_eq!(
        ev(
            &e,
            "(every (lambda (r) (stringp (cdr r))) (island-rejected isl))"
        ),
        "T"
    );
    ev(&e, "(def rep (island-install! isl))");
    assert_eq!(
        ev(&e, "(cdr (island-agreement rep))"),
        "()",
        "the kernel disputed a member of the stdlib island"
    );
    assert_eq!(
        ev(&e, "(length (car (island-agreement rep)))"),
        ev(&e, "(length (island-members isl))")
    );
}
