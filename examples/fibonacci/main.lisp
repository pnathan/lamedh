;;; fibonacci -- three ways, with the instrumentation to compare them.
;;; Shows: naive recursion, hash memoization, iterative TCO, and
;;; step-count (0.3) making the complexity difference visible.
;;; Run: cargo run -- examples/fibonacci/main.lisp

(defun fib-naive (n)
  (if (< n 2) n (+ (fib-naive (- n 1)) (fib-naive (- n 2)))))

(def $fib-memo (make-hash-table))
(defun fib-memo (n)
  (cond ((< n 2) n)
        ((has-key-p $fib-memo n) (gethash $fib-memo n))
        (t (put! $fib-memo n (+ (fib-memo (- n 1)) (fib-memo (- n 2)))))))

(defun fib-iter-aux (a b n)
  (if (= n 0) a (fib-iter-aux b (+ a b) (- n 1))))
(defun fib-iter (n) (fib-iter-aux 0 1 n))

;; step-count returns (steps . value).
(format t "fib(20) naive: ~a  (~a kernel steps)~%"
        (fib-naive 20) (car (step-count (fib-naive 20))))
(format t "fib(20) memo:  ~a  (~a kernel steps)~%"
        (fib-memo 20) (car (step-count (fib-memo 20))))
(format t "fib(20) iter:  ~a  (~a kernel steps)~%"
        (fib-iter 20) (car (step-count (fib-iter 20))))

;; self-check: all three agree, and fib(50) is reachable iteratively.
(if (and (= (fib-naive 20) 6765)
         (= (fib-memo 20) 6765)
         (= (fib-iter 20) 6765)
         (= (fib-iter 50) 12586269025))
    (print 'ok)
    (error "fibonacci self-check failed"))
