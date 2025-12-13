(defun onep (x)
  "Test if x equals 1"
  (= x 1))

(defun minusp (x)
  "Test if x is negative"
  (< x 0))

(defun add1 (x)
  "Add 1 to x"
  (+ x 1))

(defun sub1 (x)
  "Subtract 1 from x"
  (- x 1))

(defun max (&rest numbers)
  "Return maximum of numbers"
  (cond ((null numbers) (error "MAX requires at least one argument"))
        ((null (cdr numbers)) (car numbers))
        (t (let ((max-rest (apply #'max (cdr numbers))))
             (if (> (car numbers) max-rest)
                 (car numbers)
                 max-rest)))))

(defun min (&rest numbers)
  "Return minimum of numbers"
  (cond ((null numbers) (error "MIN requires at least one argument"))
        ((null (cdr numbers)) (car numbers))
        (t (let ((min-rest (apply #'min (cdr numbers))))
             (if (< (car numbers) min-rest)
                 (car numbers)
                 min-rest)))))

(defun abs (x)
  "Absolute value"
  (if (minusp x) (- x) x))
