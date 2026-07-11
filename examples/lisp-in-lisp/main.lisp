;;; lisp-in-lisp -- the metacircular evaluator (Lisp 1.5 / SICP classic).
;;; Shows: eval/apply written in the language they implement, alist
;;; environments, closures as tagged data, and the homoiconic payoff:
;;; the test programs are just quoted forms.
;;; Run: cargo run -- examples/lisp-in-lisp/main.lisp

(defun env-lookup (sym env)
  (let ((hit (assoc sym env)))
    (if hit (cdr hit) (error (concat "unbound: " (princ-to-string sym))))))

(defun env-extend (params args env)
  (append (mapcar #'cons params args) env))

(defun mini-eval (form env)
  (cond
    ((numberp form) form)
    ((stringp form) form)
    ((symbolp form) (env-lookup form env))
    ((equal (car form) 'quote) (cadr form))
    ((equal (car form) 'if)
     (if (mini-eval (cadr form) env)
         (mini-eval (caddr form) env)
         (mini-eval (car (cdddr form)) env)))
    ((equal (car form) 'lambda)
     (list 'closure (cadr form) (caddr form) env))
    (t (mini-apply (mini-eval (car form) env)
                   (mapcar (lambda (a) (mini-eval a env)) (cdr form))))))

(defun mini-apply (f args)
  (cond
    ((and (consp f) (equal (car f) 'closure))
     (mini-eval (caddr f)
                (env-extend (cadr f) args (car (cdddr f)))))
    ((and (consp f) (equal (car f) 'prim))
     (apply (cdr f) args))
    (t (error "not a function"))))

(def $global
  (list (cons '+ (cons 'prim #'+)) (cons '- (cons 'prim #'-))
        (cons '* (cons 'prim #'*)) (cons '= (cons 'prim #'=))
        (cons '< (cons 'prim #'<)) (cons 'cons (cons 'prim #'cons))
        (cons 'car (cons 'prim #'car)) (cons 'cdr (cons 'prim #'cdr))
        (cons 'null-p (cons 'prim #'null))))

(defun run (form) (mini-eval form $global))

;; Recursion without define: the Y-ish self-application trick.
(def $fact
  '((lambda (f) ((lambda (x) (f (lambda (v) ((x x) v))))
                 (lambda (x) (f (lambda (v) ((x x) v))))))
    (lambda (self) (lambda (n) (if (= n 0) 1 (* n (self (- n 1))))))))

(format t "((mini) 1+2*3): ~a~%" (run '(+ 1 (* 2 3))))
(format t "((mini) fact 10): ~a~%" (run (list $fact 10)))

;; self-check: arithmetic, closures capture, factorial via Y.
(if (and (= (run '(+ 1 (* 2 3))) 7)
         (= (run '(((lambda (x) (lambda (y) (+ x y))) 3) 4)) 7)
         (equal (run '(quote (a b))) '(a b))
         (= (run (list $fact 10)) 3628800))
    (print 'ok)
    (error "metacircular self-check failed"))
