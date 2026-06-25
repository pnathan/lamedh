;;; Modern "real-world-ish" workload — uses current lamedh features
;;; (hash tables, FOR/WHILE, LIST, multi-form LET) for benchmarking the
;;; current build. NOT portable to old revisions; for cross-version
;;; comparison use realistic.lisp instead.
;;;
;;; Drive with: (bench-ht REPS) — returns an integer checksum.

;; ---- 1. hash-table word-count / histogram ----
;; Hash a stream of N pseudo-keys into B buckets, counting occurrences,
;; then sum every bucket's count back out.
(defun ht-histogram (n buckets)
  (let ((h (make-hash-table)) (total 0))
    (for (i 1 n)
      (let ((k (remainder (* i 2654435761) buckets)))
        (set-bang h k (+ 1 (or (gethash h k) 0)))))
    (for (b 0 (- buckets 1))
      (setq total (+ total (or (gethash h b) 0))))
    total))

;; ---- 2. hash-table as a memo table: iterative fib with memoisation ----
(defun ht-memo-fib (n)
  (let ((m (make-hash-table)))
    (set-bang m 0 0)
    (set-bang m 1 1)
    (for (i 2 n)
      (set-bang m i (+ (gethash m (- i 1)) (gethash m (- i 2)))))
    (gethash m n)))

;; ---- 3. list building with LIST + nested FOR ----
(defun grid-sum (rows cols)
  (let ((acc 0))
    (for (r 1 rows)
      (for (c 1 cols)
        (setq acc (+ acc (remainder (* r c) 7)))))
    acc))

;; ---- 4. WHILE-driven list drain ----
(defun drain-count (lst)
  (let ((n 0))
    (while lst
      (setq n (+ n 1))
      (setq lst (cdr lst)))
    n))

(defun range-up (n acc)
  (cond ((zerop n) acc) (t (range-up (- n 1) (cons n acc)))))

;; ---- one unit of mixed work ----
(defun run-ht-once ()
  (+ (+ (ht-histogram 5000 64)
        (ht-memo-fib 90))
     (+ (grid-sum 80 80)
        (drain-count (range-up 2000 nil)))))

(defun bench-ht (reps)
  (let ((acc 0))
    (for (i 1 reps)
      (setq acc (+ acc (run-ht-once))))
    acc))
