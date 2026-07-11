;;; macro-control -- grow the language: when-not, until, and swap!.
;;; Shows: defmacro with quasiquote and &rest (macro parameter lists take
;;; &rest, not a dotted tail), macroexpand as the debugging tool, a vau
;;; operative reading its own source, and hygiene-by-gensym. (The stdlib
;;; already has UNLESS, so we define fresh names -- shadowing core forms
;;; in examples is how bugs are born.)
;;; Run: cargo run -- examples/macro-control/main.lisp

(defmacro when-not (test &rest body)
  `(if ,test () (progn ,@body)))

(defmacro until (test &rest body)
  `(while (not ,test) ,@body))

(defmacro swap! (a b)
  "Swap two SETQ-able places (symbols)."
  (let ((tmp (gensym)))
    `(let ((,tmp ,a))
       (setq ,a ,b)
       (setq ,b ,tmp))))

;; A vau operative sees its argument UNEVALUATED -- no quoting needed.
(defvau show (x e)
  "(show expr) -- print expr = value, return the value."
  (let ((v (eval (car x) e)))
    (format t "~a = ~a~%" (car x) v)
    v))

(def $log ())
(when-not (> 1 2)
  (setq $log (cons 'fired $log)))

(let ((n 0))
  (until (>= n 3)
    (setq n (1+ n))
    (setq $log (cons n $log))))

(def $x 'first)
(def $y 'second)
(swap! $x $y)

(show (+ 40 2))

;; self-check: control flow behaved; swap swapped even with the capture-
;; prone names; macroexpand shows the template; the operative returned
;; its value.
(if (and (equal $log '(3 2 1 fired))
         (equal $x 'second)
         (equal $y 'first)
         (equal (car (macroexpand '(when-not c a))) 'if)
         (let ((tmp 1) (b 2))         ; would capture without gensym
           (swap! tmp b)
           (and (= tmp 2) (= b 1)))
         (= (show (* 6 7)) 42))
    (print 'ok)
    (error "macro-control self-check failed"))
