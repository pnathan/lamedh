;;; matrix-algebra -- multiply, transpose, identity, power.
;;; Shows: matrices as lists of rows, transpose via N-list mapcar zip
;;; (0.3), dot products with reduce, and fibonacci via matrix power as
;;; the cross-check.
;;; Run: cargo run -- examples/matrix-algebra/main.lisp

(defun transpose (m)
  "Rows become columns: N-list mapcar with #'list zips them."
  (apply #'mapcar (cons #'list m)))

(defun dot (u v)
  (reduce #'+ (mapcar #'* u v) 0))

(defun mat-mul (a b)
  (let ((bt (transpose b)))
    (mapcar (lambda (row) (mapcar (lambda (col) (dot row col)) bt)) a)))

(defun identity-mat (n)
  (mapcar (lambda (i)
            (mapcar (lambda (j) (if (= i j) 1 0)) (iota n)))
          (iota n)))

(defun mat-pow (m k)
  (cond ((= k 0) (identity-mat (length m)))
        ((evenp k) (let ((half (mat-pow m (/ k 2)))) (mat-mul half half)))
        (t (mat-mul m (mat-pow m (- k 1))))))

(def $a '((1 2) (3 4)))
(format t "A        = ~a~%" $a)
(format t "A^T      = ~a~%" (transpose $a))
(format t "A x A    = ~a~%" (mat-mul $a $a))
(format t "A x I    = ~a~%" (mat-mul $a (identity-mat 2)))

;; Fibonacci via ((1 1) (1 0))^n -- fast matrix exponentiation.
(defun fib-mat (n) (car (cdr (car (mat-pow '((1 1) (1 0)) n)))))
(format t "fib(30) via matrices: ~a~%" (fib-mat 30))

;; self-check: algebra laws on this instance + the fibonacci oracle.
(if (and (equal (mat-mul $a (identity-mat 2)) $a)
         (equal (transpose (transpose $a)) $a)
         (equal (mat-mul $a $a) '((7 10) (15 22)))
         (= (fib-mat 30) 832040)
         (= (fib-mat 50) 12586269025))
    (print 'ok)
    (error "matrix-algebra self-check failed"))
