;; Port-specific regression suite (issue #536): no Common Lisp ratio may
;; ever reach Lamedh code. Unlike the other files here, this one is not a
;; copy of ../tests/lisp/; every assertion also holds on the Rust reference.

(deftest no-ratio-divide-arity
  (assert-equal (/ 7 2) 3)
  (assert-equal (/ -7 2) -3)
  (assert-equal (/ 7.0 2) 3.5)
  (assert-nil (errorset '(/ 5)))
  (assert-nil (errorset '(/ 5.0)))
  (assert-nil (errorset '(/ 1 2 3)))
  (assert-nil (errorset '(/))))

(deftest no-ratio-expt-negative-exponent
  (assert-true (floatp (expt 3 -2)))
  (assert-equal (expt 2 -1) 0.5)
  (assert-equal (expt -2 -1) -0.5)
  (assert-equal (expt 4 -2) 0.0625)
  (assert-equal (expt 2.0 -2) 0.25)
  (assert-equal (expt 1 -5) 1.0)
  (assert-equal (expt 2 10) 1024))

(deftest no-ratio-expt-zero-negative-exponent-is-inf
  (assert-true (floatp (expt 0 -1)))
  (assert-true (> (expt 0 -1) 1.0e308))
  (assert-equal (prin1-to-string (expt 0 -1)) "inf"))
