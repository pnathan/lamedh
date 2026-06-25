(defun <= (a b)
  "True if a is less than or equal to b."
  (not (> a b)))

(defun >= (a b)
  "True if a is greater than or equal to b."
  (not (< a b)))

(defun /= (a b)
  "True if a and b are numerically unequal."
  (not (= a b)))

(defun onep (x)
  "Test if x equals 1"
  (= x 1))

(defun minusp (x)
  "Test if x is negative"
  (< x 0))

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
