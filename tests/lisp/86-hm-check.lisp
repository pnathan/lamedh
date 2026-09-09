;; Lisp-level regression coverage for the portable Hindley-Milner checker
;; (lib/45-hm-check.lisp, issue #451), alongside tests/test_hm_check.rs's
;; string-pinned host-visible assertions.
;;
;; The checker is a port of the reference host's own checker, so the standard
;; every test here holds it to is FIDELITY: what it reports is what the native
;; checker reports for the same code, including the native checker's gradual
;; choices (a free symbol is ANY; `and`/`or`/`when` are ANY; a callee it
;; cannot see is ANY). Tests that would only pass for a "better" checker would
;; be testing the wrong thing.

;;; ---- the inference core --------------------------------------------------

(deftest hm-check-scalars
  (assert-equal (hm-check-expr 1) '(checked int64))
  (assert-equal (hm-check-expr 1.5) '(checked float64))
  (assert-equal (hm-check-expr "s") '(checked string)))

(deftest hm-check-identity-is-polymorphic
  (assert-equal (hm-check-lambda '(x) '(x)) '(checked (forall (a) (-> (a) a)))))

(deftest hm-check-arithmetic-constrains-operands
  ;; Both operands of `+` unify, and a string operand is the evaluator's own
  ;; runtime rejection made static.
  (assert-equal (hm-check-lambda '(x) '((+ x 1))) '(checked (-> (int64) int64)))
  (assert-equal (car (hm-check-expr '(+ "a" "b"))) 'type-error))

(deftest hm-check-division-arity-matches-the-evaluator
  ;; `/` and `mod` are strictly binary in the evaluator, so the checker must
  ;; reject every other arity rather than invent a meaning.
  (assert-equal (car (hm-check-expr '(/ 1))) 'type-error)
  (assert-equal (car (hm-check-expr '(/ 1 2 3))) 'type-error)
  (assert-equal (hm-check-expr '(/ 6 2)) '(checked int64)))

(deftest hm-check-comparison-yields-bool
  ;; The operands unify with each other but are not forced numeric unless
  ;; something already made them so -- the native rule rejects only the
  ;; KNOWN non-comparable kinds, so two free parameters stay polymorphic.
  (assert-equal (hm-check-lambda '(a b) '((< a b)))
                '(checked (forall (a) (-> (a a) bool))))
  (assert-equal (car (hm-check-expr '(< "a" "b"))) 'type-error))

(deftest hm-check-if-unifies-branches
  ;; The condition follows Lisp truthiness (any type) exactly as the native
  ;; checker has it -- so X stays polymorphic here.
  (assert-equal (car (hm-check-lambda '(x) '((if x 1 2)))) 'checked)
  (assert-equal (car (hm-check-lambda '(x) '((if x 1 "s")))) 'type-error))

(deftest hm-check-nil-branch-honesty-rule
  ;; A literal-nil branch meeting a NON-list branch degrades the whole IF to
  ;; ANY rather than forcing `(list _)` onto the other branch: the nil-on-miss
  ;; honesty rule (native `join_branch_types`).
  (assert-equal (hm-check-expr '(if 1 2 nil)) '(checked any))
  ;; ... but a nil branch meeting a genuine list branch still unifies.
  (assert-equal (hm-check-expr '(if 1 (list 1 2) nil))
                '(checked (list int64))))

(deftest hm-check-list-rules
  (assert-equal (hm-check-expr '(list 1 2 3)) '(checked (list int64)))
  (assert-equal (car (hm-check-expr '(list 1 "s"))) 'type-error)
  (assert-equal (hm-check-expr '(car (list 1 2))) '(checked int64))
  (assert-equal (hm-check-expr '(cdr (list 1 2))) '(checked (list int64)))
  ;; A cons onto a known non-list ground type is a dotted PAIR.
  (assert-equal (hm-check-expr '(cons 1 "s")) '(checked (pair int64 string))))

(deftest hm-check-occurs-check
  ;; (x x) is a call whose head is the parameter X -- the native checker does
  ;; not type an application through a local, so this is the gradual frontier,
  ;; not an occurs-check failure. The occurs-check itself is exercised
  ;; directly against the unifier below.
  (assert-equal (car (hm-check-lambda '(x) '((x x)))) 'checked)
  (let ((st (hm-new-state)))
    (let ((a (hm-fresh st)))
      (assert-nil (hm-unifies-p st a (list 'list a))))))

(deftest hm-check-variadic-params-are-dynamic
  (assert-equal (car (hm-check-lambda '(x &rest r) '(x))) 'dynamic)
  (assert-equal (car (hm-check-lambda '(x &optional y) '(x))) 'dynamic)
  (assert-equal (car (hm-check-lambda '(&key k) '(k))) 'dynamic))

;;; ---- row polymorphism ----------------------------------------------------

(deftest hm-check-row-polymorphic-field-access
  ;; Reading a field through RECORD-REF DERIVES an open row requirement with
  ;; no declare-type! axioms: the property lib/20-condensation.lisp's
  ;; row-typed record accessors depend on.
  (assert-equal
   (hm-check-lambda '(r) '((record-ref r 'x)))
   '(checked (forall (a b) (-> ((record ((x a)) b)) a)))))

(deftest hm-check-row-accepts-any-record-naming-the-field
  ;; Two DIFFERENT record brands both flow through one row-typed reader.
  (defrecord HmRowA (x int64) (y int64))
  (defrecord HmRowB (x int64) (z string))
  (assert-equal (car (hm-check-expr '(record-ref (make-HmRowA 1 2) 'x))) 'checked)
  (assert-equal (car (hm-check-expr '(record-ref (make-HmRowB 1 "s") 'x))) 'checked)
  ;; ... and a field neither brand has is a static error.
  (assert-equal (car (hm-check-expr '(record-ref (make-HmRowA 1 2) 'nope)))
                'type-error))

;;; ---- the declaration plane: defrecord ------------------------------------

(deftest hm-defrecord-populates-the-portable-registry
  ;; COMPILED tier (all-native fields): registered as a nominal record with
  ;; its field types, from DEFRECORD's own expansion -- no separate
  ;; declaration step, even though this tier expands to the host-only
  ;; DEFSTRUCT-TYPED special form.
  (defrecord HmPoint (x int64) (y int64))
  (assert-true (hm-struct-p 'HmPoint))
  (assert-equal (hm-struct-def 'HmPoint) '((x . int64) (y . int64)))
  ;; DYNAMIC tier (a non-native field): the same registration, plus the
  ;; per-accessor axiom this tier generates, rendered exactly as the native
  ;; checker renders it.
  (defrecord HmLabel (n int64) (s string))
  (assert-equal (hm-struct-def 'HmLabel) '((n . int64) (s . string)))
  (assert-equal (hm-see-type 'HmLabel-s) '(declared (-> (HmLabel) string))))

(deftest hm-defrecord-brands-are-nominal
  (defrecord HmChest (w int64))
  (defrecord HmCrate (w int64))
  ;; Same shape, different brand: a CHEST is not a CRATE.
  (let ((st (hm-new-state)))
    (assert-nil (hm-unifies-p st '(struct HmChest) '(struct HmCrate))))
  ;; ... but both subsume into a row naming only W.
  (let ((st (hm-new-state)))
    (assert-true (hm-unifies-p st '(struct HmChest)
                               (list 'record (list (cons 'w 'int64))
                                     (hm-fresh st)))))
  (let ((st (hm-new-state)))
    (assert-true (hm-unifies-p st '(struct HmCrate)
                               (list 'record (list (cons 'w 'int64))
                                     (hm-fresh st))))))

(deftest hm-defrecord-closed-row-needs-every-field
  (defrecord HmPair2 (a int64) (b int64))
  ;; A CLOSED record naming only one of the two fields does not match.
  (let ((st (hm-new-state)))
    (assert-nil (hm-unifies-p st '(struct HmPair2)
                              (list 'record (list (cons 'a 'int64)) nil))))
  ;; Naming both does.
  (let ((st (hm-new-state)))
    (assert-true (hm-unifies-p st '(struct HmPair2)
                               (list 'record (list (cons 'a 'int64)
                                                   (cons 'b 'int64))
                                     nil)))))

;;; ---- the declaration plane: defvariant -----------------------------------

(deftest hm-defvariant-populates-the-portable-registry
  (defvariant HmShape
    (HmCircle (r int64))
    (HmRect (w int64) (h int64)))
  (assert-true (hm-variant-p 'HmShape))
  (assert-equal (hm-variant-ctors 'HmShape) '(HmCircle HmRect))
  ;; Each constructor is itself a registered nominal record.
  (assert-equal (hm-struct-def 'HmCircle) '((r . int64)))
  ;; A constructor brand absorbs into its variant (one-way), and two variants
  ;; only unify by name.
  (let ((st (hm-new-state)))
    (assert-true (hm-unifies-p st '(struct HmCircle) '(variant HmShape))))
  (let ((st (hm-new-state)))
    (assert-true (hm-unifies-p st '(variant HmShape) '(struct HmRect))))
  (let ((st (hm-new-state)))
    (assert-nil (hm-unifies-p st '(struct HmCircle) '(variant HmNoSuch)))))

(deftest hm-variant-case-binds-fields-and-joins-clauses
  (defvariant HmBox
    (HmFull (v int64))
    (HmEmpty))
  (assert-equal
   (hm-check-lambda '(b) '((variant-case b
                             (HmFull (v) v)
                             (HmEmpty () 0))))
   '(checked (-> (HmBox) int64)))
  ;; Clause bodies that disagree are a static error.
  (assert-equal
   (car (hm-check-lambda '(b) '((variant-case b
                                  (HmFull (v) v)
                                  (HmEmpty () "s")))))
   'type-error)
  ;; Binding the wrong number of variables is a static error.
  (assert-equal
   (car (hm-check-lambda '(b) '((variant-case b (HmFull (v w) v)))))
   'type-error))

(deftest hm-parametric-variant-registers-generically
  ;; OPTION/RESULT are ordinary parametric variants declared in
  ;; lib/25-variants.lisp: the portable registry learned them from the same
  ;; DEFVARIANT expansion the native one did.
  (assert-true (hm-generic-p 'option))
  (assert-equal (hm-generic-arity (hm-generic-def 'option)) 1)
  (assert-true (member 'some (hm-generic-ctors (hm-generic-def 'option))))
  ;; SOME's application absorbs into OPTION's application, arguments pairwise.
  (let ((st (hm-new-state)))
    (assert-true (hm-unifies-p st '(app some (int64)) '(app option (int64)))))
  (let ((st (hm-new-state)))
    (assert-nil (hm-unifies-p st '(app some (int64)) '(app option (string))))))

;;; ---- the declaration plane: protocols ------------------------------------

(deftest hm-protocol-instances-reach-the-portable-registry
  ;; LENGTH is the stdlib's pilot protocol (lib/29-protocols.lisp).
  (assert-true (hm-protocol-p 'length))
  (assert-true (< 0 (length (hm-protocol-instances 'length))))
  ;; Every instance returns INT64, so even an unresolved dispatch argument
  ;; still yields the shared ground result.
  (assert-equal (hm-check-lambda '(x) '((length x)))
                '(checked (forall (a) (-> (a) int64))))
  ;; A resolved dispatch argument selects the matching instance.
  (assert-equal (hm-check-expr '(length (list 1 2))) '(checked int64)))

(deftest hm-protocol-dispatch-position-is-registered
  ;; Fn-first protocols dispatch on argument 1, not 0.
  (assert-equal (hm-protocol-dispatch-index 'length) 0))

;;; ---- declared axioms -----------------------------------------------------

(deftest hm-declared-axioms-are-registered-and-reported
  (declare-type! 'hm-axiom-demo '(forall (a) (-> ((list a)) a)))
  (assert-equal (hm-see-type 'hm-axiom-demo)
                '(declared (forall (a) (-> ((list a)) a))))
  ;; A call site consumes the axiom.
  (assert-equal (hm-check-expr '(hm-axiom-demo (list 1 2))) '(checked int64))
  (assert-equal (car (hm-check-expr '(hm-axiom-demo 1))) 'type-error))

(deftest hm-stdlib-declarations-are-all-representable
  ;; Nothing the standard library declares was dropped for want of a portable
  ;; spelling: the portable registry really did learn every declaration.
  (assert-equal (hm-dropped-declarations) nil))

;;; ---- the DEFUN hook ------------------------------------------------------

(deftest hm-defun-hook-drives-the-portable-checker
  (let ((prev (hm-check-policy! 'eager)))
    (progn
      ;; Defining a function under the EAGER policy runs the portable checker
      ;; through `$defun-auto-compile` -- the one door every DEFUN goes
      ;; through -- and caches the verdict.
      (defun hm-hook-demo (a b) (+ a b))
      (assert-equal (car (hm-verdict 'hm-hook-demo)) 'checked)
      ;; Redefining invalidates the cache, so the verdict tracks the body.
      (defun hm-hook-demo (a b) (concat a b))
      (assert-equal (hm-verdict 'hm-hook-demo) '(checked (-> (string string) string)))
      (hm-check-policy! prev))))

(deftest hm-verdict-is-cached-and-invalidated
  ;; Bodies chosen so one-door DEFUN leaves them INTERPRETED: a natively
  ;; compiled function is a membrane, not a plain lambda, which the portable
  ;; checker honestly reports as DYNAMIC.
  (defun hm-cache-demo (x) (concat x "!"))
  (assert-equal (hm-verdict 'hm-cache-demo) '(checked (-> (string) string)))
  (defun hm-cache-demo (x) (car x))
  (assert-equal (hm-verdict 'hm-cache-demo) '(checked (forall (a) (-> ((list a)) a)))))

;;; ---- interop with the condensation layer ---------------------------------

(deftest hm-verdicts-classify-through-condense-classify
  (defun hm-classify-demo (x) (concat x "!"))
  (assert-equal (condense-classify (hm-verdict 'hm-classify-demo)) 'checked)
  (assert-equal (condense-classify (hm-see-type 'hm-axiom-demo)) 'declared)
  (assert-equal (condense-classify '(type-error "boom")) 'type-error)
  (assert-equal (condense-classify '(dynamic "unseen")) 'dynamic))

(deftest hm-scheme-interoperates-with-condense-vacuous-p
  ;; A rendered CHECKED scheme is exactly the `(forall (vars) ty)` sexpr
  ;; CONDENSE-VACUOUS-P already classifies.
  (assert-nil (condense-vacuous-p (cadr (hm-check-lambda '(x) '(x)))))
  (assert-true (condense-vacuous-p '(forall (a b) (-> (a) b)))))

(deftest hm-condense-verdict-runs-the-portable-checker
  ;; The condensation layer is defined against CONDENSE-VERDICT, and the
  ;; PORTABLE checker is what answers through it -- so CONDENSE-CLASSIFY, the
  ;; dynamic frontier and EDIT!'s type barrier are all driven by this port.
  (defun hm-condense-demo (x) (concat x "!"))
  (assert-equal (condense-verdict 'hm-condense-demo)
                (hm-see-type 'hm-condense-demo))
  (assert-equal (condense-classify (condense-verdict 'string-upcase)) 'declared))
