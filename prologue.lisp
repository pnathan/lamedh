(defmacro defun (name params &rest body)
  (if (stringp (car body))
    (let ((lambda-expr (cons 'lambda (cons params (cdr body)))))
      `(def ,name ,lambda-expr ,(car body)))
    (let ((lambda-expr (cons 'lambda (cons params body))))
      `(def ,name ,lambda-expr))))

(defun null (x)
  (eq x nil))

(defun pairlis (keys vals)
  (if (or (null keys) (null vals))
      nil
      (cons (cons (car keys) (car vals))
            (pairlis (cdr keys) (cdr vals)))))

(defun documentation (sym)
  "Retrieves the docstring for a symbol."
  (GETP sym "docstring"))

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

(defun equal (a b)
  (if (atom a)
      (eq a b)
      (if (atom b)
          nil
          (if (equal (car a) (car b))
              (equal (cdr a) (cdr b))
              nil))))

;; Essential list functions
(defun append (x y)
  (cond ((null x) y)
        (t (cons (car x) (append (cdr x) y)))))

(defun member (item list)
  (cond ((null list) nil)
        ((equal item (car list)) list)
        (t (member item (cdr list)))))

(defun length (list)
  (cond ((null list) 0)
        (t (+ 1 (length (cdr list))))))

(defun reverse-aux (lst acc)
  (if (null lst)
      acc
      (reverse-aux (cdr lst) (cons (car lst) acc))))

(defun reverse (list)
  (reverse-aux list nil))
