(defmacro defun (name params body)
  `(def ,name (lambda ,params ,body)))

(defun null (x)
  (eq x nil))

(defun pairlis (keys vals)
  (if (or (null keys) (null vals))
      nil
      (cons (cons (car keys) (car vals))
            (pairlis (cdr keys) (cdr vals)))))
