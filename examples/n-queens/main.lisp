;;; n-queens -- backtracking search over board columns.
;;; Shows: recursion with an accumulator of placements, safety predicates
;;; over enumerate, and counting all solutions.
;;; Run: cargo run -- examples/n-queens/main.lisp

(defun safe-p (col placed)
  "COL is safe against PLACED (list of columns, most recent first)?"
  (every (lambda (pair)
           (let ((dist (1+ (car pair))) (c (cadr pair)))
             (and (not (= c col))
                  (not (= (abs (- c col)) dist)))))
         (enumerate placed)))

(defun solve (n placed count-so-far)
  "Number of complete placements extending PLACED."
  (if (= (length placed) n)
      (1+ count-so-far)
      (reduce (lambda (acc col)
                (if (safe-p col placed)
                    (solve n (cons col placed) acc)
                    acc))
              (iota n)
              count-so-far)))

(defun queens (n) (solve n () 0))

(for-each (list 4 5 6 7 8)
  (lambda (n) (format t "~a-queens: ~a solutions~%" n (queens n))))

;; self-check: the known sequence 2, 10, 4, 40, 92.
(if (equal (map (list 4 5 6 7 8) #'queens) (list 2 10 4 40 92))
    (print 'ok)
    (error "n-queens self-check failed"))
