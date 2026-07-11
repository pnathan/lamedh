;;; mergesort -- split, sort halves, merge. O(n log n) and stable.
;;; Shows: multiple-value-free splitting via take/drop, a two-pointer
;;; merge, and stability checked with sort-by on pairs.
;;; Run: cargo run -- examples/mergesort/main.lisp

(defun merge-sorted (a b lessp)
  (cond ((null a) b)
        ((null b) a)
        ((funcall lessp (car b) (car a))
         (cons (car b) (merge-sorted a (cdr b) lessp)))
        (t (cons (car a) (merge-sorted (cdr a) b lessp)))))

(defun mergesort (lst lessp)
  (let ((n (length lst)))
    (if (< n 2)
        lst
        (merge-sorted (mergesort (take lst (/ n 2)) lessp)
                      (mergesort (drop lst (/ n 2)) lessp)
                      lessp))))

(format t "~a~%" (mergesort (list 5 2 8 1 9 3 7 4 6) #'<))

;; self-check 1: agrees with builtin sort on random input.
(random-seed! 11)
(def $xs (mapcar (lambda (i) (random 1000)) (iota 300)))
(if (equal (mergesort (copy $xs) #'<) (sort (copy $xs) #'<)) () (error "mergesort wrong"))

;; self-check 2: STABLE -- equal keys keep their original order.
(def $pairs (list (cons 1 'a) (cons 0 'b) (cons 1 'c) (cons 0 'd) (cons 1 'e)))
(if (equal (mergesort $pairs (lambda (x y) (< (car x) (car y))))
           (list (cons 0 'b) (cons 0 'd) (cons 1 'a) (cons 1 'c) (cons 1 'e)))
    (print 'ok)
    (error "mergesort not stable"))
