;; Merge sort of 10^5 LCG-generated integers (one run); all loops tail-recursive.
;; Result: a rolling checksum of the sorted list, negated if it is unsorted.
(defun gen (n x acc)
  (if (= n 0) acc
      (gen (- n 1) (mod (+ (* x 1664525) 1013904223) 4294967296) (cons x acc))))
(defun rev (l acc) (if (null l) acc (rev (cdr l) (cons (car l) acc))))
(defun split (l a b)
  (if (null l) (cons a b)
      (split (cdr l) (cons (car l) b) a)))
(defun merge (a b acc)
  (if (null a) (rev acc b)
      (if (null b) (rev acc a)
          (if (< (car b) (car a))
              (merge a (cdr b) (cons (car b) acc))
              (merge (cdr a) b (cons (car a) acc))))))
(defun msort (l)
  (if (null l) l
      (if (null (cdr l)) l
          (let ((halves (split l '() '())))
            (merge (msort (car halves)) (msort (cdr halves)) '())))))
(defun check (l prev acc)
  (if (null l) acc
      (if (< (car l) prev) (- 0 1)
          (check (cdr l) (car l) (mod (+ (* acc 31) (car l)) 1000000007)))))
(defun run (reps)
  (let ((r 0) (tot 0))
    (while (< r reps)
      (setq tot (+ tot (check (msort (gen 100000 (+ r 42) '())) 0 0)))
      (setq r (+ r 1)))
    tot))
(print (run 1))
