;; Special forms: PROG, LET, LABEL, DEFEXPR, DEFMACRO, MACROEXPAND, quasiquote.

(deftest sf-prog-loop
  ;; PROG with labels, SETQ, GO, and RETURN
  (assert-equal
    (prog (i)
      (setq i 0)
      loop
      (if (= i 3) (return i) nil)
      (setq i (+ i 1))
      (go loop))
    3))

(deftest sf-let-basic
  (assert-equal (let ((a 1) (b 2)) (+ a b)) 3))

(deftest sf-let-single-body
  (assert-equal (let ((x 10)) x) 10))

(deftest sf-label
  ;; LABEL returns a callable function value
  (assert-true (functionp (label fact (lambda (n) (if (= n 0) 1 (* n (fact (- n 1)))))))))

(deftest sf-defexpr
  ;; DEFEXPR: the single formal is bound to the list of all unevaluated arguments
  (progn
    (defexpr sf-test-fexpr (x) x)
    (assert-equal (sf-test-fexpr (+ 1 2)) '((+ 1 2)))))

(deftest sf-defmacro
  ;; DEFMACRO: expands and then evaluates; quoting the arg returns the symbol
  (progn
    (defmacro sf-test-mac (x) (list 'quote x))
    (assert-equal (sf-test-mac hello) 'HELLO)))

(deftest sf-macroexpand
  ;; MACROEXPAND returns the expansion of a macro call without evaluating it.
  ;; The DEFUN macro now wraps the DEF in a PROGN that also triggers the
  ;; call-graph hook (guarded by BOUNDP so it is safe before file 19 is loaded).
  (assert-equal
    (macroexpand '(defun foo (x) (+ x 1)))
    '(PROGN
       (DEF FOO (LAMBDA (X) (+ X 1)))
       (IF (BOUNDP 'DEFUN-UPDATE-CALL-GRAPH!)
           (DEFUN-UPDATE-CALL-GRAPH! 'FOO '(X) '((+ X 1)))
           NIL)
       'FOO)))

(deftest sf-quasiquote-literal
  ;; Quasiquote with no unquotes produces a plain list
  (assert-equal (quasiquote (+ 1 2)) '(+ 1 2)))

(deftest sf-quasiquote-unquote
  ;; Unquote evaluates a sub-expression inside a quasiquote
  (assert-equal (quasiquote (a (unquote (+ 1 2)) c)) '(A 3 C)))

(deftest sf-quasiquote-splice
  ;; ,@ splices a list's elements into the surrounding list
  (let ((xs '(2 3 4)))
    (assert-equal `(1 ,@xs 5) '(1 2 3 4 5))))

(deftest sf-quasiquote-splice-empty
  ;; Splicing an empty list contributes nothing
  (let ((xs '()))
    (assert-equal `(start ,@xs end) '(START END))))

(deftest sf-quasiquote-splice-and-unquote
  ;; Unquote and unquote-splicing compose within one quasiquote
  (let ((a 1) (xs '(2 3)))
    (assert-equal `(,a ,@xs done) '(1 2 3 DONE))))
