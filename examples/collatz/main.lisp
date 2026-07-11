;;; collatz -- the 3n+1 conjecture: chain lengths and the champion.
;;; Shows: while-free tail recursion, sort-by (0.3), take, and enumerate.
;;; Run: cargo run -- examples/collatz/main.lisp

(defun collatz-length-aux (n steps)
  (cond ((= n 1) steps)
        ((evenp n) (collatz-length-aux (/ n 2) (1+ steps)))
        (t (collatz-length-aux (+ 1 (* 3 n)) (1+ steps)))))

(defun collatz-length (n) (collatz-length-aux n 0))

;; The five longest chains for starting points 1..1000.
(def $champions
  (take (sort-by (mapcar (lambda (n) (cons n (collatz-length n)))
                         (iota 1000 1))
                 #'cdr #'>)
        5))

(for-each (lambda (row)
    (let ((rank (car row)) (cell (cadr row)))
      (format t "~a. start ~a -> ~a steps~%"
              rank (car cell) (cdr cell)))) (enumerate $champions 1))

;; self-check: the known champion under 1000 is 871 with 178 steps.
(if (equal (car $champions) (cons 871 178))
    (print 'ok)
    (error "collatz self-check failed"))
