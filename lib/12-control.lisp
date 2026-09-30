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
        (result (cdr (cdr spec)))
        (tail (gensym)))
    ;; A WHILE loop, not MAPC: constant stack, and natively compileable.
    ;; VAR gets a fresh binding per element, as the MAPC lambda gave it.
    (list 'let (list (list tail lst))
          (list 'while (list 'not (list 'null tail))
                (cons 'let (cons (list (list var (list 'car tail)))
                                 (if body body (list nil))))
                (list 'setq tail (list 'cdr tail)))
          (if result
              (list 'let (list (list var nil)) (car result))
              nil))))

;; DOTIMES-BODY-CLOSES-P: may FORM, once expanded, build a closure over the
;; loop variable? FOR reuses one counter slot for the whole loop, so a closure
;; made in its body sees the last value (#528). The walk expands global macro
;; heads (DOLIST, FLET, ... expand to LAMBDA) and answers T at any closure
;; constructor; QUOTE data is skipped. It is conservative: a false T only costs
;; a per-iteration LET, so running out of expansion fuel, or an expansion
;; that signals, answers T.
(defun dotimes-closure-head-p (head)
  (member head '(lambda function label macro fexpr vau
                 def define defexpr defmacro defun-typed defun*)))

(defun dotimes-body-closes-p (form fuel)
  (cond ((not (consp form)) nil)
        ((< fuel 1) t)
        ((eq (car form) 'quote) nil)
        ((dotimes-closure-head-p (car form)) t)
        (t (let ((x (handler-case (macroexpand form) (error (e) 'lambda))))
             (if (equal x form)
                 (dotimes-forms-close-p form fuel)
                 (dotimes-body-closes-p x (- fuel 1)))))))

(defun dotimes-forms-close-p (forms fuel)
  (cond ((not (consp forms)) nil)
        ((dotimes-body-closes-p (car forms) fuel) t)
        (t (dotimes-forms-close-p (cdr forms) fuel))))

;; DOTIMES: (dotimes (var count [result]) body...)
(defmacro dotimes (spec &rest body)
  "Iterate VAR from 0 below COUNT, evaluating BODY each time.
Each iteration binds VAR afresh, as DOLIST does: a closure made in BODY keeps
the value of its own iteration. Returns RESULT (with VAR bound to COUNT) or NIL."
  (let ((var (car spec))
        (count-form (car (cdr spec)))
        (result (cdr (cdr spec)))
        (n (gensym)))
    (list 'let (list (list n count-form))
          (cons 'for (cons (list var 0 (list '- n 1))
                           ;; Rebind only when BODY can close over VAR: the
                           ;; closure-free loop keeps FOR's single reused slot.
                           (if (dotimes-forms-close-p body 64)
                               (list (cons 'let (cons (list (list var var)) body)))
                               body)))
          (if result
              (list 'let (list (list var n)) (car result))
              nil))))

;;; ------------------------------------------------------------------
;;; Local operator bindings: flet / macrolet / fexprlet / vaulet.
;;;
;;; Lamedh is a Lisp-1, and operator dispatch resolves the head symbol
;;; through the ordinary lexical environment chain. So a name locally
;;; bound to a LAMBDA / MACRO / FEXPR / VAU value is automatically used
;;; as an operator inside that scope — these forms are just LET over the
;;; matching anonymous constructor. The kernel provides those four value
;;; constructors (LAMBDA, MACRO, FEXPR, VAU); the binding sugar lives here.
;;;
;;; Each clause is (name (params...) body...), mirroring DEFUN/DEFMACRO.
;;; Bindings are parallel (LET semantics): clauses do not see one another,
;;; which matches Common Lisp FLET / MACROLET. LABELS is the recursive
;;; exception: it binds every name first and then SETQs each to its LAMBDA,
;;; so the closures see one another (the classic letrec-by-mutation).

(defun make-oplet-binding (head clause)
  "Turn a (name (params...) body...) clause into a LET binding
(name (HEAD (params...) body...)) for the given constructor HEAD."
  (list (car clause) (cons head (cdr clause))))

(defmacro flet (bindings &rest body)
  "Locally bind named functions for the extent of BODY (non-recursive).
Each binding is (name (params...) body...)."
  (cons 'let
        (cons (mapcar (lambda (b) (make-oplet-binding 'lambda b)) bindings)
              body)))

(defmacro labels (bindings &rest body)
  "Locally bind named functions that may call themselves and one another
(Common Lisp LABELS). Each binding is (name (params...) body...). Expands to
a LET binding every name to NIL, then a SETQ of each name to its LAMBDA, then
BODY -- so every closure captures the same, completed bindings."
  (cons 'let
        (cons (mapcar (lambda (b) (list (car b) nil)) bindings)
              (append (mapcar (lambda (b)
                                (list 'setq (car b) (cons 'lambda (cdr b))))
                              bindings)
                      (if body body (list nil))))))

(defmacro macrolet (bindings &rest body)
  "Locally bind macros for the extent of BODY.
Each binding is (name (params...) body...); the bodies are expanded at
call sites just like DEFMACRO definitions."
  (cons 'let
        (cons (mapcar (lambda (b) (make-oplet-binding 'macro b)) bindings)
              body)))

(defmacro fexprlet (bindings &rest body)
  "Locally bind fexprs (unevaluated-argument operatives) for BODY.
Each binding is (name (params...) body...); the operands reach the body
unevaluated, as with DEFEXPR."
  (cons 'let
        (cons (mapcar (lambda (b) (make-oplet-binding 'fexpr b)) bindings)
              body)))

(defmacro vaulet (bindings &rest body)
  "Locally bind vau operatives for the extent of BODY.
Each binding is (name (operands env) body...): OPERANDS receives the
unevaluated operand list and ENV the caller's environment, as with VAU."
  (cons 'let
        (cons (mapcar (lambda (b) (make-oplet-binding 'vau b)) bindings)
              body)))
