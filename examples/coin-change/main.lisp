;;; coin-change -- count the ways, and find the fewest coins.
;;; Shows: the two classic DP shapes (combinations vs minimization) over
;;; one coin set, arrays as tables.
;;; Run: cargo run -- examples/coin-change/main.lisp

(def $coins (list 1 5 10 25))

(defun ways (amount)
  "Distinct multisets of coins totalling AMOUNT."
  (let ((table (array (1+ amount))))
    (dotimes (i (1+ amount)) (put! table i 0))
    (put! table 0 1)
    (for-each $coins
      (lambda (c)
        (let ((a c))
          (while (<= a amount)
            (put! table a (+ (ref table a) (ref table (- a c))))
            (setq a (1+ a))))))
    (ref table amount)))

(defun fewest (amount)
  "Minimum number of coins totalling AMOUNT (-1 if impossible)."
  (let ((table (array (1+ amount))))
    (put! table 0 0)
    (dotimes (i amount)
      (let ((a (1+ i)))
        (put! table a
              (reduce (lambda (best c)
                        (if (and (>= a c) (ref table (- a c)))
                            (let ((cand (1+ (ref table (- a c)))))
                              (if (or (null best) (< cand best)) cand best))
                            best))
                      $coins
                      ()))))
    (let ((r (ref table amount))) (if r r -1))))

(for-each (list 11 25 63 99)
  (lambda (a)
    (format t "~a cents: ~a ways, fewest ~a coins~%" a (ways a) (fewest a))))

;; self-check: classic US-coin answers.
(if (and (= (ways 25) 13)
         (= (fewest 63) 6)     ; 25+25+10+1+1+1
         (= (fewest 30) 2)     ; 25+5
         (= (ways 0) 1))
    (print 'ok)
    (error "coin-change self-check failed"))
