(defun equal (a b)
  "Recursively checks if two S-expressions are equal."
  (if (atom a)
      (eq a b)
      (if (atom b)
          nil
          (if (equal (car a) (car b))
              (equal (cdr a) (cdr b))
              nil))))
