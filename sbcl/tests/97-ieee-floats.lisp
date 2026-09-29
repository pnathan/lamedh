;; IEEE-754 float edge cases (#534). Port-specific: KERNEL §V and the
;; reference implementation give inf / -inf / NaN results where Common Lisp
;; would signal an arithmetic condition or return a complex number. Every
;; expected value below is the reference implementation's own output.

(deftest ieee-division-by-zero
  (assert-equal (number->string (/ 1.0 0)) "inf")
  (assert-equal (number->string (/ -1.0 0)) "-inf")
  (assert-equal (number->string (/ 1 0.0)) "inf")
  (assert-equal (number->string (/ 0.0 0)) "NaN")
  (assert-equal (number->string (quotient 1.0 0)) "inf"))

(deftest ieee-log
  (assert-equal (number->string (log 0)) "-inf")
  (assert-equal (number->string (log 0.0)) "-inf")
  (assert-equal (number->string (log -1)) "NaN")
  (assert-equal (number->string (log 0 10)) "-inf")
  (assert-equal (number->string (log -8 2)) "NaN")
  (assert-equal (number->string (log 8 1)) "inf")
  (assert-equal (log 8 2) 3.0))

(deftest ieee-sqrt-expt-exp
  (assert-equal (number->string (sqrt -1)) "NaN")
  (assert-equal (number->string (sqrt -1.0)) "NaN")
  (assert-true (floatp (sqrt -1)))
  (assert-equal (sqrt 4) 2.0)
  (assert-equal (number->string (expt -8 0.5)) "NaN")
  (assert-equal (number->string (expt 10.0 400)) "inf")
  (assert-equal (number->string (expt 0.0 -1)) "inf")
  (assert-equal (number->string (exp 1000)) "inf")
  (assert-equal (exp (/ -1.0 0)) 0.0))

(deftest ieee-float-to-int-saturates
  (assert-equal (truncate (/ 1.0 0)) 9223372036854775807)
  (assert-equal (truncate (/ -1.0 0)) -9223372036854775808)
  (assert-equal (truncate (/ 0.0 0)) 0)
  (assert-equal (truncate (* 1000000000000000000.0 1000000000000.0)) 9223372036854775807)
  (assert-equal (floor (/ -1.0 0)) -9223372036854775808)
  (assert-equal (ceiling (/ 1.0 0)) 9223372036854775807)
  (assert-equal (round (/ 1.0 0)) 9223372036854775807)
  (assert-equal (round (/ -1.0 0)) -9223372036854775808)
  (assert-equal (floor (/ 0.0 0)) 0)
  (assert-equal (truncate 2.7) 2)
  (assert-equal (round -2.5) -3))

(deftest ieee-propagation-and-comparison
  (assert-equal (prin1-to-string (list (/ 1.0 0) (log 0) (sqrt -1))) "(inf -inf NaN)")
  (assert-equal (number->string (- (/ 1.0 0) (/ 1.0 0))) "NaN")
  (assert-equal (number->string (* 0.0 (/ 1.0 0))) "NaN")
  (assert-equal (number->string (+ (/ 1.0 0) 1)) "inf")
  (assert-equal (number->string (- (/ 1.0 0))) "-inf")
  (assert-equal (number->string (sin (/ 1.0 0))) "NaN")
  (assert-true  (= (/ 1.0 0) (/ 1.0 0)))
  (assert-false (= (sqrt -1) (sqrt -1)))
  (assert-true  (< 1.0 (/ 1.0 0)))
  (assert-false (> (sqrt -1) 1.0))
  (assert-equal (signum (/ -1.0 0)) -1.0))
