;;; Realistic workload with JIT-compiled hot functions.
;;; fib, tsum, and ackermann are compiled via defun-typed;
;;; array/hash-table operations stay in the tree-walker (the JIT
;;; tier doesn't handle those yet).

;; ---- JIT-compiled scalar functions ----
(defun-typed (fib-jit int64) ((n int64))
  (if (< n 2) n (+ (fib-jit (- n 1)) (fib-jit (- n 2)))))

(defun-typed (tsum-jit int64) ((n int64) (acc int64))
  (if (= n 0) acc (tsum-jit (- n 1) (+ acc n))))

(defun-typed (ack-jit int64) ((m int64) (n int64))
  (if (= m 0) (+ n 1)
    (if (= n 0) (ack-jit (- m 1) 1)
      (ack-jit (- m 1) (ack-jit m (- n 1))))))

;; ---- tree-walker functions (arrays/hash tables) ----
(defun bench-array-lists-jit (n)
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

(defun bench-ht-lookup-jit (n reps)
  (let ((h (make-hash-table)) (total 0))
    (for (i 1 n)
      (set-bang h i (* i 3)))
    (for (i 0 (- reps 1))
      (setq total (+ total (or (gethash h (+ 1 (remainder i n))) 0))))
    total))

(defun process-array-records-jit (n)
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

;; ---- one unit of mixed work (JIT scalar + tree-walker containers) ----
(defun run-jit-once ()
  (+ (bench-array-lists-jit 300)
     (+ (bench-ht-lookup-jit 250 1500)
        (+ (fib-jit 23)
           (+ (tsum-jit 300 0)
              (+ (ack-jit 2 50)
                 (process-array-records-jit 300)))))))

(defun bench-jit (reps)
  (let ((acc 0))
    (for (i 1 reps)
      (setq acc (+ acc (run-jit-once))))
    acc))
