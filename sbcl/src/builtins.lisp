;;;; builtins.lisp -- native (CL-backed) Lamedh primitives.
;;;;
;;;; Everything that can reasonably be written in Lamedh itself lives in
;;;; sbcl/lib/*.lisp instead (mirroring the reference implementation's own
;;;; prefer-the-Lisp-layer philosophy); this file holds only the primitives
;;;; those bootstrap files are built on.

(in-package #:lamedh-rt)

(defun bool (x) (if x *t-sym* nil))

(defmacro defbuiltin (name lambda-list &body body)
  "Bind NAME (a string) in the global environment to a native function."
  `(env-set-local *global-env* (lsym ,name) (lambda ,lambda-list ,@body)))

(defun lapply-fn (fn args)
  "The one calling convention every FUNCALL/APPLY/HOF callback goes through."
  (cond
    ((functionp fn) (apply fn args))
    ((lambda-obj-p fn)
     (let* ((env (make-child-env (lambda-obj-env fn)))
            (dyn (bind-params env (lambda-obj-params fn) (lambda-obj-rest fn) args "lambda")))
       (with-dyn-pairs dyn (leval (lambda-obj-body fn) env))))
    ((fexpr-obj-p fn) (lamedh-error "cannot FUNCALL/APPLY a fexpr: it needs unevaluated operands"))
    ((macro-obj-p fn) (lamedh-error "cannot FUNCALL/APPLY a macro"))
    ((vau-obj-p fn) (lamedh-error "cannot FUNCALL/APPLY a vau"))
    (t (lamedh-error (format nil "not a function: ~A" (lprint-to-string fn))))))

(defun lamedh-eq (a b)
  "Lamedh's EQ: identity for conses/symbols/callables, but VALUE equality
for the immutable atomic types (numbers, characters, strings) -- matching
the reference implementation, where LispVal's derived structural equality
makes EQ on two equal atoms true regardless of allocation identity."
  (cond
    ((and (numberp a) (numberp b)) (eql a b))
    ((and (characterp a) (characterp b)) (char= a b))
    ((and (stringp a) (stringp b)) (string= a b))
    (t (eq a b))))

(defun lamedh-equal (a b) (lamedh-truthy-p (lapply-fn (env-resolve *global-env* (lsym "EQUAL")) (list a b))))

;;; ---- core kernel: cons cells, identity, predicates --------------------------

(defbuiltin "CAR" (x) (if (consp x) (car x) (if (null x) nil (lamedh-error (format nil "CAR: not a list: ~A" (lprint-to-string x))))))
(defbuiltin "CDR" (x) (if (consp x) (cdr x) (if (null x) nil (lamedh-error (format nil "CDR: not a list: ~A" (lprint-to-string x))))))
(defbuiltin "CONS" (a b) (cons a b))
(defbuiltin "ATOM" (x) (bool (not (consp x))))
(defbuiltin "EQ" (a b) (bool (lamedh-eq a b)))
(defbuiltin "NOT" (x) (bool (null x)))
(defbuiltin "$LENGTH" (lst)
  (let ((n 0) (cur lst))
    (loop
      (cond ((null cur) (return n))
            ((consp cur) (incf n) (setf cur (cdr cur)))
            (t (lamedh-error "length: not a proper list"))))))
(defbuiltin "NTH" (n lst) (nth n lst))
(defbuiltin "NTHCDR" (n lst) (nthcdr n lst))
(defbuiltin "LAST" (lst) (last lst))
(defbuiltin "LIST" (&rest args) args)
(defbuiltin "MAPCAR" (fn &rest lists) (apply #'mapcar (lambda (&rest xs) (lapply-fn fn xs)) lists))
(defbuiltin "MAPLIST" (fn lst) (loop for tail on lst collect (lapply-fn fn (list tail))))
(defbuiltin "ASSOC" (key alist) (find key alist :key #'car :test #'lamedh-equal))
(defbuiltin "SUBST" (new old tree)
  (labels ((walk (x) (cond ((lamedh-equal x old) new)
                            ((consp x) (cons (walk (car x)) (walk (cdr x))))
                            (t x))))
    (walk tree)))
(defbuiltin "SUBLIS" (alist tree)
  (labels ((walk (x) (let ((cell (assoc x alist :test #'lamedh-equal)))
                        (cond (cell (cdr cell))
                              ((consp x) (cons (walk (car x)) (walk (cdr x))))
                              (t x)))))
    (walk tree)))
(defbuiltin "INDEX" (s i) (string (char (->str s) i)))
(defbuiltin "EXPLODE" (sym) (map 'list (lambda (c) (intern-lamedh (string c))) (symbol-name sym)))
(defbuiltin "IMPLODE" (lst) (intern-lamedh (apply #'concatenate 'string (mapcar #'symbol-name lst))))
(defbuiltin "MAKNAM" (lst) (intern-lamedh (apply #'concatenate 'string (mapcar #'symbol-name lst))))
(defbuiltin "APPEND" (&rest lists) (apply #'append lists))

(defbuiltin "NUMBERP" (x) (bool (numberp x)))
(defbuiltin "FIXP" (x) (bool (and (integerp x))))
(defbuiltin "FLOATP" (x) (bool (floatp x)))
(defbuiltin "STRINGP" (x) (bool (stringp x)))
(defbuiltin "SYMBOLP" (x) (bool (or (null x) (symbolp x))))
(defbuiltin "CHARP" (x) (bool (characterp x)))
(defbuiltin "FUNCTIONP" (x) (bool (callable-p x)))
(defbuiltin "ARRAYP" (x) (bool (simple-vector-p x)))
(defbuiltin "HASH-TABLE-P" (x) (bool (hash-table-p x)))
(defbuiltin "BOUNDP" (sym) (bool (env-boundp (or *current-env* *global-env*) sym)))
(defbuiltin "GETP" (sym key) (getp sym key))
(defbuiltin "PUTP" (sym key val) (putp sym key val))
(defbuiltin "REMPROP" (sym key) (remprop* sym key))
(defbuiltin "PLIST" (sym) (plist-flat sym))

(defvar *flags* (make-hash-table :test 'eq))
(defbuiltin "SET-FLAG" (sym) (setf (gethash sym *flags*) t) sym)
(defbuiltin "FLAG-SET-P" (sym) (bool (gethash sym *flags*)))
(defbuiltin "CLEAR-FLAG" (sym) (remhash sym *flags*) sym)

;;; ---- bitwise --------------------------------------------------------------------

(defbuiltin "ASH" (n count) (ash n count))
(defbuiltin "LEFTSHIFT" (n count) (ash n count))
(defbuiltin "LOGNOT" (n) (lognot n))
(defbuiltin "LOGAND" (&rest ns) (apply #'logand ns))
(defbuiltin "LOGIOR" (&rest ns) (apply #'logior ns))
(defbuiltin "LOGXOR" (&rest ns) (apply #'logxor ns))

;;; ---- funcall / apply / eval --------------------------------------------------

(defbuiltin "FUNCALL" (fn &rest args) (lapply-fn fn args))
(defbuiltin "APPLY" (fn &rest args) (lapply-fn fn (apply #'list* args)))
(defbuiltin "EVAL" (form &rest more) (leval form (if more (car more) *global-env*)))
(defbuiltin "EVLIS" (lst &rest more)
  (let ((env (if more (car more) *global-env*))) (mapcar (lambda (f) (leval f env)) lst)))
(defbuiltin "OPTIMIZE" (form)
  "The builtin constant-folder the Lisp-level optimizer passes hand off
to. A no-op here (identity): every earlier pass already preserves
semantics, so skipping the additional constant-fold is conservative, not
incorrect -- see sbcl/README.md."
  form)
(defbuiltin "MACROEXPAND" (form)
  (if (and (consp form) (symbolp (car form)))
      (let ((fn (ignore-errors (env-resolve *global-env* (car form)))))
        (if (macro-obj-p fn) (expand-macro-form-only fn (cdr form)) form))
      form))

(defun expand-macro-form-only (m arg-forms)
  "Like EXPAND-MACRO-CALL but for MACROEXPAND: returns the expansion
without evaluating it further."
  (let* ((menv (make-child-env (macro-obj-env m)))
         (dyn (bind-params menv (macro-obj-params m) (macro-obj-rest m) arg-forms "macro")))
    (with-dyn-pairs dyn (leval (macro-obj-body m) menv))))

(defvar *lamedh-gensym-counter* 0)
(defbuiltin "GENSYM" (&optional prefix)
  (declare (ignore prefix))
  (lsym (format nil "%GENSYM~D%" (incf *lamedh-gensym-counter*))))

(defbuiltin "INTERN" (name) (intern-lamedh name))

;;; ---- arithmetic ---------------------------------------------------------------

(defun numify (x) (if (characterp x) (char-code x) x))

(macrolet ((wrap (name fn) `(defbuiltin ,name (&rest args) (apply ,fn (mapcar #'numify args)))))
  (wrap "+" #'+) (wrap "-" #'-) (wrap "*" #'*)
  (wrap "PLUS" #'+) (wrap "TIMES" #'*)
  (wrap "=" #'=) (wrap "<" #'<) (wrap ">" #'>)
  (wrap "MAX" #'max) (wrap "MIN" #'min)
  (wrap "GCD" #'gcd) (wrap "LCM" #'lcm)
  (wrap "LESSP" #'<) (wrap "GREATERP" #'>))

(defun lamedh-divide (&rest args)
  "Integer / -- truncating (C/Rust-style integer division), not CL's exact
rational result -- when every argument is an integer; ordinary division
otherwise (a float argument, or any single-argument reciprocal)."
  (let ((args (mapcar #'numify args)))
    (if (and (every #'integerp args) (cdr args))
        (reduce (lambda (a b) (truncate a b)) args)
        (apply #'/ args))))
(defbuiltin "/" (&rest args) (apply #'lamedh-divide args))

(defbuiltin "DIFFERENCE" (a b) (- (numify a) (numify b)))
(defbuiltin "QUOTIENT" (a b) (lamedh-divide a b))
(defbuiltin "MOD" (a b) (mod (numify a) (numify b)))
(defbuiltin "REMAINDER" (a b) (rem (numify a) (numify b)))
(defbuiltin "EXPT" (a b) (expt (numify a) (numify b)))
(defbuiltin "ZEROP" (x) (bool (zerop (numify x))))
(defbuiltin "EVENP" (x) (bool (evenp (numify x))))
(defbuiltin "ODDP" (x) (bool (oddp (numify x))))
(defbuiltin "PLUSP" (x) (bool (plusp (numify x))))
(defbuiltin "ADD1" (x) (+ (numify x) 1))
(defbuiltin "SUB1" (x) (- (numify x) 1))
(env-set-local *global-env* (lsym "1+") (lambda (x) (+ (numify x) 1)))
(env-set-local *global-env* (lsym "1-") (lambda (x) (- (numify x) 1)))
(defbuiltin "SQRT" (x) (sqrt (coerce x 'double-float)))
(defbuiltin "ISQRT" (x) (isqrt x))
(defbuiltin "SIN" (x) (sin (coerce x 'double-float)))
(defbuiltin "COS" (x) (cos (coerce x 'double-float)))
(defbuiltin "TAN" (x) (tan (coerce x 'double-float)))
(defbuiltin "LOG" (x &optional base) (if base (log (coerce x 'double-float) (coerce base 'double-float)) (log (coerce x 'double-float))))
(defbuiltin "EXP" (x) (exp (coerce x 'double-float)))
(defbuiltin "FLOOR" (x &optional (y 1)) (values (floor x y)))
(defbuiltin "CEILING" (x &optional (y 1)) (values (ceiling x y)))
(defbuiltin "ROUND" (x &optional (y 1))
  "Round half AWAY FROM ZERO (C/Rust f64::round convention), not CL's
round-half-to-even."
  (let ((q (/ x y)))
    (if (minusp q) (- (floor (+ (- q) 1/2))) (floor (+ q 1/2)))))
(defbuiltin "TRUNCATE" (x &optional (y 1)) (values (truncate x y)))
(defbuiltin "SIGNUM" (x) (let ((s (signum x))) (if (floatp x) s (truncate s))))
(defbuiltin "FLOAT" (x) (coerce x 'double-float))

;;; ---- strings ------------------------------------------------------------------

(defun ->str (x)
  (cond ((stringp x) x) ((characterp x) (string x))
        (t (lamedh-error (format nil "expected a string or char, got ~A" (lprint-to-string x))))))

(defbuiltin "CONCAT" (&rest args) (apply #'concatenate 'string (mapcar #'->str args)))
(defbuiltin "STRING-LENGTH*" (s) (length (->str s)))
(defbuiltin "SUBSTRING" (s start &optional end) (subseq (->str s) start end))
(defbuiltin "CHAR-CODE" (c) (char-code (if (characterp c) c (char (->str c) 0))))
(defbuiltin "CODE-CHAR" (n) (string (code-char (numify n))))
(defbuiltin "MAKE-CHAR" (n) (code-char (numify n)))
(defbuiltin "STRING->NUMBER" (s)
  (let ((trimmed (string-trim '(#\Space #\Tab #\Newline #\Return) s)))
    (if (zerop (length trimmed))
        nil
        (let ((c (make-cursor trimmed)))
          (multiple-value-bind (v ok) (try-read-number c)
            (if (and ok (cur-eof-p c)) v nil))))))
(defbuiltin "NUMBER->STRING" (n) (lprint-to-string n nil))
(defbuiltin "STRING-CASEFOLD*" (s) (string-downcase (->str s)))
(defbuiltin "PRIN1-TO-STRING" (x) (lprint-to-string x t))
(defbuiltin "PRINC-TO-STRING" (x) (lprint-to-string x nil))

;;; ---- I/O ------------------------------------------------------------------------

(defbuiltin "PRINT" (x) (lprint x))
(defbuiltin "PRINC" (x) (lprinc x))
(defbuiltin "PRIN1" (x) (lprin1 x))
(defbuiltin "TERPRI" () (terpri) nil)
(defbuiltin "READ-FROM-STRING" (s) (lread s))

;;; ---- errors ------------------------------------------------------------------

(defbuiltin "ERROR" (msg &optional data)
  (if (lamedh-error-obj-p msg)
      (error 'lamedh-condition :value msg)
      (lamedh-error (->str msg) data)))
(defbuiltin "MAKE-ERROR" (msg &optional data) (make-lamedh-error-obj :message msg :data data))
(defbuiltin "ERROR-P" (x) (bool (lamedh-error-obj-p x)))
(defbuiltin "ERROR-MESSAGE" (x) (if (lamedh-error-obj-p x) (lamedh-error-obj-message x) (lprint-to-string x nil)))
(defbuiltin "ERROR-DATA" (x) (if (lamedh-error-obj-p x) (lamedh-error-obj-data x) nil))

;;; ---- sort -----------------------------------------------------------------------

(defbuiltin "SORT" (lst pred) (sort (copy-list lst) (lambda (a b) (lamedh-truthy-p (lapply-fn pred (list a b))))))

;;; ---- hash tables --------------------------------------------------------------

(defbuiltin "MAKE-HASH-TABLE" () (make-hash-table :test 'equal))
(defbuiltin "GETHASH" (table key) (gethash key table))
(defbuiltin "SET-BANG" (table key val) (setf (gethash key table) val))
(defbuiltin "SETHASH" (table key val) (setf (gethash key table) val))
(defbuiltin "REMHASH" (table key) (remhash key table) nil)
(defbuiltin "KEYS" (table) (loop for k being the hash-keys of table collect k))

;;; ---- arrays ---------------------------------------------------------------------

(defbuiltin "ARRAY" (n) (make-array n :initial-element nil))
(defbuiltin "FETCH" (arr i)
  (if (and (>= i 0) (< i (length arr))) (aref arr i)
      (lamedh-error (format nil "FETCH: index ~D out of bounds for array of length ~D" i (length arr)))))
(defbuiltin "STORE" (arr i val)
  (if (and (>= i 0) (< i (length arr)))
      (setf (aref arr i) val)
      (lamedh-error (format nil "STORE: index ~D out of bounds for array of length ~D" i (length arr)))))
(defbuiltin "ARRAY-LENGTH*" (arr) (length arr))
(defbuiltin "AREF" (arr i)
  (if (and (>= i 0) (< i (length arr))) (aref arr i)
      (lamedh-error (format nil "AREF: index ~D out of bounds for array of length ~D" i (length arr)))))
(defbuiltin "ASET" (arr i val)
  (if (and (>= i 0) (< i (length arr)))
      (setf (aref arr i) val)
      (lamedh-error (format nil "ASET: index ~D out of bounds for array of length ~D" i (length arr)))))
(defbuiltin "$ARRAY->LIST" (arr) (coerce arr 'list))
(defbuiltin "$LIST->ARRAY" (lst) (coerce lst 'simple-vector))

;;; ---- minimal module system stub (REQUIRE/PROVIDE/DEFMODULE) --------------------
;;;
;;; The reference implementation's module system (lib/06-require.lisp,
;;; lib/27-modules.lisp) does dynamic file loading and namespacing this
;;; port does not need: every bootstrap file it ships is loaded unconditionally
;;; at startup (see sbcl/src/bootstrap.lisp), so REQUIRE/PROVIDE/DEFMODULE only
;;; need to not signal an error when the *files that are actually loaded*
;;; use them for documentation/registration purposes.

(defvar *provided-modules* (make-hash-table :test 'eq))
(defbuiltin "PROVIDE" (name) (setf (gethash name *provided-modules*) t))
(defbuiltin "REQUIRE" (name) (bool (gethash name *provided-modules*)))
