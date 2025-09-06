(defmacro defun (name params body)
  `(def ,name (lambda ,params ,body)))

(defun null (x)
  (eq x nil))
