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
