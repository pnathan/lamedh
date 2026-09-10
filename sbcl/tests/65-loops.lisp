;; Iterative special forms (FOR, WHILE) and multi-form LET / single-frame LET*.
;; Auto-loaded by tests/test_lisp_suite.rs via (load-directory "tests/lisp").

;; ── FOR ────────────────────────────────────────────────────────────────────

(deftest for-sums-inclusive-range
  ;; End bound is inclusive: 1+2+3+4+5 = 15
  (assert-equal
    (prog (s) (setq s 0) (for (i 1 5) (setq s (+ s i))) (return s))
    15))

(deftest for-returns-nil
  (assert-nil (for (i 1 3) i)))

(deftest for-positive-step
  ;; 0,2,4,6,8,10 -> 30
  (assert-equal
    (prog (s) (setq s 0) (for (i 0 10 2) (setq s (+ s i))) (return s))
    30))

(deftest for-negative-step
  ;; 10,8,6,4,2 -> 30
  (assert-equal
    (prog (s) (setq s 0) (for (i 10 1 -2) (setq s (+ s i))) (return s))
    30))

(deftest for-zero-iterations-when-start-past-end
  (assert-equal
    (prog (n) (setq n 0) (for (i 5 1) (setq n (+ n 1))) (return n))
    0))

(deftest for-nested
  ;; 10 x 10 grid -> 100
  (assert-equal
    (prog (n)
      (setq n 0)
      (for (i 1 10) (for (j 1 10) (setq n (+ n 1))))
      (return n))
    100))

(deftest for-body-assignment-does-not-change-iteration
  ;; Clobbering the loop var inside the body must not affect the driver.
  (assert-equal
    (prog (n) (setq n 0) (for (i 1 3) (setq i 100) (setq n (+ n 1))) (return n))
    3))

;; ── WHILE ────────────────────────────────────────────────────────────────────

(deftest while-counts-up
  ;; 0+1+2+3 = 6
  (assert-equal
    (prog (c n)
      (setq c 0) (setq n 0)
      (while (< n 4) (setq c (+ c n)) (setq n (+ n 1)))
      (return c))
    6))

(deftest while-body-skipped-when-false
  (assert-equal
    (prog (n) (setq n 0) (while nil (setq n 99)) (return n))
    0))

(deftest while-returns-nil
  (assert-nil
    (prog (n) (setq n 0) (return (while (< n 2) (setq n (+ n 1)))))))

(deftest while-drains-a-list
  (assert-equal
    (prog (lst n)
      (setq lst '(a b c d)) (setq n 0)
      (while lst (setq n (+ n 1)) (setq lst (cdr lst)))
      (return n))
    4))

;; ── LET / LET* multi-form bodies and scoping ─────────────────────────────────

(deftest let-multi-form-body
  ;; Two body forms: mutate then use; returns the last form's value.
  (assert-equal (let ((x 2) (y 3)) (setq x (+ x 1)) (* x y)) 9))

(deftest let-is-parallel
  ;; Inner y binds to the OUTER x (= 1), not the sibling x.
  (assert-equal (let ((x 1)) (let ((x 2) (y x)) y)) 1))

(deftest let*-is-sequential
  ;; Here y sees the just-bound inner x (= 2).
  (assert-equal (let ((x 1)) (let* ((x 2) (y x)) y)) 2))

(deftest let*-multi-form-body
  (assert-equal (let* ((x 1) (y (+ x 1))) (setq x 10) (+ x y)) 12))

(deftest let*-chains-dependent-bindings
  ;; a=2, b=a*a=4, c=b+1=5
  (assert-equal (let* ((a 2) (b (* a a)) (c (+ b 1))) c) 5))

(deftest let*-closure-captures-single-frame
  ;; A lambda bound later sees an earlier binding in the same frame.
  (assert-equal (let* ((a 5) (f (lambda () a))) (funcall f)) 5))
