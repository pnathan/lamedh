;; Port-specific regression tests (not a copy of a tests/lisp/ fixture).
;; #538: MOD is the Euclidean remainder, 0 <= r < |b| (KERNEL Part V),
;; not CL's floored MOD. Expected values are the Rust reference's.

(deftest mod-euclidean-sign-matrix
  (assert-equal (mod 7 2) 1)
  (assert-equal (mod -7 2) 1)
  (assert-equal (mod 7 -2) 1)
  (assert-equal (mod -7 -2) 1)
  (assert-equal (mod 7 3) 1)
  (assert-equal (mod -7 3) 2)
  (assert-equal (mod 7 -3) 1)
  (assert-equal (mod -7 -3) 2))

(deftest mod-euclidean-exact-and-zero-dividend
  (assert-equal (mod 0 5) 0)
  (assert-equal (mod 0 -5) 0)
  (assert-equal (mod 6 -3) 0)
  (assert-equal (mod -6 3) 0)
  (assert-equal (mod -6 -3) 0)
  (assert-equal (mod 2 5) 2)
  (assert-equal (mod -2 5) 3)
  (assert-equal (mod 2 -5) 2)
  (assert-equal (mod -2 -5) 3))

(deftest mod-euclidean-remainder-unchanged
  ;; REMAINDER stays truncated, sign following the dividend.
  (assert-equal (remainder -7 2) -1)
  (assert-equal (remainder 7 -2) 1))

(deftest mod-division-by-zero-errors
  (assert-equal (handler-case (mod 7 0) (error (e) 'div0)) 'div0)
  (assert-equal (handler-case (mod -7 0) (error (e) 'div0)) 'div0)
  (assert-equal (handler-case (mod 0 0) (error (e) 'div0)) 'div0))
