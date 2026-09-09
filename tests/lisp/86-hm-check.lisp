;; Lisp-level regression coverage for the portable HM checker
;; (lib/45-hm-check.lisp, issue #451), alongside tests/test_hm_check.rs's
;; string-pinned host-visible assertions.

(deftest hm-check-scalars
  (assert-equal (hm-check-expr 1) '(checked int64))
  (assert-equal (hm-check-expr 1.5) '(checked float64))
  (assert-equal (hm-check-expr "s") '(checked string))
  (assert-equal (hm-check-expr t) '(checked bool)))

(deftest hm-check-identity-is-polymorphic
  (assert-equal (hm-check-lambda '(x) '(x)) '(checked (forall (a) (-> (a) a)))))

(deftest hm-check-application
  (assert-equal (hm-check-lambda '(f x) '((f x)))
                '(checked (forall (a b) (-> ((-> (a) b) a) b)))))

(deftest hm-check-if-unifies-branches
  (assert-equal (hm-check-lambda '(x) '((if x 1 2))) '(checked (-> (bool) int64)))
  (assert-equal (car (hm-check-lambda '(x) '((if x 1 "s")))) 'type-error))

(deftest hm-check-let-polymorphism
  (assert-equal
   (hm-check-expr '(let ((id (lambda (y) y))) (mk-record (a (id 1)) (b (id "s")))))
   '(checked (record ((a . int64) (b . string)) nil))))

(deftest hm-check-row-polymorphic-field-access
  (assert-equal
   (hm-check-lambda '(r) '((field-ref r x)))
   '(checked (forall (a b) (-> ((record ((x . a)) b)) a)))))

(deftest hm-check-record-round-trip
  (assert-equal (hm-check-expr '(field-ref (mk-record (x 1) (y 2)) x)) '(checked int64)))

(deftest hm-check-closed-record-rejects-missing-field
  (assert-equal
   (car (hm-check-lambda '(x) '((the (record ((x int64))) (mk-record (y 1))))))
   'type-error))

(deftest hm-check-named-nominal-unification
  (assert-equal
   (hm-check-lambda '(x) '((if t (the (point int64 int64) x) (the (point int64 int64) x))))
   '(checked (-> ((named point int64 int64)) (named point int64 int64))))
  (assert-equal
   (car (hm-check-lambda '(x) '((if t (the (point int64 int64) x) (the (shape int64) x)))))
   'type-error))

(deftest hm-check-occurs-check
  (assert-equal (car (hm-check-lambda '(x) '((x x)))) 'type-error))

(deftest hm-check-unbound-variable
  (assert-equal (hm-check-expr 'nowhere-defined) '(type-error "unbound variable NOWHERE-DEFINED")))

(deftest hm-check-variadic-params-are-dynamic
  (assert-equal (car (hm-check-lambda '(x &rest r) '(x))) 'dynamic)
  (assert-equal (car (hm-check-lambda '(x &optional y) '(x))) 'dynamic)
  (assert-equal (car (hm-check-lambda '(&key k) '(k))) 'dynamic))

;; A rendered CHECKED scheme is a `(forall (vars) ty)` sexpr with plain
;; symbol vars -- the same shape lib/20-condensation.lisp's
;; CONDENSE-VACUOUS-P already classifies (issue #451's row-polymorphism
;; groundwork is meant to interoperate with the existing condensation
;; layer, not just resemble it).
(deftest hm-check-scheme-interoperates-with-condense-vacuous-p
  (assert-nil (condense-vacuous-p (cadr (hm-check-lambda '(x) '(x)))))
  (assert-true (condense-vacuous-p '(forall (a b) (-> (a) b)))))
