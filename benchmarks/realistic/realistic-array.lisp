;;; Modern realistic workload — uses arrays, hash tables, FOR/WHILE,
;;; and modern string ops.  Drive with: (bench-arr REPS)

;; ---- 1. array sum-of-squares of evens ----
(defun bench-array-lists (n)
  (let ((arr (make-array n))
        (total 0))
    (for (i 0 (- n 1))
      (aset arr i (+ i 1)))
    ;; map: square each element
    (for (i 0 (- n 1))
      (aset arr i (* (aref arr i) (aref arr i))))
    ;; filter + fold: sum the even ones
    (for (i 0 (- n 1))
      (let ((v (aref arr i)))
        (if (zerop (remainder v 2))
            (setq total (+ total v))
            nil)))
    (+ total (* 2 n))))

;; ---- 2. hash-table key-value store (same as realistic-hashtable) ----
(defun bench-ht-lookup (n reps)
  (let ((h (make-hash-table)) (total 0))
    (for (i 1 n)
      (set-bang h i (* i 3)))
    (for (i 0 (- reps 1))
      (setq total (+ total (or (gethash h (+ 1 (remainder i n))) 0))))
    total))

;; ---- 3. recursive + tail-recursive + ackermann ----
(defun fib-arr (n) (if (< n 2) n (+ (fib-arr (- n 1)) (fib-arr (- n 2)))))
(defun tsum-arr (n acc) (if (zerop n) acc (tsum-arr (- n 1) (+ acc n))))
(defun ack-arr (m n)
  (if (zerop m) (+ n 1)
    (if (zerop n) (ack-arr (- m 1) 1)
      (ack-arr (- m 1) (ack-arr m (- n 1))))))

;; ---- 4. array-based record processing ----
(defun process-array-records (n)
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

;; ---- one unit of mixed work ----
(defun run-arr-once ()
  (+ (bench-array-lists 300)
     (+ (bench-ht-lookup 250 1500)
        (+ (fib-arr 23)
           (+ (tsum-arr 300 0)
              (+ (ack-arr 2 50)
                 (process-array-records 300)))))))

(defun bench-arr (reps)
  (let ((acc 0))
    (for (i 1 reps)
      (setq acc (+ acc (run-arr-once))))
    acc))
