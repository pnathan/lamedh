(defmacro defun (name params &rest body)
  (if (stringp (car body))
    (let ((lambda-expr (cons 'lambda (cons params (cdr body)))))
      `(def ,name (label ,name ,lambda-expr) ,(car body)))
    (let ((lambda-expr (cons 'lambda (cons params body))))
      `(def ,name (label ,name ,lambda-expr)))))

(defun null (x)
  (eq x nil))

(defun pairlis (keys vals)
  (if (or (null keys) (null vals))
      nil
      (cons (cons (car keys) (car vals))
            (pairlis (cdr keys) (cdr vals)))))

(defun documentation (sym)
  "Retrieves the docstring for a symbol."
  (GETP sym "docstring"))
