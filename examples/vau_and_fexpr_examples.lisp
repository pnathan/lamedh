;;; vau_and_fexpr_examples -- four VAU operatives, four DEFEXPR fexprs.
;;;
;;; Both forms receive their operands UNEVALUATED, which is what lets them
;;; implement control structures a plain function cannot (a function call
;;; evaluates every argument before the function body ever runs). They
;;; differ in how the caller's environment is exposed:
;;;
;;;   (defvau name (operands env) body...)   -- Kernel-style operative.
;;;     ENV is an explicit parameter bound to the caller's environment.
;;;     Nothing is evaluated unless the body calls (eval expr env).
;;;
;;;   (defexpr name (operands) body...)      -- classic fexpr.
;;;     No environment parameter. Bare (eval expr) resolves against the
;;;     ambient dynamic environment at the point of the eval call.
;;;
;;; Run: cargo run -- examples/vau_and_fexpr_examples.lisp

;;; --- VAU ---------------------------------------------------------------

;; 1. A conditional: evaluate the test, then exactly one branch, all in
;;    the caller's environment.
(defvau $unless (ops e)
  "(\\$unless test then) -- like UNLESS, but built from scratch."
  (if (eval (car ops) e) nil (eval (cadr ops) e)))

;; 2. A place-swap: the operands are bare symbols, never evaluated as
;;    expressions -- they're read with CAR/CADR and mutated with SETQ
;;    against the captured caller environment.
(defvau $swap! (ops e)
  "(\\$swap! a b) -- exchange the values of two SETQ-able places."
  (let ((a (car ops)) (b (cadr ops)))
    (let ((tmp (eval a e)))
      (eval (list 'setq a (eval b e)) e)
      (eval (list 'setq b tmp) e))))

;; 3. A manual, lazy LET: bind NAME in a fresh child environment before
;;    evaluating BODY there. Demonstrates MAKE-ENVIRONMENT plus threading
;;    a second, derived environment through EVAL.
(defvau $let1 (ops e)
  "(\\$let1 name val-expr body) -- bind NAME to VAL-EXPR's value, then run BODY."
  (let ((name (car ops)) (val-expr (cadr ops)) (body (caddr ops)))
    (let ((e2 (make-environment e)))
      (eval (list 'setq name (eval val-expr e)) e2)
      (eval body e2))))

;; 4. A loop: self-recurses on its own unevaluated operand list, so the
;;    TEST and BODY forms are re-read on every iteration rather than
;;    being pre-evaluated once.
(defvau $while (ops e)
  "(\\$while test body...) -- loop while TEST is true."
  (let ((test (car ops)) (body (cdr ops)))
    (if (eval test e)
        (progn (eval (cons '$sequence body) e)
               (eval (cons '$while ops) e))
        nil)))

(print ($unless nil "unless: taken"))
(print ($unless t "unless: skipped"))

(setq x 1)
(setq y 2)
($swap! x y)
(print (list 'swap! x y))

(print (list '$let1 ($let1 z (+ 2 3) (* z z))))

(setq i 0)
($while (< i 3) (print (list 'while i)) (setq i (+ i 1)))

;;; --- FEXPR ---------------------------------------------------------------

;; 1. QUOTE, built by hand -- the operand list arrives unevaluated.
(defexpr my-quote (args)
  "(my-quote expr) -- return EXPR unevaluated."
  (car args))

;; 2. A control structure: UNLESS as a fexpr instead of a vau. Note there
;;    is no environment parameter -- EVAL below just uses the ambient
;;    dynamic environment at the call site.
(defexpr my-unless (args)
  "(my-unless test then) -- evaluate THEN only when TEST is false."
  (if (eval (car args)) nil (eval (cadr args))))

;; 3. A tracing wrapper: capture the call form as data, print it, then
;;    decide to evaluate it. Only possible because the form arrives as
;;    an unevaluated s-expression.
(defexpr trace-call (args)
  "(trace-call form) -- print FORM and its result, then return the result."
  (let ((form (car args)))
    (print (list 'calling form))
    (let ((result (eval form)))
      (print (list 'result result))
      result)))

;; 4. COND rebuilt as a fexpr: recurses over its own unevaluated clause
;;    list, evaluating only the winning clause's test and body.
(defexpr my-cond (clauses)
  "(my-cond (test body)...) -- evaluate the body of the first true clause."
  (if (null clauses)
      nil
      (if (eval (caar clauses))
          (eval (cadar clauses))
          (eval (cons 'my-cond (cdr clauses))))))

(print (my-quote (+ 1 2)))
(print (my-unless nil "unless: ran"))

(setq n 5)
(trace-call (* n n))

(print (my-cond ((= n 1) "one") ((= n 5) "five") (t "other")))
