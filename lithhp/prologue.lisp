(defmacro defun (name params body)
  `(def ,name (lambda ,params ,body)))
