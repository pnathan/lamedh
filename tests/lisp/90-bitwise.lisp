;; Bitwise and shift operation coverage.

(deftest bw-ash-left
  (assert-equal (ash 1 4) 16)
  (assert-equal (ash 3 2) 12))

(deftest bw-ash-right
  (assert-equal (ash 16 -1) 8)
  (assert-equal (ash 8  -3) 1))

(deftest bw-lognot
  (assert-equal (lognot  0)  -1)
  (assert-equal (lognot -1)   0)
  (assert-equal (lognot  1)  -2))

(deftest bw-logand
  (assert-equal (logand 12 10) 8)
  (assert-equal (logand 15  0) 0)
  (assert-equal (logand 15 15) 15))

(deftest bw-logor
  (assert-equal (logor 12 10) 14)
  (assert-equal (logor  0  0)  0)
  (assert-equal (logor  8  4) 12))

(deftest bw-logxor
  (assert-equal (logxor 12 10) 6)
  (assert-equal (logxor  0  0) 0)
  (assert-equal (logxor 15 15) 0))

(deftest bw-leftshift
  (assert-equal (leftshift 1 4) 16)
  (assert-equal (leftshift 3 2) 12)
  (assert-equal (leftshift 1 0)  1))
