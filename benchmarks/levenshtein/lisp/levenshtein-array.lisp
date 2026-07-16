;;; Levenshtein distance — array-based two-row DP (modern Lamedh).
;;; Uses native arrays with O(1) access instead of lists with O(n) access.

(defun lev (s1 s2)
  "Levenshtein edit distance between strings S1 and S2."
  (let ((m (length s1))
        (n (length s2)))
    (if (< n m) (lev s2 s1)
      (let ((prev (make-array (+ m 1)))
            (curr (make-array (+ m 1))))
        ;; Initialize prev = [0, 1, 2, ..., m]
        (for (i 0 m) (aset prev i i))
        ;; DP fill
        (for (i 1 n)
          (aset curr 0 i)
          (for (j 1 m)
            (let ((cost (if (eq (index s1 (- j 1)) (index s2 (- i 1))) 0 1)))
              (aset curr j
                (min (+ (aref prev j) 1)
                     (min (+ (aref curr (- j 1)) 1)
                          (+ (aref prev (- j 1)) cost))))))
          ;; Swap rows
          (let ((tmp prev))
            (setq prev curr)
            (setq curr tmp)))
        (aref prev m)))))
