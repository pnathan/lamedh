;;; Common Lisp compatibility layer (issue #245).
;;;
;;; Closes the "CL by reflex" gap: the staples that fingers (and LLMs) type
;;; without thinking. Everything here is expressible over existing primitives,
;;; per the project's prefer-the-Lisp-layer philosophy.
;;;
;;; SETF places supported:
;;;   (setf sym v)                  -> (setq sym v)
;;;   (setf (gethash table k) v)    -> hash write   (either gethash arg order)
;;;   (setf (fetch a i) v)          -> (store a i v)      (also aref)
;;;   (setf (elt seq i) v)          -> array/hash-aware store
;;;   (setf (name obj) v)           -> (set-name! obj v)  (struct accessors)
;;;
;;; CAVEAT: like the rest of the layer this is a *practical* subset — PLACE
;;; subforms are evaluated once per mention (PUSH/POP/INCF mention the place
;;; twice), so keep places side-effect-free. CAR/CDR are not places because
;;; cons cells are immutable in Lamedh (issue #114).

;; --- setf and places --------------------------------------------------------

(defun setf-hash-store (table key value)
  "Write VALUE under KEY for a (gethash table key) place — collection
first, one order (0.3 regularity)."
  (set-bang table key value)
  value)

(defun setf-expand-1 (place value)
  "Expand one SETF pair into an assignment form."
  (cond
    ((symbolp place) (list 'setq place value))
    ((consp place)
     (let ((head (car place)))
       (cond
         ((eq head 'gethash)
          (list 'setf-hash-store (cadr place) (caddr place) value))
         ((or (eq head 'fetch) (eq head 'aref))
          (list 'store (cadr place) (caddr place) value))
         ((eq head 'elt)
          (list 'store (cadr place) (caddr place) value))
         ;; Accessor convention: (setf (point-x p) v) -> (set-point-x! p v).
         ;; Matches the mutators defstruct generates; an unknown accessor
         ;; surfaces as an unbound SET-...! error at the call site.
         (t (cons (intern (concat "SET-" (prin1-to-string head) "!"))
                  (list (cadr place) value))))))
    (t (error "setf: unsupported place"))))

(defun setf-expand (pairs)
  (cond ((null pairs) nil)
        ((null (cdr pairs)) (error "setf: odd number of arguments"))
        ((null (cddr pairs)) (setf-expand-1 (car pairs) (cadr pairs)))
        (t (cons 'progn
                 (cons (setf-expand-1 (car pairs) (cadr pairs))
                       (list (setf-expand (cddr pairs))))))))

(defmacro setf (&rest pairs)
  "CL-style SETF over the places listed in lib/21-cl-compat.lisp."
  (setf-expand pairs))

(defmacro push (item place)
  "Prepend ITEM to the list stored in PLACE; returns the new list."
  (setf-expand-1 place (list 'cons item place)))

(defmacro pop (place)
  "Remove and return the head of the list stored in PLACE."
  (list 'prog1 (list 'car place)
        (setf-expand-1 place (list 'cdr place))))

(defmacro incf (place &rest delta)
  "Increment PLACE by DELTA (default 1); returns the new value."
  (setf-expand-1 place (list '+ place (if delta (car delta) 1))))

(defmacro decf (place &rest delta)
  "Decrement PLACE by DELTA (default 1); returns the new value."
  (setf-expand-1 place (list '- place (if delta (car delta) 1))))

;; --- variable definition -----------------------------------------------------

(defmacro defparameter (name value &rest doc)
  "Define NAME as a dynamic variable and assign VALUE (CL DEFPARAMETER)."
  (cons 'defdynamic (cons name (cons value doc))))

;; --- sequence staples ---------------------------------------------------------

(defun remove (item lst)
  "Return LST without the elements EQUAL to ITEM (non-destructive)."
  (filter (lambda (x) (not (equal x item))) lst))

(defun count (item lst)
  "Number of elements of LST that are EQUAL to ITEM."
  (length (filter (lambda (x) (equal x item)) lst)))

(defun count-if (pred lst)
  "Number of elements of LST satisfying PRED."
  (length (filter pred lst)))

(defun copy-list (lst)
  "Return a fresh copy of the spine of LST."
  (if (consp lst)
      (cons (car lst) (copy-list (cdr lst)))
      lst))

(defun list-length (lst)
  "CL alias for LENGTH on lists."
  (length lst))

(defun cl-reverse-aux (lst acc)
  (if (null lst) acc (cl-reverse-aux (cdr lst) (cons (car lst) acc))))

(defun reverse (seq)
  "Reverse a list or a string (extends the list-only REVERSE from lib/01)."
  (if (stringp seq)
      (list->string (cl-reverse-aux (string->list seq) nil))
      (cl-reverse-aux seq nil)))

(defun nreverse (seq)
  "CL alias for REVERSE. Lamedh lists are immutable, so this is never
destructive — the reversed sequence is returned and must be used."
  (reverse seq))

(defun subseq (seq start &rest maybe-end)
  "Subsequence of a list or string from START (inclusive) to END (exclusive,
default: end of SEQ)."
  (if (stringp seq)
      (substring seq start
                 (if maybe-end (car maybe-end) (string-length seq)))
      (let ((end (if maybe-end (car maybe-end) (length seq))))
        (take (drop seq start) (- end start)))))

(defun elt (seq n)
  "Element N of a list, string (as a one-character string), or array."
  (cond ((stringp seq) (substring seq n (+ n 1)))
        ((arrayp seq) (fetch seq n))
        (t (nth n seq))))

;; --- accessors ----------------------------------------------------------------

(defun first (lst) "CL alias for CAR." (car lst))
(defun rest (lst) "CL alias for CDR." (cdr lst))
(defun second (lst) "CL alias for CADR." (cadr lst))
(defun third (lst) "CL alias for CADDR." (caddr lst))

;; --- numbers -------------------------------------------------------------------

(defun rem (a b)
  "CL REM: remainder truncating toward zero (alias of REMAINDER).
Contrast MOD, which is always non-negative."
  (remainder a b))

;; --- hash tables ----------------------------------------------------------------

;; REMHASH is the kernel builtin as of 0.3 (collection first).
