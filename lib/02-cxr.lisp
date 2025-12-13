;; Helper function for the defcxr macro.
;; It recursively builds the nested car/cdr expression.
(defun build-cxr-expr (ops)
  (if (null ops)
      'x
      (if (eq (car ops) 'a)
          `(car ,(build-cxr-expr (cdr ops)))
          `(cdr ,(build-cxr-expr (cdr ops))))))

;; Macro to generate CAR/CDR compositions
(defmacro defcxr (name operations)
  "Generate a CAR/CDR composition function"
  `(defun ,name (x)
     ,(build-cxr-expr (eval operations))))

;; Generate all 2-level combinations
(defcxr caar '(a a))
(defcxr cadr '(a d))
(defcxr cdar '(d a))
(defcxr cddr '(d d))

;; Generate 3-level combinations
(defcxr caaar '(a a a))
(defcxr caadr '(a a d))
(defcxr cadar '(a d a))
(defcxr caddr '(a d d))
(defcxr cdaar '(d a a))
(defcxr cdadr '(d a d))


(defcxr cddar '(d d a))
(defcxr cdddr '(d d d))

;; Generate 4-level combinations
(defcxr caaaar '(a a a a))
(defcxr caaadr '(a a a d))
(defcxr caadar '(a a d a))
(defcxr caaddr '(a a d d))
(defcxr cadaar '(a d a a))
(defcxr cadadr '(a d a d))
(defcxr caddar '(a d d a))
(defcxr cadddr '(a d d d))
(defcxr cdaaar '(d a a a))
(defcxr cdaadr '(d a a d))
(defcxr cdadar '(d a d a))
(defcxr cdaddr '(d a d d))
(defcxr cddaar '(d d a a))
(defcxr cddadr '(d d a d))
(defcxr cdddar '(d d d a))
(defcxr cddddr '(d d d d))
