;; Arithmetic and numeric-predicate coverage.

(deftest arith-basic
  (assert-equal (+ 1 2) 3)
  (assert-equal (+ 1 2 3 4) 10)
  (assert-equal (- 10 3) 7)
  (assert-equal (- 5) -5)
  (assert-equal (* 4 5) 20)
  (assert-equal (* 2 3 4) 24)
  (assert-equal (/ 20 4) 5))

(deftest arith-spec-names
  (assert-equal (plus 2 3) 5)
  (assert-equal (times 2 3) 6)
  (assert-equal (difference 9 4) 5)
  (assert-equal (quotient 12 3) 4))

(deftest arith-comparisons
  (assert-true  (= 3 3))
  (assert-false (= 3 4))
  (assert-true  (< 2 3))
  (assert-false (< 3 2))
  (assert-true  (> 5 1))
  (assert-true  (zerop 0))
  (assert-false (zerop 1)))

(deftest arith-mod-expt
  (assert-equal (mod 7 3) 1)
  (assert-equal (remainder 7 3) 1)
  (assert-equal (expt 2 5) 32)
  (assert-equal (expt 3 0) 1))

(deftest arith-incr-decr
  (assert-equal (add1 4) 5)
  (assert-equal (sub1 4) 3)
  (assert-equal (1+ 9) 10)
  (assert-equal (1- 9) 8))

(deftest arith-parity-sign
  (assert-true  (evenp 4))
  (assert-false (evenp 5))
  (assert-true  (oddp 5))
  (assert-true  (plusp 1))
  (assert-false (plusp -1))
  (assert-true  (minusp -2))
  (assert-true  (onep 1)))

(deftest arith-math-utils
  (assert-equal (abs -7) 7)
  (assert-equal (abs 7) 7)
  (assert-equal (max 3 1 4 1 5) 5)
  (assert-equal (min 3 1 4 1 5) 1)
  (assert-equal (max 42) 42)
  (assert-equal (max 0.5 1) 1.0)
  (assert-equal (min 3 2 1.5) 1.5)
  (assert-equal (min 1 1.0) 1.0)
  (assert-equal (max 1.0 1) 1.0)
  (assert-equal (max 3 2 1.5) 3.0))

;; MIN/MAX: IEEE 754-2019 maximum/minimum for floats, float contagion for
;; untyped calls (#530). Sign of zero and NaN are checked directly: `equal`
;; does not tell 0.0 from -0.0, and NaN is unequal to itself.
(deftest arith-min-max-ieee
  ;; Float contagion: any float argument makes the result a float.
  (assert-equal (max 1 1.0) 1.0)
  (assert-equal (max 0 -0.0) 0.0)
  (assert-equal (min 0 -0.0) -0.0)
  (assert-equal (min 1 2 -0.0) -0.0)
  ;; All-integer calls return integers.
  (assert-true (fixp (max 1 2 3)))
  (assert-true (fixp (min 3 2 1)))
  (assert-equal (max 2 1 0) 2)
  ;; Three-argument variadic.
  (assert-equal (max 1 3 9) 9)
  (assert-equal (min 5 2 8) 2)
  ;; Signed zeros: -0.0 orders below +0.0, in both argument orders.
  (assert-equal (max -0.0 0.0) 0.0)
  (assert-equal (max 0.0 -0.0) 0.0)
  (assert-true (> (/ 1.0 (max -0.0 0.0)) 0))
  (assert-true (> (/ 1.0 (max 0.0 -0.0)) 0))
  (assert-equal (min 0.0 -0.0) -0.0)
  (assert-equal (min -0.0 0.0) -0.0)
  (assert-true (< (/ 1.0 (min 0.0 -0.0)) 0))
  (assert-true (< (/ 1.0 (min -0.0 0.0)) 0))
  ;; NaN propagates from the first, middle and last position.
  (assert-true (/= (max (/ 0.0 0.0) 1 2) (max (/ 0.0 0.0) 1 2)))
  (assert-true (/= (max 1 (/ 0.0 0.0) 2) (max 1 (/ 0.0 0.0) 2)))
  (assert-true (/= (max 1 2 (/ 0.0 0.0)) (max 1 2 (/ 0.0 0.0))))
  (assert-true (/= (min (/ 0.0 0.0) 1 2) (min (/ 0.0 0.0) 1 2)))
  (assert-true (/= (min 1 (/ 0.0 0.0) 2) (min 1 (/ 0.0 0.0) 2)))
  (assert-true (/= (min 1 2 (/ 0.0 0.0)) (min 1 2 (/ 0.0 0.0)))))
