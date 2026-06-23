;;; Kernel-style vau operative combinators.
;;; These demonstrate derived forms built on (vau (operands-param env-param) body...).

(def $if
  ($vau (x e)
    (if (eval (car x) e)
        (eval (cadr x) e)
        (eval (caddr x) e))))

(def $and
  ($vau (x e)
    (cond ((null x) t)
          ((null (cdr x)) (eval (car x) e))
          ((eval (car x) e) (eval (cons '$and (cdr x)) e))
          (t nil))))

(def $or
  ($vau (x e)
    (cond ((null x) nil)
          ((eval (car x) e) t)
          (t (eval (cons '$or (cdr x)) e)))))

(def $sequence
  ($vau (x e)
    (cond ((null x) nil)
          ((null (cdr x)) (eval (car x) e))
          (t (eval (car x) e)
             (eval (cons '$sequence (cdr x)) e)))))
