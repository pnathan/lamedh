;;; 28-types.lisp -- THE TYPE TABLE: declared schemes for builtins and
;;; stdlib functions (0.3 census, "typing with vigor").
;;;
;;; Every entry below is an AXIOM the checker trusts at call sites, so each
;;; was verified against actual evaluator behavior before being declared
;;; (docs/audit-0-3.md records the probes). Two honesty rules exclude
;;; entries rather than weaken them:
;;;
;;;   1. NIL-ON-MISS functions are never given a result type: `nth` out of
;;;      range, `assoc` on a missing key, `string->number` on garbage, and
;;;      `string-index-of` on absence all return () -- declaring the "hit"
;;;      type would let checked code consume a () the evaluator legally
;;;      produced. (Contrast `member`, whose miss value () IS a list -- it
;;;      gets a full scheme. Errors are fine: a partial function that
;;;      ERRORS on bad input, like `unwrap`, types its success.)
;;;   2. VARIADIC / multi-arity functions can't carry a fixed-arity scheme
;;;      (`reduce`, `substring`, `mapcar`); the hot ones have checker-native
;;;      rules in the kernel instead (append, concat, min/max, bit ops).
;;;
;;; Numeric-polymorphic arguments (sqrt takes int or float) are declared
;;; with ANY argument and a KNOWN result -- half the vigor is result types.

;;; ---- predicates: (forall (a) (-> (a) bool)) -------------------------------

(mapc (lambda (p) (declare-type! p '(forall (a) (-> (a) bool))))
      '(consp numberp stringp symbolp floatp charp atom listp arrayp
        hash-table-p functionp boundp proper-list-p))

;; Integer-only predicates: the evaluator rejects non-integers, so the
;; checker may too (parity in the strict direction).
(mapc (lambda (p) (declare-type! p '(-> (int64) bool)))
      '(zerop evenp oddp minusp plusp))

;;; ---- list functions --------------------------------------------------------

(declare-type! 'member '(forall (a) (-> (a (list a)) (list a))))
;; filter's list scheme now lives on its protocol instance (lib/29).
(declare-type! 'mapc '(forall (a b) (-> ((-> (a) b) (list a)) (list a))))
(declare-type! 'every '(forall (a) (-> ((-> (a) bool) (list a)) bool)))
(declare-type! 'exists '(forall (a) (-> ((-> (a) bool) (list a)) bool)))
(declare-type! 'notany '(forall (a) (-> ((-> (a) bool) (list a)) bool)))
(declare-type! 'list->array '(forall (a) (-> ((list a)) (array a))))
(declare-type! 'array->list '(forall (a) (-> ((array a)) (list a))))
(declare-type! 'frequencies '(forall (a) (-> ((list a)) (list (pair a int64)))))
;; enumerate / sort-by / string-pad-* take &optional args -- honesty rule 2.

;;; ---- strings and symbols ---------------------------------------------------

(declare-type! 'string-length* '(-> (string) int64))
(declare-type! 'string-repeat '(-> (string int64) string))
(declare-type! 'string-upcase '(-> (string) string))
(declare-type! 'string-downcase '(-> (string) string))
(declare-type! 'string-split '(-> (string string) (list string)))
(declare-type! 'string-join '(-> ((list string) string) string))
(declare-type! 'princ-to-string '(forall (a) (-> (a) string)))
(declare-type! 'prin1-to-string '(forall (a) (-> (a) string)))
(declare-type! 'number->string '(-> (any) string))
(declare-type! 'intern '(-> (string) symbol))
(declare-type! 'implode '(-> ((list any)) symbol))
(declare-type! 'explode '(-> (symbol) (list symbol)))
(declare-type! 'error-message '(-> (any) string))

;;; ---- math: known results, gradual arguments where ints coerce -------------

(mapc (lambda (f) (declare-type! f '(-> (any) float64)))
      '(sqrt sin cos tan exp log))
(mapc (lambda (f) (declare-type! f '(-> (any) int64)))
      '(floor ceiling round truncate))
(declare-type! 'isqrt '(-> (int64) int64))
