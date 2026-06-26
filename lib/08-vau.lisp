;;; Kernel-style vau operative combinators.
;;; These demonstrate derived forms built on (defvau name (operands-param env-param) body...).

(defvau $if (x e)
  "Kernel-style conditional operative. Evaluates the test in the caller's
environment; evaluates exactly one of the two branches."
  (if (eval (car x) e)
      (eval (cadr x) e)
      (eval (caddr x) e)))

(defvau $and (x e)
  "Short-circuit AND operative. Evaluates operands left-to-right in the
caller's environment; stops at the first false value and returns NIL.
Returns T if all operands are true, or T for an empty operand list."
  (cond ((null x) t)
        ((null (cdr x)) (eval (car x) e))
        ((eval (car x) e) (eval (cons '$and (cdr x)) e))
        (t nil)))

(defvau $or (x e)
  "Short-circuit OR operative. Evaluates operands left-to-right in the
caller's environment; returns T at the first true value without evaluating
the rest. Returns NIL for an empty operand list."
  (cond ((null x) nil)
        ((eval (car x) e) t)
        (t (eval (cons '$or (cdr x)) e))))

(defvau $sequence (x e)
  "Sequences evaluation of operands in the caller's environment, returning
the value of the last form. Analogous to PROGN but as an operative."
  (cond ((null x) nil)
        ((null (cdr x)) (eval (car x) e))
        (t (eval (car x) e)
           (eval (cons '$sequence (cdr x)) e))))
