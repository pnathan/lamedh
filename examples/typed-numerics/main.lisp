;;; typed-numerics -- the gradual-to-native story on one page.
;;; Shows: defun quietly compiling through the one-door pipeline,
;;; defun-typed pinning signatures, explain-compile as the "why (not)"
;;; oracle, see-type on derived schemes, and value parity between tiers.
;;; Run: cargo run -- examples/typed-numerics/main.lisp

;; Plain defun: types pinned by the literal, compiles behind the scenes.
(defun sum-squares (n)
  (if (= n 0) 0 (+ (* n n) (sum-squares (- n 1)))))

;; Explicitly typed: the signature is a contract.
(defun-typed (hypot2 int64) ((a int64) (b int64))
  (+ (* a a) (* b b)))

;; A lambda-touching body stays interpreted -- explain-compile says why.
(defun with-callback (f x) (funcall f x))

(format t "sum-squares(100) = ~a~%" (sum-squares 100))
(format t "hypot2(3,4)      = ~a~%" (hypot2 3 4))

(def $verdict-typed (explain-compile 'hypot2))
(def $verdict-hof (explain-compile 'with-callback))
(format t "hypot2:        ~a~%" $verdict-typed)
(format t "with-callback: ~a~%" $verdict-hof)

;; self-check: values right; the typed one reports a compiled tier with
;; the right signature; the HOF reports a blocker instead of a lie;
;; typed misuse is a checker error.
(defun tier-of (verdict) (cdr (assoc 'tier verdict)))
(if (and (= (sum-squares 100) 338350)
         (= (hypot2 3 4) 25)
         (equal (tier-of $verdict-typed) 'compiled)
         (not (equal (tier-of $verdict-hof) 'compiled))
         (contains-p (check-type (hypot2 1.5 2)) "type error")
         (= (with-callback #'1+ 41) 42))
    (print 'ok)
    (error "typed-numerics self-check failed"))
