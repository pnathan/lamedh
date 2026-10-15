(defun <= (a &rest more)
  "Monotone non-decreasing chain: (<= a b c) means a <= b and b <= c."
  (if (null more)
      t
      (and (not (> a (car more))) (apply #'<= more))))

(defun >= (a &rest more)
  "Monotone non-increasing chain: (>= a b c) means a >= b and b >= c."
  (if (null more)
      t
      (and (not (< a (car more))) (apply #'>= more))))

(defun /= (a b)
  "True if a and b are numerically unequal."
  (not (= a b)))

(defun onep (x)
  "Test if x equals 1"
  (= x 1))

(defun minusp (x)
  "Test if x is negative"
  (< x 0))

(defun $max-loop (best rest)
  "Tail-recursive MAX reduction (constant stack depth)."
  (if (null rest)
      best
      ($max-loop (if (> (car rest) best) (car rest) best) (cdr rest))))

(defun $min-loop (best rest)
  "Tail-recursive MIN reduction (constant stack depth)."
  (if (null rest)
      best
      ($min-loop (if (< (car rest) best) (car rest) best) (cdr rest))))

; The reduction runs over the reversed list so that ties resolve as the
; original right fold did (the rightmost of equal values wins, which is
; observable for signed zeros).
(defun max (&rest numbers)
  "Return maximum of numbers"
  (cond ((null numbers) (error "MAX requires at least one argument"))
        ((null (cdr numbers)) (car numbers))
        (t (let ((rev (reverse numbers)))
             ($max-loop (car rev) (cdr rev))))))

(defun min (&rest numbers)
  "Return minimum of numbers"
  (cond ((null numbers) (error "MIN requires at least one argument"))
        ((null (cdr numbers)) (car numbers))
        (t (let ((rev (reverse numbers)))
             ($min-loop (car rev) (cdr rev))))))

(defun abs (x)
  "Absolute value"
  ;; `(+ x 0)`, not `x`: -0.0 is not MINUSP, and -0.0 + 0 = +0.0 (#518).
  ;; For an integer it is the identity.
  (if (minusp x) (- x) (+ x 0)))
