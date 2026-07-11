;;; symbolic-diff -- differentiate expressions as data (SICP 2.3.2).
;;; Shows: the homoiconic payoff -- calculus as list surgery, a
;;; simplifier via the 0.3 rulebook (defrule/apply-rules), and checking
;;; derivatives numerically.
;;; Run: cargo run -- examples/symbolic-diff/main.lisp

(defun deriv (e x)
  (cond
    ((numberp e) 0)
    ((symbolp e) (if (equal e x) 1 0))
    ((equal (car e) '+) (list '+ (deriv (cadr e) x) (deriv (caddr e) x)))
    ((equal (car e) '*)
     (list '+
           (list '* (cadr e) (deriv (caddr e) x))
           (list '* (deriv (cadr e) x) (caddr e))))
    (t (error "unknown operator"))))

;; Simplification as rewrite rules (lib/24-rules.lisp).
(defrule diff-add-zero-l (+ 0 ?x) ?x)
(defrule diff-add-zero-r (+ ?x 0) ?x)
(defrule diff-mul-zero-l (* 0 ?x) 0)
(defrule diff-mul-zero-r (* ?x 0) 0)
(defrule diff-mul-one-l (* 1 ?x) ?x)
(defrule diff-mul-one-r (* ?x 1) ?x)

(defun simplify (e) (apply-rules e))

(def $e '(+ (* x x) (* 3 x)))
(def $d (simplify (deriv $e 'x)))
(format t "d/dx ~a = ~a~%" $e $d)

;; Numeric spot check: evaluate the derivative as code.
(defun eval-at (e x-val)
  (eval (list 'let (list (list 'x x-val)) e) (the-environment)))

;; self-check: (x^2 + 3x)' = 2x + 3 at several points, plus the
;; simplifier actually shrank the raw derivative.
(if (and (every (lambda (v) (= (eval-at $d v) (+ (* 2 v) 3)))
                (list 0 1 5 -3))
         (equal (simplify (deriv 'x 'x)) 1)
         (equal (simplify (deriv 42 'x)) 0)
         (< (length (flatten $d)) (length (flatten (deriv $e 'x)))))
    (print 'ok)
    (error "symbolic-diff self-check failed"))
