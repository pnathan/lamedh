;;; Portable "real-world-ish" workload for cross-version benchmarking.
;;; Uses only primitives common to old and new lamedh (no for/while, no
;;; hash tables, no list/mod/sub1) so the SAME file runs on both binaries.
;;; Each workload returns an integer; the driver sums them into a checksum so
;;; (a) the work can't be optimized away and (b) both versions must agree.

;; ---- generic list helpers (function values passed as args) ----
(defun range-up (n acc)            ; (range-up n nil) => (1 2 ... n)
  (cond ((zerop n) acc)
        (t (range-up (- n 1) (cons n acc)))))

(defun my-map (f lst)
  (cond ((null lst) nil)
        (t (cons (f (car lst)) (my-map f (cdr lst))))))

(defun my-filter (pred lst)
  (cond ((null lst) nil)
        ((pred (car lst)) (cons (car lst) (my-filter pred (cdr lst))))
        (t (my-filter pred (cdr lst)))))

(defun my-foldl (f acc lst)
  (cond ((null lst) acc)
        (t (my-foldl f (f acc (car lst)) (cdr lst)))))

;; ---- 1. list processing: sum of squares of evens in 1..n, plus some
;;        append/reverse/length churn to exercise allocation ----
(defun bench-lists (n)
  (let ((lst (range-up n nil)))
    (progn
      (my-foldl (lambda (a b) (+ a b))
                (length (reverse (append lst lst)))
                (my-filter (lambda (x) (zerop (remainder x 2)))
                           (my-map (lambda (x) (* x x)) lst))))))

;; ---- 2. association-list key/value store: build then do many lookups ----
(defun build-alist (n acc)
  (cond ((zerop n) acc)
        (t (build-alist (- n 1) (cons (cons n (* n 3)) acc)))))

(defun alookup (k al)
  (let ((pair (assoc k al)))
    (cond ((null pair) 0) (t (cdr pair)))))

(defun bench-alist (al reps acc)
  (prog (i sum)
    (setq i 0) (setq sum acc)
   loop
    (cond ((< i reps)
           (progn
             (setq sum (+ sum (alookup (+ 1 (remainder i 200)) al)))
             (setq i (+ i 1))
             (go loop))))
    (return sum)))

;; ---- 3. function-call heavy: classic recursion + tail recursion + ackermann ----
(defun fib (n) (cond ((< n 2) n) (t (+ (fib (- n 1)) (fib (- n 2))))))
(defun tsum (n acc) (cond ((zerop n) acc) (t (tsum (- n 1) (+ acc n)))))
(defun ack (m n)
  (cond ((zerop m) (+ n 1))
        ((zerop n) (ack (- m 1) 1))
        (t (ack (- m 1) (ack m (- n 1))))))

;; ---- 4. a deliberately messy large function: walk a list of "records"
;;        (each an alist), categorise, accumulate per-category totals, track
;;        count and max — lots of locals, branches, assoc, and a go-loop ----
(defun rec-field (key rec)
  (let ((p (assoc key rec))) (cond ((null p) 0) (t (cdr p)))))

(defun build-records (n acc)
  (cond ((zerop n) acc)
        (t (build-records (- n 1)
             (cons (cons (cons (quote id)  n)
                   (cons (cons (quote amt) (remainder (* n 7) 100))
                   (cons (cons (quote cat) (+ 1 (remainder n 3))) nil)))
                   acc)))))

(defun process-records (records)
  (prog (cur rec amt cat t1 t2 t3 cnt mx)
    (setq cur records)
    (setq t1 0) (setq t2 0) (setq t3 0) (setq cnt 0) (setq mx 0)
   loop
    (cond ((null cur) (go done)))
    (setq rec (car cur))
    (setq amt (rec-field (quote amt) rec))
    (setq cat (rec-field (quote cat) rec))
    (setq cnt (+ cnt 1))
    (cond ((> amt mx) (setq mx amt)))
    (cond ((= cat 1) (setq t1 (+ t1 amt)))
          ((= cat 2) (setq t2 (+ t2 amt)))
          (t         (setq t3 (+ t3 amt))))
    (setq cur (cdr cur))
    (go loop)
   done
    (return (+ (+ t1 t2) (+ t3 (+ cnt mx))))))

;; ---- one unit of mixed work (fixed sizes; recursion depths kept modest so
;;      the same workload runs on the old binary's smaller native stack) ----
(defun run-once ()
  (+ (+ (bench-lists 300)
        (bench-alist (build-alist 250 nil) 1500 0))
     (+ (+ (fib 23) (tsum 300 0))
        (+ (ack 2 50)
           (process-records (build-records 300 nil))))))

;; ---- driver: repeat REPS times via a prog loop (no deep recursion, so it
;;      runs the same whether or not the engine has TCO) ----
(defun bench (reps)
  (prog (i acc)
    (setq i 0) (setq acc 0)
   loop
    (cond ((< i reps)
           (progn (setq acc (+ acc (run-once))) (setq i (+ i 1)) (go loop))))
    (return acc)))
