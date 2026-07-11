;;; newton-sqrt -- Newton's method for square roots (SICP 1.1.7).
;;; Shows: float arithmetic, convergence loops as recursion, closures
;;; over a tolerance, and checking against the builtin.
;;; Run: cargo run -- examples/newton-sqrt/main.lisp

(def $tolerance 0.0000001)

(defun average (a b) (* 0.5 (+ a b)))

(defun improve (guess x) (average guess (/ x guess)))

(defun good-enough-p (guess x)
  (< (abs (- (* guess guess) x)) $tolerance))

(defun newton-sqrt-aux (guess x)
  (if (good-enough-p guess x)
      guess
      (newton-sqrt-aux (improve guess x) x)))

(defun newton-sqrt (x) (newton-sqrt-aux 1.0 x))

(for-each (lambda (x)
            (format t "sqrt(~a) ~~ ~a (builtin ~a)~%" x (newton-sqrt x) (sqrt x)))
          (list 2.0 9.0 100.0 0.25))

;; self-check: within tolerance of the builtin everywhere.
(if (every (lambda (x) (< (abs (- (newton-sqrt x) (sqrt x))) 0.001))
           (list 2.0 9.0 100.0 0.25 12345.0))
    (print 'ok)
    (error "newton-sqrt self-check failed"))
