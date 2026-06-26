(defmacro defun (name params &rest body)
  (if (stringp (car body))
    (let ((lambda-expr (cons 'lambda (cons params (cdr body)))))
      `(def ,name ,lambda-expr ,(car body)))
    (let ((lambda-expr (cons 'lambda (cons params body))))
      `(def ,name ,lambda-expr))))

;; Named vau operative with optional docstring as first body form.
;; (defvau name (operands-param env-param) ["docstring"] body...)
;; Expands to (def name ($vau (operands-param env-param) body...) "docstring").
(defmacro defvau (name params &rest body)
  (if (stringp (car body))
    (let ((vau-expr (cons '$vau (cons params (cdr body)))))
      `(def ,name ,vau-expr ,(car body)))
    (let ((vau-expr (cons '$vau (cons params body))))
      `(def ,name ,vau-expr))))

(defun prog2 (first second &rest rest)
  "Evaluate all forms; return the value of the second."
  second)

(defmacro cset (sym val)
  "Set the global value of symbol SYM (unevaluated) to VAL."
  `(setq ,sym ,val))

(defmacro csetq (sym val)
  "Alias for CSET; set the global value of SYM to VAL."
  `(setq ,sym ,val))
