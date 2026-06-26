;;; Control-flow and binding macros (issue #142, epic #141).
;;;
;;; Pure Lisp, non-mutating. NOTE: this implementation has no unquote-splicing
;;; (`,@`), so macros build their expansion with explicit LIST/CONS the way the
;;; DEFUN macro in 00-core.lisp does.

(defmacro when (test &rest body)
  "Evaluate BODY in an implicit PROGN when TEST is non-NIL; otherwise NIL."
  (list 'if test (cons 'progn body) nil))

(defmacro unless (test &rest body)
  "Evaluate BODY in an implicit PROGN when TEST is NIL; otherwise NIL."
  (list 'if test nil (cons 'progn body)))

(defmacro prog1 (first &rest body)
  "Evaluate FIRST and BODY in order; return the value of FIRST."
  (let ((tmp (gensym)))
    (list 'let (list (list tmp first))
          (cons 'progn body)
          tmp)))

;; CASE: (case key (vals body...) ... (t body...))
;; A clause key may be a single datum, a list of data, or T / OTHERWISE for the
;; default. KEY is evaluated once. Matching uses EQUAL.
(defun case-clause->cond-clause (k clause)
  (let ((sel (car clause))
        (body (cdr clause)))
    (cond ((or (eq sel t) (eq sel 'otherwise))
           (cons t body))
          ((consp sel)
           (cons (list 'member k (list 'quote sel)) body))
          (t
           (cons (list 'equal k (list 'quote sel)) body)))))

(defmacro case (key &rest clauses)
  "Multi-way branch on KEY (evaluated once), compared with EQUAL.
A clause is (DATUM body...) or (LIST-OF-DATA body...); T or OTHERWISE is the default."
  (let ((k (gensym)))
    (list 'let (list (list k key))
          (cons 'cond
                (mapcar (lambda (clause)
                                  (case-clause->cond-clause k clause)) clauses)))))

(defun typecase-type-test (k type)
  "Return a test expression checking that the value bound to K has TYPE.
Recognized types: NUMBER INTEGER FLOAT STRING SYMBOL CHAR CHARACTER CONS LIST
NULL ATOM."
  (cond ((eq type 'number)    (list 'numberp k))
        ((eq type 'integer)   (list 'and (list 'numberp k) (list 'not (list 'floatp k))))
        ((eq type 'float)     (list 'floatp k))
        ((eq type 'string)    (list 'stringp k))
        ((eq type 'symbol)    (list 'symbolp k))
        ((eq type 'char)      (list 'charp k))
        ((eq type 'character) (list 'charp k))
        ((eq type 'cons)      (list 'consp k))
        ((eq type 'list)      (list 'listp k))
        ((eq type 'null)      (list 'null k))
        ((eq type 'atom)      (list 'atom k))
        (t (error "TYPECASE: unknown type specifier"))))

(defun typecase-clause->cond-clause (k clause)
  (let ((type (car clause))
        (body (cdr clause)))
    (cond ((or (eq type t) (eq type 'otherwise))
           (cons t body))
          (t (cons (typecase-type-test k type) body)))))

(defmacro typecase (key &rest clauses)
  "Multi-way branch on the TYPE of KEY (evaluated once).
A clause is (TYPE body...); T or OTHERWISE is the default. Recognized types:
NUMBER INTEGER FLOAT STRING SYMBOL CHAR CHARACTER CONS LIST NULL ATOM."
  (let ((k (gensym)))
    (list 'let (list (list k key))
          (cons 'cond
                (mapcar (lambda (clause)
                                  (typecase-clause->cond-clause k clause)) clauses)))))

;; DOLIST: (dolist (var list [result]) body...)
(defmacro dolist (spec &rest body)
  "Iterate VAR over the elements of LIST, evaluating BODY each time.
Returns RESULT (evaluated with VAR bound to NIL) or NIL."
  (let ((var (car spec))
        (lst (car (cdr spec)))
        (result (cdr (cdr spec))))
    (list 'progn
          (list 'mapc (cons 'lambda (cons (list var) body)) lst)
          (if result
              (list 'let (list (list var nil)) (car result))
              nil))))

;; DOTIMES: (dotimes (var count [result]) body...)
(defmacro dotimes (spec &rest body)
  "Iterate VAR from 0 below COUNT, evaluating BODY each time.
Returns RESULT (with VAR bound to COUNT) or NIL."
  (let ((var (car spec))
        (count-form (car (cdr spec)))
        (result (cdr (cdr spec)))
        (n (gensym)))
    (list 'let (list (list n count-form))
          (cons 'for (cons (list var 0 (list '- n 1)) body))
          (if result
              (list 'let (list (list var n)) (car result))
              nil))))
