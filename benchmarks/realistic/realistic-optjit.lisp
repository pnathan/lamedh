;;; Realistic workload with OptJIT-compiled hot functions.
;;; Same as realistic-jit.lisp but uses defun-typed-opt (source
;;; optimizer runs before HM checking and Cranelift compilation).

;; ---- OptJIT-compiled scalar functions ----
(defun-typed-opt (fib-opt int64) ((n int64))
  (if (< n 2) n (+ (fib-opt (- n 1)) (fib-opt (- n 2)))))

(defun-typed-opt (tsum-opt int64) ((n int64) (acc int64))
  (if (= n 0) acc (tsum-opt (- n 1) (+ acc n))))

(defun-typed-opt (ack-opt int64) ((m int64) (n int64))
  (if (= m 0) (+ n 1)
    (if (= n 0) (ack-opt (- m 1) 1)
      (ack-opt (- m 1) (ack-opt m (- n 1))))))

;; ---- tree-walker functions (same as realistic-jit.lisp) ----
(defun bench-array-lists-opt (n)
  (let ((arr (make-array n))
        (total 0))
    (for (i 0 (- n 1))
      (aset arr i (+ i 1)))
    (for (i 0 (- n 1))
      (aset arr i (* (aref arr i) (aref arr i))))
    (for (i 0 (- n 1))
      (let ((v (aref arr i)))
        (if (zerop (remainder v 2))
            (setq total (+ total v))
            nil)))
    (+ total (* 2 n))))

(defun bench-ht-lookup-opt (n reps)
  (let ((h (make-hash-table)) (total 0))
    (for (i 1 n)
      (set-bang h i (* i 3)))
    (for (i 0 (- reps 1))
      (setq total (+ total (or (gethash h (+ 1 (remainder i n))) 0))))
    total))

(defun process-array-records-opt (n)
  (let ((amts (make-array n))
        (cats (make-array n))
        (t1 0) (t2 0) (t3 0) (mx 0))
    (for (i 0 (- n 1))
      (aset amts i (remainder (* (+ i 1) 7) 100))
      (aset cats i (+ 1 (remainder (+ i 1) 3))))
    (for (i 0 (- n 1))
      (let ((amt (aref amts i))
            (cat (aref cats i)))
        (if (> amt mx) (setq mx amt) nil)
        (if (= cat 1) (setq t1 (+ t1 amt))
          (if (= cat 2) (setq t2 (+ t2 amt))
            (setq t3 (+ t3 amt))))))
    (+ t1 (+ t2 (+ t3 (+ n mx))))))

(defun run-opt-once ()
  (+ (bench-array-lists-opt 300)
     (+ (bench-ht-lookup-opt 250 1500)
        (+ (fib-opt 23)
           (+ (tsum-opt 300 0)
              (+ (ack-opt 2 50)
                 (process-array-records-opt 300)))))))

(defun bench-opt (reps)
  (let ((acc 0))
    (for (i 1 reps)
      (setq acc (+ acc (run-opt-once))))
    acc))
