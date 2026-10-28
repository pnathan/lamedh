(defun <= (a &rest more)
  "Monotone non-decreasing chain: (<= a b c) means a <= b and b <= c."
  (if (null more)
      t
      (and (or (< a (car more)) (= a (car more))) (apply #'<= more))))

(defun >= (a &rest more)
  "Monotone non-increasing chain: (>= a b c) means a >= b and b >= c."
  (if (null more)
      t
      (and (or (> a (car more)) (= a (car more))) (apply #'>= more))))

(defun /= (a b)
  "True if a and b are numerically unequal."
  (not (= a b)))

(defun onep (x)
  "Test if x equals 1"
  (= x 1))

(defun minusp (x)
  "Test if x is negative"
  (< x 0))

;; MIN and MAX follow IEEE 754-2019 maximum/minimum for floats: a NaN
;; argument makes the result NaN, and -0.0 orders below +0.0. Untyped calls
;; keep float contagion: if any argument is a float, the result is a float
;; (#530). The reduction is a tail-recursive fold, so a long argument list
;; runs in constant stack depth.

(defun $nan-p (x)
  "True only for a float NaN: NaN is the one value unequal to itself."
  (/= x x))

(defun $neg-zero-p (x)
  "True only for the float -0.0: the one zero whose reciprocal is negative."
  (if (floatp x) (if (= x 0) (< (/ 1.0 x) 0) nil) nil))

(defun $max2 (a b)
  "The larger of A and B, NaN-propagating, with -0.0 below +0.0."
  (cond (($nan-p a) a)
        (($nan-p b) b)
        ((> a b) a)
        ((> b a) b)
        (($neg-zero-p a) b)
        (t a)))

(defun $min2 (a b)
  "The smaller of A and B, NaN-propagating, with -0.0 below +0.0."
  (cond (($nan-p a) a)
        (($nan-p b) b)
        ((< a b) a)
        ((< b a) b)
        (($neg-zero-p a) a)
        (t b)))

(defun $max-fold (acc rest)
  "Fold $max2 over REST, starting from ACC (tail-recursive)."
  (if (null rest) acc ($max-fold ($max2 acc (car rest)) (cdr rest))))

(defun $min-fold (acc rest)
  "Fold $min2 over REST, starting from ACC (tail-recursive)."
  (if (null rest) acc ($min-fold ($min2 acc (car rest)) (cdr rest))))

(defun $any-float-p (xs)
  "True if any element of the list XS is a float."
  (if (null xs) nil (if (floatp (car xs)) t ($any-float-p (cdr xs)))))

(defun $promote-float (r numbers)
  "R, the value selected from NUMBERS, as a float when any of them is one.
A float R is returned untouched: `(+ -0.0 0.0)` would turn -0.0 into 0.0."
  (if (floatp r) r (if ($any-float-p numbers) (+ r 0.0) r)))

(defun max (&rest numbers)
  "Return maximum of numbers"
  (if (null numbers)
      (error "MAX requires at least one argument")
      ($promote-float ($max-fold (car numbers) (cdr numbers)) numbers)))

(defun min (&rest numbers)
  "Return minimum of numbers"
  (if (null numbers)
      (error "MIN requires at least one argument")
      ($promote-float ($min-fold (car numbers) (cdr numbers)) numbers)))

(defun abs (x)
  "Absolute value"
  ;; `(+ x 0)`, not `x`: -0.0 is not MINUSP, and -0.0 + 0 = +0.0 (#518).
  ;; For an integer it is the identity.
  (if (minusp x) (- x) (+ x 0)))
