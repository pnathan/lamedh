;; SBCL-port-only regressions for KERNEL.md conformance deviations.
;; Unlike the other fixtures here, this file is NOT a copy of the reference
;; implementation's tests/lisp/; it pins port bugs fixed against the spec.

;; #539: (zerop x) accepts only a fixnum (KERNEL: "(zerop 0.0) is an error").
(deftest kernel-zerop-fixnum-only
  (assert-equal (errorset '(zerop 0)) '(t))
  (assert-equal (errorset '(zerop 5)) '(nil))
  (assert-equal (errorset '(zerop -3)) '(nil))
  (assert-nil (errorset '(zerop 0.0)))
  (assert-nil (errorset '(zerop -0.0)))
  (assert-nil (errorset '(zerop 1.5)))
  (assert-nil (errorset '(zerop 'a))))

;; #539: IF takes exactly three operands; no implicit NIL else.
(deftest kernel-if-exactly-three-operands
  (assert-equal (if nil 1 2) 2)
  (assert-equal (if t 1 2) 1)
  (assert-nil (errorset '(if nil 1)))
  (assert-nil (errorset '(if t 1)))
  (assert-nil (errorset '(if t)))
  (assert-nil (errorset '(if)))
  (assert-nil (errorset '(if nil 1 2 3))))

;; #539: a compiled (DEFUN) body obeys the same IF arity as the evaluator.
;; The compiler used to accept (if test then) as an implicit-NIL else.
(deftest kernel-if-arity-in-compiled-bodies
  (defun kc-if2 (x) (if x 1))
  (defun kc-if4 (x) (if x 1 2 3))
  (defun kc-if3 (x) (if x 1 2))
  (assert-equal (kc-if3 nil) 2)
  (assert-equal (kc-if3 t) 1)
  (assert-nil (errorset '(kc-if2 nil)))
  (assert-nil (errorset '(kc-if2 t)))
  (assert-nil (errorset '(kc-if4 nil))))

;; #539: KERNEL Part V -- if ANY operand of a comparison is a float, EVERY
;; operand is converted to f64 (call-wide), so a fixnum above 2^53 compares
;; equal to the float it rounds to. Two fixnums still compare exactly.
;; (#516 asks the Rust core to make this exact instead; until KERNEL.md is
;; changed, f64 is the spec.)
(deftest kernel-mixed-compare-via-f64
  (assert-equal (= 9007199254740993 9007199254740992.0) t)
  (assert-equal (= 9007199254740992.0 9007199254740993) t)
  (assert-nil (< 9007199254740992.0 9007199254740993))
  (assert-nil (> 9007199254740993 9007199254740992.0))
  (assert-nil (lessp 9007199254740992.0 9007199254740993))
  (assert-nil (greaterp 9007199254740993 9007199254740992.0))
  ;; call-wide, not pairwise: the two fixnums are only equal once the float
  ;; in the call forces them both through f64.
  (assert-equal (= 9007199254740993 9007199254740992 9007199254740992.0) t)
  ;; two fixnums: exact
  (assert-nil (= 9007199254740993 9007199254740992))
  (assert-equal (< 9007199254740992 9007199254740993) t)
  ;; characters join the float path as code points
  (assert-equal (= #\a 97.0) t)
  (assert-equal (= 5 5.0) t)
  (assert-equal (< 1 2.5 3) t)
  (assert-nil (= 1.0 1.5))
  ;; NaN is never equal to itself
  (assert-nil (= (/ 0.0 0.0) (/ 0.0 0.0)))
  ;; non-numbers remain errors
  (assert-nil (errorset '(= 1 2.0 'a))))

;; compiled bodies take the same path
(deftest kernel-mixed-compare-via-f64-compiled
  (defun kc-eq (a b) (= a b))
  (defun kc-lt (a b) (< a b))
  (assert-equal (kc-eq 9007199254740993 9007199254740992.0) t)
  (assert-nil (kc-lt 9007199254740992.0 9007199254740993)))
