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

(defun lresolve-fn (fn)
  "A symbol in function position (`(funcall 'car ...)`) names its binding in
the calling environment, resolved once -- matching the reference
implementation's FUNCALL/APPLY, including its `Function not found: NAME`
error when unbound. T is bound to itself there, so it resolves to T (and
then fails as not a function); NIL is the empty list, never a name."
  (cond
    ((or (null fn) (not (symbolp fn)) (eq fn *t-sym*)) fn)
    (t (let ((env (or *current-env* *global-env*)))
         (if (env-boundp env fn)
             (env-resolve env fn)
             (lamedh-error (format nil "Function not found: ~A" (symbol-name fn))))))))

(defun lapply-fn (fn args)
  "The one calling convention every FUNCALL/APPLY/HOF callback goes through."
  (setf fn (lresolve-fn fn))
  (cond
    ((functionp fn) (apply fn args))
    ((lambda-obj-p fn)
     (if (and (lambda-obj-compiled fn) (lc-arity-ok-p fn args))
         (funcall (lambda-obj-compiled fn) args)
         (let* ((env (make-child-env (lambda-obj-env fn)))
                (dyn (bind-params env (lambda-obj-params fn) (lambda-obj-rest fn) args "lambda")))
           (with-dyn-pairs dyn (leval (lambda-obj-body fn) env)))))
    ((fexpr-obj-p fn) (lamedh-error "cannot FUNCALL/APPLY a fexpr: it needs unevaluated operands"))
    ((macro-obj-p fn) (lamedh-error "cannot FUNCALL/APPLY a macro"))
    ((vau-obj-p fn) (lamedh-error "cannot FUNCALL/APPLY a vau"))
    (t (lamedh-error (format nil "not a function: ~A" (lprint-to-string fn))))))

(defun lamedh-deep-eq (a b)
  "Structural equality mirroring the reference implementation's derived
PartialEq for LispVal -- used for record/struct FIELDS (a Vec<LispVal>
there), which recurses through nested cons cells even though top-level
EQ itself never does (see LAMEDH-EQ)."
  (cond
    ((and (consp a) (consp b)) (and (lamedh-deep-eq (car a) (car b)) (lamedh-deep-eq (cdr a) (cdr b))))
    ((or (consp a) (consp b)) nil)
    ((and (lamedh-struct-p a) (lamedh-struct-p b)) (lamedh-struct-deep-eq a b))
    ((and (numberp a) (numberp b)) (eql a b))
    ((and (characterp a) (characterp b)) (char= a b))
    ((and (stringp a) (stringp b)) (string= a b))
    (t (eq a b))))

(defun lamedh-struct-deep-eq (a b)
  (and (eq (lamedh-struct-type-name a) (lamedh-struct-type-name b))
       (let ((va (lamedh-struct-values a)) (vb (lamedh-struct-values b)))
         (and (= (length va) (length vb)) (every #'lamedh-deep-eq va vb)))))

(defun lamedh-eq (a b)
  "Lamedh's EQ: identity for symbols/callables, but VALUE equality for the
immutable atomic types (numbers, characters, strings) and DEEP structural
equality for records/structs (recursing into every field, cons cells
included) -- matching the reference implementation's derived LispVal
PartialEq exactly, including its one asymmetry: a cons cell is never EQ
to anything, not even itself by identity (Lisp 1.5 manual: EQ is defined
only for atoms), while a Struct field that happens to hold a cons still
gets compared structurally as part of the struct's own deep equality."
  (cond
    ((and (numberp a) (numberp b)) (eql a b))
    ((and (characterp a) (characterp b)) (char= a b))
    ((and (stringp a) (stringp b)) (string= a b))
    ((or (consp a) (consp b)) nil)
    ((and (lamedh-struct-p a) (lamedh-struct-p b)) (lamedh-struct-deep-eq a b))
    (t (eq a b))))

(defun lamedh-equal (a b) (lamedh-truthy-p (lapply-fn (env-resolve *global-env* (lsym "EQUAL")) (list a b))))

;;; ---- core kernel: cons cells, identity, predicates --------------------------

(defbuiltin "CAR" (x) (if (consp x) (car x) (if (null x) nil (lamedh-error (format nil "CAR: not a list: ~A" (lprint-to-string x))))))
(defbuiltin "CDR" (x) (if (consp x) (cdr x) (if (null x) nil (lamedh-error (format nil "CDR: not a list: ~A" (lprint-to-string x))))))
(defbuiltin "CONS" (a b) (cons a b))
;; RPLACA/RPLACD are NON-mutating, as in the reference implementation
;; (#508): each returns a NEW cell sharing the untouched half, so no
;; circular list can be built and the original cell is left intact.
(defbuiltin "RPLACA" (x new-car)
  (if (consp x) (cons new-car (cdr x))
      (lamedh-error (format nil "RPLACA: expected a cons cell as its first argument, got ~A" (lprint-to-string x)))))
(defbuiltin "RPLACD" (x new-cdr)
  (if (consp x) (cons (car x) new-cdr)
      (lamedh-error (format nil "RPLACD: expected a cons cell as its first argument, got ~A" (lprint-to-string x)))))
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
(defbuiltin "ARRAYP" (x) (bool (lamedh-array-p x)))
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
(defbuiltin "THE-ENVIRONMENT" () (or *current-env* *global-env*))
(defbuiltin "CURRENT-ENVIRONMENT" () (or *current-env* *global-env*))
(defbuiltin "MAKE-ENVIRONMENT" (&optional parent)
  (cond
    ((null parent) (make-fresh-root-env))
    ((lenv-p parent) (make-child-env parent))
    (t (lamedh-error (format nil "MAKE-ENVIRONMENT: argument must be an environment, got ~A" (lprint-to-string parent))))))
(defbuiltin "ENVIRONMENT-P" (v) (bool (lenv-p v)))
(defbuiltin "FEATURES" () (sort (copy-list *enabled-features*) #'string<))
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

(defvar *lamedh-random-state* (make-random-state t)
  "A dedicated random state RANDOM-SEED! reseeds, kept separate from
CL:*RANDOM-STATE* so seeding Lamedh's RNG never perturbs unrelated CL
code sharing this Lisp image.")
(defbuiltin "RANDOM" (n)
  (unless (and (integerp n) (plusp n))
    (lamedh-error (format nil "RANDOM: expected a positive integer, got ~A" (lprint-to-string n))))
  (random n *lamedh-random-state*))
(defbuiltin "RANDOM-SEED!" (n)
  (unless (integerp n)
    (lamedh-error (format nil "RANDOM-SEED!: expected an integer, got ~A" (lprint-to-string n))))
  ;; Deterministic given the same seed (matching the reference implementation's
  ;; contract), but not bit-for-bit identical to its RNG -- callers rely on
  ;; statistical/structural properties (a shuffled list stays a permutation,
  ;; a Monte Carlo estimate converges), never on the exact sequence.
  (setf *lamedh-random-state* (sb-ext:seed-random-state n))
  n)
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
;; One-pass walks backing STRING->LIST, STRING-SPLIT and STRING-JOIN in
;; lib/14-strings.lisp (issue #510).
(defbuiltin "STRING->LIST*" (s) (map 'list #'string (->str s)))
(defbuiltin "STRING-SPLIT*" (s delim)
  (let ((s (->str s)) (delim (->str delim)))
    (if (zerop (length delim))
        (list s)
        (loop with m = (length delim)
              for start = 0 then (+ idx m)
              for idx = (search delim s :start2 start)
              collect (subseq s start idx)
              while idx))))
(defbuiltin "STRING-JOIN*" (strs sep)
  (let ((sep (->str sep)))
    (with-output-to-string (out)
      (loop for (x . more) on strs
            do (unless (stringp x)
                 (lamedh-error (format nil "STRING-JOIN*: expected a list of strings, got element ~A"
                                       (lprint-to-string x))))
               (write-string x out)
               (when more (write-string sep out))))))
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

(defconstant +max-array+ (* 16 1024 1024)
  "The reference implementation's MakeArray/MakeTypedArray size cap (16 M elements).")

(defun check-array-size (n what)
  (cond ((not (and (integerp n) (>= n 0)))
         (lamedh-error (format nil "~:@(~A~): size must be a non-negative integer, got ~A" what (lprint-to-string n))))
        ((> n +max-array+)
         (lamedh-error (format nil "~(~A~): size ~D exceeds maximum of ~D" what n +max-array+)))
        (t n)))

(defun make-lamedh-array (n) (make-array (check-array-size n "array") :initial-element nil))
(defbuiltin "ARRAY" (n) (make-lamedh-array n))
(defbuiltin "MAKE-ARRAY" (n) (make-lamedh-array n))

;;; Flat typed arrays, `(typed-array n 'int64|'float64)`: the reference
;;; implementation's zero-copy JIT membrane array (TypedArrayObj). Here they
;;; are SBCL's own specialized vectors, zero-initialized; FETCH/STORE/AREF/
;;; ASET/ARRAY-LENGTH*/ARRAYP/$ARRAY->LIST accept them alongside a plain array.
(deftype int64-array () '(simple-array (signed-byte 64) (*)))
(deftype float64-array () '(simple-array double-float (*)))
(defun typed-array-p (x) (or (typep x 'int64-array) (typep x 'float64-array)))
(defun lamedh-array-p (x) (or (simple-vector-p x) (typed-array-p x)))
(defun typed-array-elem-name (arr) (if (typep arr 'int64-array) "int64" "float64"))

(defbuiltin "TYPED-ARRAY" (n elem)
  (check-array-size n "typed-array")
  (cond ((not (and elem (symbolp elem)))
         (lamedh-error (format nil "TYPED-ARRAY: element type must be a symbol, got ~A" (lprint-to-string elem))))
        ((string= (symbol-name elem) "INT64") (make-array n :element-type '(signed-byte 64) :initial-element 0))
        ((string= (symbol-name elem) "FLOAT64") (make-array n :element-type 'double-float :initial-element 0d0))
        (t (lamedh-error (format nil "TYPED-ARRAY: unknown element type '~A, expected 'int64 or 'float64" (symbol-name elem))))))
(defbuiltin "TYPED-ARRAY-P" (x) (bool (typed-array-p x)))

(defun typed-array-store (arr i val)
  "TypedArrayObj::set: an int64 array takes only integers, a float64 array
floats or integers (widened); anything else is refused, never coerced."
  (let ((word (cond ((typep arr 'int64-array)
                     (and (integerp val) (typep val '(signed-byte 64)) val))
                    ((floatp val) (coerce val 'double-float))
                    ((integerp val) (coerce val 'double-float)))))
    (cond ((null word)
           (lamedh-error (format nil "typed array of ~A: cannot store ~A" (typed-array-elem-name arr) (lprint-to-string val))))
          ((not (and (integerp i) (>= i 0) (< i (length arr))))
           (lamedh-error (format nil "typed array: index ~A out of bounds (length ~D)" (lprint-to-string i) (length arr))))
          (t (setf (aref arr i) word) val))))

;;; ARRAY-SUM / ARRAY-DOT: int64 reductions wrap (two's complement); float64
;;; reductions follow Fortran's SUM -- a processor-dependent approximation
;;; with an unspecified order of additions (#392). The float shape below is
;;; the reference implementation's (f64_reduce_by: 8 strided lanes, a
;;; balanced combine, then the tail in order), so the two agree bit for bit
;;; today; that agreement is an implementation property, not the contract.

(defun wrap-int64 (n)
  (let ((m (ldb (byte 64 0) n))) (if (logbitp 63 m) (- m (ash 1 64)) m)))

(defun f64-reduce-by (n get)
  (sb-int:with-float-traps-masked (:overflow :invalid :inexact :divide-by-zero)
    (let* ((l (make-array 8 :element-type 'double-float :initial-element 0d0))
           (body (- n (mod n 8))))
      (loop for i from 0 below body by 8
            do (dotimes (j 8) (incf (aref l j) (funcall get (+ i j)))))
      (let ((acc (+ (+ (+ (aref l 0) (aref l 2)) (+ (aref l 4) (aref l 6)))
                    (+ (+ (aref l 1) (aref l 3)) (+ (aref l 5) (aref l 7))))))
        (loop for k from body below n do (incf acc (funcall get k)))
        acc))))

(defun reduce-operand (name v)
  "(VALUES elements :int|:float) -- the reference implementation's
reduce_operand: an all-integer plain array or an int64 typed array reduces
as int64; a plain array mixing integers and floats reduces as float64."
  (cond ((typep v 'int64-array) (values v :int))
        ((typep v 'float64-array) (values v :float))
        ((simple-vector-p v)
         (if (every #'integerp v)
             (values v :int)
             (values (map 'vector
                          (lambda (x)
                            (if (or (integerp x) (floatp x))
                                (coerce x 'double-float)
                                (lamedh-error (format nil "~A: elements must be int64 or float64, got ~A" name (lprint-to-string x)))))
                          v)
                     :float)))
        (t (lamedh-error (format nil "~A: argument must be an array, got ~A" name (lprint-to-string v))))))

(defbuiltin "ARRAY-SUM" (arr)
  (multiple-value-bind (v kind) (reduce-operand "array-sum" arr)
    (if (eq kind :int)
        (wrap-int64 (reduce #'+ v))
        (f64-reduce-by (length v) (lambda (i) (aref v i))))))

(defbuiltin "ARRAY-DOT" (a b)
  (multiple-value-bind (x kx) (reduce-operand "array-dot" a)
    (multiple-value-bind (y ky) (reduce-operand "array-dot" b)
      (let ((n (min (length x) (length y))))
        (if (and (eq kx :int) (eq ky :int))
            (wrap-int64 (loop for i below n sum (* (aref x i) (aref y i))))
            (f64-reduce-by n (lambda (i) (* (coerce (aref x i) 'double-float)
                                            (coerce (aref y i) 'double-float)))))))))
(defbuiltin "FETCH" (arr i)
  (if (and (>= i 0) (< i (length arr))) (aref arr i)
      (lamedh-error (format nil "FETCH: index ~D out of bounds for array of length ~D" i (length arr)))))
(defbuiltin "STORE" (arr i val)
  (cond
    ((typed-array-p arr) (typed-array-store arr i val))
    ((and (>= i 0) (< i (length arr)))
     (setf (aref arr i) val))
    (t (lamedh-error (format nil "STORE: index ~D out of bounds for array of length ~D" i (length arr))))))
(defbuiltin "ARRAY-LENGTH*" (arr) (length arr))
(defbuiltin "AREF" (arr i)
  (if (and (>= i 0) (< i (length arr))) (aref arr i)
      (lamedh-error (format nil "AREF: index ~D out of bounds for array of length ~D" i (length arr)))))
(defbuiltin "ASET" (arr i val)
  (cond
    ((typed-array-p arr) (typed-array-store arr i val))
    ((and (>= i 0) (< i (length arr)))
     (setf (aref arr i) val))
    (t (lamedh-error (format nil "ASET: index ~D out of bounds for array of length ~D" i (length arr))))))
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
