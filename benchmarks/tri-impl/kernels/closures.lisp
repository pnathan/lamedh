;; Closures and higher-order functions: map/filter/fold with lambdas and a
;; captured-variable adder over a 10^4-element list, 60 rounds.
(defun rev (l acc) (if (null l) acc (rev (cdr l) (cons (car l) acc))))
(defun iota (n acc) (if (= n 0) acc (iota (- n 1) (cons n acc))))
(defun map-onto (f l acc) (if (null l) (rev acc '()) (map-onto f (cdr l) (cons (f (car l)) acc))))
(defun filter-onto (p l acc)
  (if (null l) (rev acc '())
      (if (p (car l)) (filter-onto p (cdr l) (cons (car l) acc)) (filter-onto p (cdr l) acc))))
(defun fold (f acc l) (if (null l) acc (fold f (f acc (car l)) (cdr l))))
(defun make-adder (k) (lambda (x) (+ x k)))
(defun run (reps)
  (let ((xs (iota 10000 '())) (r 0) (tot 0))
    (while (< r reps)
      (setq tot (+ tot
                   (fold (lambda (a b) (+ a b)) 0
                         (filter-onto (lambda (v) (= (mod v 3) 0))
                                      (map-onto (make-adder r)
                                                (map-onto (lambda (v) (* v 2)) xs '()) '()) '()))))
      (setq r (+ r 1)))
    tot))
(print (run 60))
