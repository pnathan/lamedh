;; Lisp-level regression coverage for the portable Hindley-Milner checker
;; (lib/46-hm-check.lisp, issue #451), alongside tests/test_hm_check.rs's
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

;;; ---- the DEFUN hook, and the absence of a verdict cache ------------------
;;;
;;; These demo functions are defined at TOP LEVEL on purpose. A DEFUN inside a
;;; DEFTEST body binds inside the test closure, where SEE-SOURCE cannot reach
;;; it, so the checker would honestly (and uninterestingly) report DYNAMIC.
;;; Bodies are chosen so one-door DEFUN leaves them INTERPRETED, which keeps
;;; the verdicts here about the checker rather than about the compiler.

(defun hm-hook-demo (a b) (concat a b))
(defun hm-classify-demo (x) (car x))
(defun hm-condense-demo (x) (concat x "!"))

;; The F1 regression pair: a CALLER whose verdict is derived from a CALLEE's
;; body, so redefining the callee must change the caller's answer.
(defun hm-callee (x) (concat x "!"))
(defun hm-caller (y) (hm-callee y))

(deftest hm-defun-hook-drives-the-portable-checker
  ;; $HM-ON-DEFUN is exactly what `$defun-auto-compile` calls on every single
  ;; definition in the language. Under EAGER it checks the definition on the
  ;; spot and records the verdict; under LAZY it does nothing and verdicts are
  ;; computed on demand.
  (let ((prev (hm-check-policy! 'eager)))
    (progn
      ($hm-on-defun 'hm-hook-demo)
      (assert-equal (hm-definition-verdict 'hm-hook-demo)
                    '(checked (-> (string string) string)))
      (hm-check-policy! prev)
      (assert-equal (hm-check-policy) 'lazy))))

(deftest hm-verdicts-are-never-cached
  ;; #451 review F1: there is deliberately NO verdict cache. A verdict is
  ;; derived from the whole world -- including the CALLEE bodies the checker
  ;; reads on demand -- so a cache keyed on the redefined name alone served
  ;; callers a confident answer computed from their callee's OLD body. The
  ;; cache table must not come back...
  (assert-nil (boundp '$hm-verdicts))
  ;; ... and HM-VERDICT must stay a straight query, not a lookup.
  (assert-equal (hm-verdict 'hm-caller) (hm-see-type 'hm-caller))
  (assert-equal (hm-verdict 'hm-caller) '(checked (-> (string) string))))
;; The behavioural proof -- redefine the CALLEE, watch the CALLER's verdict
;; follow -- needs top-level redefinition mid-test, which a DEFTEST body
;; cannot express (its DEFUNs bind inside the test closure). It lives in
;; tests/test_hm_check.rs's `a_callers_verdict_tracks_its_callees_current_body`.

(deftest hm-source-comes-from-the-live-binding
  ;; #451 review F2: SEE-SOURCE asked about a SYMBOL answers from a
  ;; `source-form` property that several host paths write and then never clear
  ;; on a later rebinding. HM-LAMBDA-SOURCE asks about the live VALUE, so a
  ;; name whose value has no inspectable body yields nothing to check rather
  ;; than a scheme for code the name no longer runs.
  (assert-nil (hm-lambda-source 'car))
  (assert-equal (car (hm-see-type 'car)) 'dynamic)
  (assert-nil (hm-lambda-source 'no-such-name-anywhere))
  (assert-equal (car (hm-see-type 'no-such-name-anywhere)) 'dynamic)
  ;; A real plain lambda does yield its parameters and body.
  (assert-equal (car (hm-lambda-source 'hm-caller)) '(y)))
;; The rebinding scenario itself -- compile a name, then rebind it past DEFUN
;; and confirm the checker follows the live value -- likewise needs top level;
;; see `a_rebinding_that_bypasses_defun_cannot_fabricate_a_verdict`.

(deftest hm-self-call-arity-is-an-error
  ;; #451 review F3: the function under check is reached through the native
  ;; checker's provisional REGISTRY entry, which rejects a wrong-arity call
  ;; rather than conceding the gradual frontier.
  (assert-equal (car (hm-check-named 'hm-sa '(x) '((if x (hm-sa x 1) 0))))
                'type-error)
  (assert-equal (car (hm-check-named 'hm-sb '(x) '((if x (hm-sb x) 0))))
                'checked))

(deftest hm-let-typed-annotations-use-the-native-grammar
  ;; #451 review F4: LET-TYPED annotations are `src/jit/parse.rs`'s much
  ;; smaller `parse_ty` grammar, not DECLARE-TYPE!'s.
  (assert-equal (hm-check-expr '(let ((a int64 1)) a)) '(checked int64))
  (assert-equal (hm-check-expr '(let ((a array (array 3))) a))
                '(checked (forall (a) (array a))))
  (assert-equal (hm-check-expr '(let ((a (array int64) (array 3))) a))
                '(checked (array int64)))
  ;; `u8`/`byte` name the byte scalar, and are accepted where DECLARE-TYPE!
  ;; would not know them...
  (assert-equal (hm-parse-annotation (hm-new-state) 'u8) 'char)
  (assert-equal (hm-parse-annotation (hm-new-state) 'byte) 'char)
  ;; ... while `(list T)`, `string`, `symbol` and `any` -- all fine in a
  ;; DECLARE-TYPE! scheme -- are not annotations the native parser accepts.
  (assert-equal (car (hm-check-expr '(let ((a (list int64) (list 1))) a)))
                'type-error)
  (assert-equal (car (hm-check-expr '(let ((a string "s")) a))) 'type-error))

;;; ---- unification corners -------------------------------------------------

(deftest hm-shared-tail-rows-meet-at-one-row
  ;; Two open rows with DIFFERENT tails share one fresh tail, so `rest` stays
  ;; a single row and each side gains the other's private labels.
  (let ((st (hm-new-state)))
    (let ((r1 (list 'record (list (cons 'a 'int64)) (hm-fresh st)))
          (r2 (list 'record (list (cons 'b 'string)) (hm-fresh st))))
      (progn
        (assert-true (hm-unifies-p st r1 r2))
        (assert-equal (hm-render-scheme (hm-generalize st r1))
                      '(forall (a) (record ((a int64) (b string)) a))))))
  ;; Two rows already sharing ONE tail cannot disagree on fields.
  (let ((st (hm-new-state)))
    (let ((rho (hm-fresh st)))
      (assert-nil (hm-unifies-p st
                                (list 'record (list (cons 'a 'int64)) rho)
                                (list 'record (list (cons 'b 'int64)) rho))))))

(deftest hm-generalize-avoiding-keeps-entangled-vars-free
  ;; A variable reachable from the AVOID set stays a free monotype, so a
  ;; nested callee's scheme cannot sever its link to the enclosing check.
  (let ((st (hm-new-state)))
    (let ((a (hm-fresh st)) (b (hm-fresh st)))
      (progn
        (assert-equal (cadr (hm-generalize st (list '-> (list a) b))) '(0 1))
        (assert-equal (cadr (hm-generalize-avoiding st (list '-> (list a) b)
                                                    (list (cadr a))))
                      '(1))))))

(deftest hm-force-any-overwrites-a-concretized-variable
  ;; The self-recursion honesty rule: ANY absorbs a still-FREE variable, so
  ;; undoing an internal concretization needs the forcing path.
  (let ((st (hm-new-state)))
    (let ((v (hm-fresh st)))
      (progn
        (hm-unify! st v 'int64)
        (assert-equal (hm-walk st v) 'int64)
        (hm-unify! st v 'any)
        (assert-equal (hm-walk st v) 'int64)
        (hm-force-any! st (cadr v))
        (assert-equal (hm-walk st v) 'any)))))

(deftest hm-protocol-misuse-is-an-error
  ;; A RESOLVED dispatch argument with no matching instance is a static error,
  ;; not a silent gradual pass.
  (assert-equal (car (hm-check-expr '(length 1))) 'type-error))

;;; ---- interop with the condensation layer ---------------------------------

(deftest hm-verdicts-classify-through-condense-classify
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
  (assert-equal (condense-verdict 'hm-condense-demo)
                (hm-see-type 'hm-condense-demo))
  (assert-equal (condense-classify (condense-verdict 'string-upcase)) 'declared))
