;;;; runtime.lisp -- environment, evaluator, and builtins for the SBCL port
;;;; of Lamedh.
;;;;
;;;; This is a from-scratch, standalone implementation: it does not embed or
;;;; call into the Rust `lamedh` crate. It is written to be *semantically*
;;;; conformant with the reference implementation (docs/lamedh-manual.md,
;;;; src/evaluator/*.rs) for the Lisp-1.5-plus-modern-extensions core: lexical
;;;; closures, macros, fexprs, Kernel-style vau operatives, dynamic (special)
;;;; variables, PROG/GO/RETURN, and the standard special-form set. See
;;;; sbcl/README.md for the precise scope and the deliberate deviations.

(in-package #:lamedh-rt)

;;; ============================================================================
;;; Environments
;;; ============================================================================
;;;
;;; A Lamedh environment is a chain of lexical frames terminating at a single
;;; root (global) frame. Non-dynamic bindings live in a frame's hash table
;;; (keyed by the LAMEDH symbol, EQ-compared -- symbols are interned once, so
;;; EQ is the right test and matches Lamedh's own EQ semantics for symbols).
;;; The root frame's "table" is unused; global bindings for non-dynamic
;;; symbols are also just written into a table -- the *global* table, kept
;;; distinct from SYMBOL-VALUE so that ordinary (non-DEFDYNAMIC) globals do
;;; not require every Lamedh symbol to be a CL special variable.
;;;
;;; A symbol marked dynamic via DEFDYNAMIC/DEFVAR is bound with CL's own
;;; PROGV, and read/written via SYMBOL-VALUE. This reuses SBCL's special
;;; variable machinery (shallow binding, correct unwind on non-local exit)
;;; instead of reimplementing it.

(defstruct (lenv (:constructor %make-lenv (parent rootp)))
  (table (make-hash-table :test 'eq) :type hash-table)
  parent
  (rootp nil))

(defvar *global-table* (make-hash-table :test 'eq))
(defvar *dynamic-syms* (make-hash-table :test 'eq))
(defvar *global-env* (%make-lenv nil t)
  "The single global (root) environment. The reference implementation
supports multiple isolated interpreter \"worlds\" via MAKE-ENVIRONMENT;
this port targets one process-wide global environment, which is what the
CLI, the REPL, and every bootstrap/test file need.")

(defun make-global-environment () *global-env*)

(declaim (inline dynamic-sym-p))
(defun dynamic-sym-p (sym) (gethash sym *dynamic-syms*))

(defun mark-dynamic (sym) (setf (gethash sym *dynamic-syms*) t))

(define-condition lamedh-unbound-variable (error)
  ((name :initarg :name :reader lamedh-unbound-variable-name))
  (:report (lambda (c s) (format s "Unbound variable: ~A" (lamedh-unbound-variable-name c)))))

(defun env-boundp (env sym)
  (if (dynamic-sym-p sym)
      (boundp sym)
      (loop for e = env then (lenv-parent e)
            while e
            do (when (lenv-rootp e)
                 (return-from env-boundp (nth-value 1 (gethash sym *global-table*))))
               (multiple-value-bind (v present) (gethash sym (lenv-table e))
                 (declare (ignore v))
                 (when present (return-from env-boundp t)))
            finally (return nil))))

(defun env-resolve (env sym)
  "Resolve SYM's value starting from ENV. Signals LAMEDH-UNBOUND-VARIABLE if
unbound."
  (if (dynamic-sym-p sym)
      (if (boundp sym)
          (symbol-value sym)
          (error 'lamedh-unbound-variable :name sym))
      (loop for e = env then (lenv-parent e)
            while e
            do (if (lenv-rootp e)
                   (multiple-value-bind (v present) (gethash sym *global-table*)
                     (if present (return v) (error 'lamedh-unbound-variable :name sym)))
                   (multiple-value-bind (v present) (gethash sym (lenv-table e))
                     (when present (return v))))
            finally (error 'lamedh-unbound-variable :name sym))))

(defun env-set-local (env sym val)
  "Bind SYM to VAL in ENV's own frame (the root frame's global table if ENV
is the root). Used by DEF, lambda/fexpr/macro/vau parameter binding, and
LET/LET* bindings for non-dynamic variables."
  (if (lenv-rootp env)
      (setf (gethash sym *global-table*) val)
      (setf (gethash sym (lenv-table env)) val))
  val)

(defun env-update (env sym val)
  "SETQ semantics: mutate the nearest existing binding for SYM, or create
one in the global table if none exists anywhere in the chain (matching the
reference implementation's SETQ, which is intentionally permissive)."
  (when (dynamic-sym-p sym)
    (if (boundp sym)
        (return-from env-update (setf (symbol-value sym) val))
        (progn (setf (symbol-value sym) val) (return-from env-update val))))
  (loop for e = env then (lenv-parent e)
        while e
        do (if (lenv-rootp e)
               (return-from env-update (setf (gethash sym *global-table*) val))
               (multiple-value-bind (v present) (gethash sym (lenv-table e))
                 (declare (ignore v))
                 (when present (return-from env-update (setf (gethash sym (lenv-table e)) val))))))
  (setf (gethash sym *global-table*) val))

(defun make-child-env (parent) (%make-lenv parent nil))

;;; ============================================================================
;;; Callable value types
;;; ============================================================================

(defstruct lambda-obj params rest env body name)
(defstruct macro-obj params rest env body)
(defstruct fexpr-obj params env body)
(defstruct vau-obj operands-sym env-sym env body)

(defun callable-p (v)
  (or (functionp v) (lambda-obj-p v) (macro-obj-p v) (fexpr-obj-p v) (vau-obj-p v)))

;;; ============================================================================
;;; Errors / conditions
;;; ============================================================================

(defstruct lamedh-error-obj message data)

(define-condition lamedh-condition (error)
  ((value :initarg :value :reader lamedh-condition-value))
  (:report (lambda (c s) (format s "~A" (lamedh-condition-value-string c)))))

(defun lamedh-condition-value-string (c)
  (let ((v (lamedh-condition-value c)))
    (if (lamedh-error-obj-p v) (lamedh-error-obj-message v) (lprint-to-string v))))

(defun lamedh-error (msg &optional data)
  "Signal a Lamedh-visible error carrying MSG (a string) and optional DATA."
  (error 'lamedh-condition :value (make-lamedh-error-obj :message msg :data data)))

;;; ============================================================================
;;; Reader-symbol helpers
;;; ============================================================================

(defun lsym (name) (intern-lamedh name))
(defvar *t-sym* (lsym "T"))

(defun lamedh-truthy-p (v) (not (null v)))

(defvar *current-env* nil
  "The Lamedh environment a native builtin is currently being called from
-- bound around every builtin invocation in LEVAL's application branch so
that env-sensitive primitives (BOUNDP) can see the caller's lexical scope,
matching the reference implementation's builtins, which all receive the
calling environment explicitly.")

(defun self-evaluating-symbol-p (sym)
  "T when SYM (already known to be a symbol, not NIL) evaluates to itself:
the truth constant T, or a keyword-style symbol (its print name starts
with ':')."
  (or (eq sym *t-sym*)
      (char= (char (symbol-name sym) 0) #\:)))

;;; ---- property lists -----------------------------------------------------------
;;;
;;; Lamedh symbol property lists (GETP/PUTP/REMPROP/PLIST, and the
;;; "docstring" indicator DEF/DEFEXPR/DEFMACRO/DEFDYNAMIC install). Each
;;; symbol's plist is an alist of (indicator . value), indicators compared
;;; under EQUAL (they are ordinarily strings), matching the reference
;;; implementation's HashMap<String, LispVal>-per-symbol plist.

(defvar *plists* (make-hash-table :test 'eq))

(defun getp (sym key) (cdr (assoc key (gethash sym *plists*) :test #'equal)))
(defun putp (sym key val)
  (let* ((alist (gethash sym *plists*)) (cell (assoc key alist :test #'equal)))
    (if cell (setf (cdr cell) val) (setf (gethash sym *plists*) (append alist (list (cons key val)))))
    val))
(defun remprop* (sym key)
  (setf (gethash sym *plists*) (remove key (gethash sym *plists*) :key #'car :test #'equal))
  sym)
(defun plist-flat (sym) (loop for (k . v) in (gethash sym *plists*) append (list k v)))

;;; ============================================================================
;;; Special form registry
;;; ============================================================================

(defvar *special-forms* (make-hash-table :test 'eq))

(defmacro defspecial (name (args-var env-var whole-var) &body body)
  "Register a special-form handler under NAME (a string). The handler
returns (VALUES :done RESULT) for a final value, or (VALUES :tail NEW-FORM
NEW-ENV) to continue the trampoline in tail position."
  (let ((fn-name (intern (format nil "SF-~A" (substitute #\- #\* (string-upcase name))))))
    `(progn
       (defun ,fn-name (,args-var ,env-var ,whole-var)
         (declare (ignorable ,args-var ,env-var ,whole-var))
         ,@body)
       (setf (gethash (lsym ,name) *special-forms*) #',fn-name))))

(defun done (v) (values :done v))
(defun tail (form env) (values :tail form env))

;;; ---- helpers shared by several special forms --------------------------------

(defun progn-eval (forms env)
  "Evaluate FORMS (a CL list of Lamedh forms) in sequence, non-tail; return
the value of the last, or NIL for an empty list."
  (let ((result nil))
    (dolist (f forms result) (setf result (leval f env)))))

(defun wrap-progn (forms)
  (cond ((null forms) nil)
        ((null (cdr forms)) (car forms))
        (t (cons (lsym "PROGN") forms))))

(defun parse-param-list (params what)
  "Split a lambda/macro-style parameter list into (VALUES FIXED REST), where
FIXED is a list of symbols and REST is a symbol or NIL. Accepts both
(a b &rest r) and the dotted-tail shorthand (a b . r)."
  (let (fixed cur)
    (setf cur params)
    (loop
      (cond
        ((null cur) (return (values (nreverse fixed) nil)))
        ((symbolp cur) (return (values (nreverse fixed) cur)))
        ((eq (car cur) (lsym "&REST"))
         (unless (and (consp (cdr cur)) (symbolp (cadr cur)) (null (cddr cur)))
           (lamedh-error (format nil "~A: &rest must be followed by exactly one symbol" what)))
         (return (values (nreverse fixed) (cadr cur))))
        (t (push (car cur) fixed) (setf cur (cdr cur)))))))

(defun bind-params (env fixed rest args what)
  "Destructively bind FIXED/REST parameters to already-evaluated (or, for
macros/fexprs, unevaluated) ARGS into ENV. Handles dynamic parameters via
PROGV by returning the list of (sym . val) pairs that must be dynamically
bound; the caller wraps the body evaluation in PROGV for those."
  (let (dyn-pairs (remaining args))
    (dolist (p fixed)
      (when (null remaining)
        (lamedh-error (format nil "~A: too few arguments" what)))
      (if (dynamic-sym-p p)
          (push (cons p (car remaining)) dyn-pairs)
          (env-set-local env p (car remaining)))
      (setf remaining (cdr remaining)))
    (if rest
        (if (dynamic-sym-p rest)
            (push (cons rest remaining) dyn-pairs)
            (env-set-local env rest remaining))
        (when remaining (lamedh-error (format nil "~A: too many arguments" what))))
    (nreverse dyn-pairs)))

(defmacro with-dyn-pairs (pairs-var &body body)
  `(if (null ,pairs-var)
       (progn ,@body)
       (progv (mapcar #'car ,pairs-var) (mapcar #'cdr ,pairs-var) ,@body)))

;;; ---- QUOTE / QUASIQUOTE ------------------------------------------------------

(defspecial "QUOTE" (args env whole) (declare (ignore env whole)) (done (car args)))

(defun qq-expand (form env depth)
  (cond
    ((not (consp form)) form)
    ((eq (car form) (lsym "UNQUOTE"))
     (if (= depth 1)
         (leval (cadr form) env)
         (list (lsym "UNQUOTE") (qq-expand (cadr form) env (1- depth)))))
    ((eq (car form) (lsym "QUASIQUOTE"))
     (list (lsym "QUASIQUOTE") (qq-expand (cadr form) env (1+ depth))))
    ((and (consp (car form)) (eq (caar form) (lsym "UNQUOTE-SPLICING")) (= depth 1))
     (append (leval (cadr (car form)) env) (qq-expand (cdr form) env depth)))
    (t (cons (qq-expand (car form) env depth) (qq-expand (cdr form) env depth)))))

(defspecial "QUASIQUOTE" (args env whole) (declare (ignore whole)) (done (qq-expand (car args) env 1)))

;;; ---- COND / IF / AND / OR ----------------------------------------------------

(defspecial "IF" (args env whole)
  (declare (ignore whole))
  (destructuring-bind (test then &optional (else nil else-p)) args
    (if (lamedh-truthy-p (leval test env))
        (tail then env)
        (if else-p (tail else env) (done nil)))))

(defspecial "COND" (args env whole)
  (declare (ignore whole))
  (dolist (clause args (done nil))
    (let ((test (leval (car clause) env)))
      (when (lamedh-truthy-p test)
        (return-from sf-cond
          (if (null (cdr clause))
              (done test)
              (tail (wrap-progn (cdr clause)) env)))))))

(defspecial "AND" (args env whole)
  (declare (ignore whole))
  (if (null args)
      (done *t-sym*)
      (loop
        (when (null (cdr args)) (return (tail (car args) env)))
        (unless (lamedh-truthy-p (leval (car args) env)) (return (done nil)))
        (setf args (cdr args)))))

(defspecial "OR" (args env whole)
  (declare (ignore whole))
  (if (null args)
      (done nil)
      (loop
        (when (null (cdr args)) (return (tail (car args) env)))
        (let ((v (leval (car args) env)))
          (when (lamedh-truthy-p v) (return (done v))))
        (setf args (cdr args)))))

(defspecial "PROGN" (args env whole)
  (declare (ignore whole))
  (if (null args)
      (done nil)
      (progn (dolist (f (butlast args)) (leval f env))
             (tail (car (last args)) env))))

;;; ---- SETQ / DEF / DEFDYNAMIC --------------------------------------------------

(defspecial "SETQ" (args env whole)
  (declare (ignore whole))
  (let ((result nil))
    (loop while args do
      (let ((var (pop args)) (val-form (pop args)))
        (setf result (env-update env var (leval val-form env)))))
    (done result)))

(defspecial "DEF" (args env whole)
  (declare (ignore whole))
  (destructuring-bind (name val-form &optional doc) args
    (let ((v (leval val-form env)))
      (when doc (putp name "docstring" doc))
      (env-set-local env name v)
      (done name))))

(defspecial "DEFDYNAMIC" (args env whole)
  (declare (ignore whole))
  (destructuring-bind (name val-form &optional doc) args
    (mark-dynamic name)
    (setf (symbol-value name) (leval val-form env))
    (when doc (putp name "docstring" doc))
    (done name)))
(setf (gethash (lsym "DEFVAR") *special-forms*) (gethash (lsym "DEFDYNAMIC") *special-forms*))

;;; ---- LAMBDA / FUNCTION / LABEL ------------------------------------------------

(defspecial "LAMBDA" (args env whole)
  (declare (ignore whole))
  (multiple-value-bind (fixed rest) (parse-param-list (car args) "lambda")
    (done (make-lambda-obj :params fixed :rest rest :env env :body (wrap-progn (cdr args))))))

(defspecial "FUNCTION" (args env whole)
  (declare (ignore whole))
  (let ((form (car args)))
    (if (and (consp form) (eq (car form) (lsym "LAMBDA")))
        (multiple-value-call #'sf-lambda (cdr form) env nil)
        (if (symbolp form)
            (let ((v (env-resolve env form)))
              (if (callable-p v) (done v)
                  (lamedh-error (format nil "Symbol '~A' is not bound to a function" form))))
            (lamedh-error "FUNCTION argument must be a LAMBDA expression or a symbol")))))

(defspecial "LABEL" (args env whole)
  (declare (ignore whole))
  (destructuring-bind (name expr) args
    (unless (and (consp expr) (eq (car expr) (lsym "LAMBDA")))
      (lamedh-error "LABEL expression must be a LAMBDA expression"))
    (let ((new-env (make-child-env env)))
      (multiple-value-bind (kind func) (sf-lambda (cdr expr) new-env nil)
        (declare (ignore kind))
        (env-set-local new-env name func)
        (done func)))))

(defspecial "DEFINE" (args env whole)
  (declare (ignore whole))
  (let* ((raw (car args))
         (def-list (if (and (consp raw) (eq (car raw) (lsym "QUOTE"))) (leval raw env) raw))
         (names nil))
    (dolist (pair def-list)
      (let ((name (car pair)) (v (leval (cadr pair) env)))
        (env-set-local env name v)
        (push name names)))
    (done (nreverse names))))

;;; ---- DEFEXPR / DEFMACRO / anonymous MACRO / FEXPR / VAU ------------------------

(defun parse-macro-fexpr-args (args)
  "(name params [doc] body...) or (params [doc] body...) for the anonymous
constructors -- returns (values name params doc body)."
  (if (symbolp (car args))
      (destructuring-bind (name params &rest rest) args
        (if (and rest (stringp (car rest)))
            (values name params (car rest) (cdr rest))
            (values name params nil rest)))
      (destructuring-bind (params &rest rest) args
        (values nil params nil rest))))

(defspecial "DEFEXPR" (args env whole)
  (declare (ignore whole))
  (multiple-value-bind (name params doc body) (parse-macro-fexpr-args args)
    (multiple-value-bind (fixed rest) (parse-param-list params "fexpr")
      (when rest (lamedh-error "fexpr parameter list takes no &rest tail"))
      (let ((f (make-fexpr-obj :params fixed :env env :body (wrap-progn body))))
        (when doc (putp name "docstring" doc))
        (env-set-local env name f)
        (done name)))))

(defspecial "DEFMACRO" (args env whole)
  (declare (ignore whole))
  (multiple-value-bind (name params doc body) (parse-macro-fexpr-args args)
    (multiple-value-bind (fixed rest) (parse-param-list params "macro")
      (let ((m (make-macro-obj :params fixed :rest rest :env env :body (wrap-progn body))))
        (when doc (putp name "docstring" doc))
        (env-set-local env name m)
        (done name)))))

(defspecial "MACRO" (args env whole)
  (declare (ignore whole))
  (multiple-value-bind (fixed rest) (parse-param-list (car args) "macro")
    (done (make-macro-obj :params fixed :rest rest :env env :body (wrap-progn (cdr args))))))

(defspecial "FEXPR" (args env whole)
  (declare (ignore whole))
  (multiple-value-bind (fixed rest) (parse-param-list (car args) "fexpr")
    (when rest (lamedh-error "fexpr parameter list takes no &rest tail"))
    (done (make-fexpr-obj :params fixed :env env :body (wrap-progn (cdr args))))))

(defspecial "VAU" (args env whole)
  (declare (ignore whole))
  (destructuring-bind (ops-sym env-sym) (car args)
    (done (make-vau-obj :operands-sym ops-sym :env-sym env-sym :env env :body (wrap-progn (cdr args))))))
(setf (gethash (lsym "$VAU") *special-forms*) (gethash (lsym "VAU") *special-forms*))

(defvar *capability-mask* nil
  "NIL = unmasked (every capability effective, matching this port -- see
sbcl/README.md on sandboxing not being reproduced); otherwise a list of
capability-name strings, the intersection of every enclosing
WITH-CAPABILITIES fence.")

(defparameter *all-capability-names*
  '("READ-FS" "CREATE-FS" "TEMP-FS" "SHELL" "IO" "NET-DNS" "NET-CONNECT"
    "NET-LISTEN" "OS-ENV" "OS-ENV-WRITE" "OS-PROCESS" "OS-SIGNAL"))

(defun capability-mask-allows-p (name)
  (or (null *capability-mask*)
      (and (member (if (symbolp name) (symbol-name name) name) *capability-mask* :test #'string=) t)))

(defvar *kernel-fuel* nil
  "NIL when unarmed; otherwise a non-negative integer step budget, charged
once per LEVAL trampoline iteration -- the same unit WITH-FUEL/STEP-COUNT
(lib/22-guard.lisp, lib/26-instrument.lisp) measure and bound.")

(defspecial "WITH-FUEL" (args env whole)
  (declare (ignore whole))
  (destructuring-bind (n-form &rest body) args
    (let ((n (leval n-form env)) (prev *kernel-fuel*))
      (done (let ((*kernel-fuel* (if *kernel-fuel* (min n *kernel-fuel*) n)))
              (unwind-protect (progn-eval body env)
                (setf *kernel-fuel* prev)))))))

(defspecial "WITH-CAPABILITIES" (args env whole)
  (declare (ignore whole))
  (destructuring-bind (caps-form &rest body) args
    (let* ((requested (mapcar #'symbol-name (leval caps-form env)))
           (new-mask (if *capability-mask* (intersection *capability-mask* requested :test #'string=) requested)))
      (done (let ((*capability-mask* new-mask)) (progn-eval body env))))))

(defspecial "DEFSTRUCT-TYPED" (args env whole)
  "(DEFSTRUCT-TYPED name (field type)...) -- the native-record kernel
primitive DEFRECORD's compiled tier expands into. In this port every
record uses the same LAMEDH-STRUCT representation (see the \"Records\"
section above), so this just declares the schema and installs a
constructor, field accessors, and mutating setters -- each given a
DECLARED type scheme, exactly as the reference implementation's compiled
tier does."
  (declare (ignore whole))
  (destructuring-bind (name . field-specs) args
    (record-declare* name field-specs)
    (let ((ctor (lsym (concatenate 'string "MAKE-" (symbol-name name)))))
      (env-set-local env ctor (lambda (&rest vals) (apply #'record-new* name vals)))
      (declare-type!* ctor (list (lsym "->") (mapcar #'cadr field-specs) name)))
    (dolist (spec field-specs)
      (let* ((field (car spec))
             (getter (lsym (concatenate 'string (symbol-name name) "-" (symbol-name field))))
             (setter (lsym (concatenate 'string "SET-" (symbol-name name) "-" (symbol-name field) "!"))))
        (env-set-local env getter (lambda (self) (record-ref* self field)))
        (declare-type!* getter (list (lsym "->") (list name) (cadr spec)))
        (env-set-local env setter (lambda (self val) (record-set!* self field val)))))
    (done name)))

(defspecial "JIT-OPTIMIZE" (args env whole)
  "No-op in the SBCL port: no separate typed JIT exists here. Native
compilation happens uniformly via SBCL's own compiler once a function's
Lamedh-level type check passes -- see COMPILE-CHECKED-LAMBDA."
  (declare (ignore env whole))
  (done (car args)))

;;; ---- PROG / RETURN / GO / WHILE / FOR -----------------------------------------

(defspecial "PROG" (args env whole)
  (declare (ignore whole))
  (let* ((var-list (car args)) (body (coerce (cdr args) 'vector))
         (prog-env (make-child-env env))
         (labels (make-hash-table :test 'eq)))
    (dolist (v var-list) (env-set-local prog-env v nil))
    (dotimes (i (length body)) (when (symbolp (aref body i)) (setf (gethash (aref body i) labels) i)))
    (done
     (catch 'lamedh-prog-return
       (let ((pc 0))
         (loop
           (when (>= pc (length body)) (return nil))
           (let ((item (aref body pc)))
             (if (symbolp item)
                 (incf pc)
                 (let ((r (catch 'lamedh-prog-go (leval item prog-env) :lamedh-prog-next)))
                   (if (eq r :lamedh-prog-next)
                       (incf pc)
                       (let ((idx (gethash r labels)))
                         (if idx (setf pc idx) (lamedh-error (format nil "GO: label not found in PROG: ~A" r))))))))))))))

(defspecial "RETURN" (args env whole)
  (declare (ignore whole))
  (throw 'lamedh-prog-return (leval (car args) env)))

(defspecial "GO" (args env whole)
  (declare (ignore env whole))
  (throw 'lamedh-prog-go (car args)))

(defspecial "WHILE" (args env whole)
  (declare (ignore whole))
  (loop while (lamedh-truthy-p (leval (car args) env))
        do (dolist (f (cdr args)) (leval f env)))
  (done nil))

(defspecial "FOR" (args env whole)
  (declare (ignore whole))
  (destructuring-bind (spec &rest body) args
    (destructuring-bind (var start-form end-form &optional step-form) spec
      (let ((v (leval start-form env)) (end (leval end-form env))
            (step (if step-form (leval step-form env) 1))
            (for-env (make-child-env env)))
        (when (zerop step) (lamedh-error "for step must be non-zero"))
        (loop while (if (plusp step) (<= v end) (>= v end)) do
          (env-set-local for-env var v)
          (dolist (f body) (leval f for-env))
          (incf v step))
        (done nil)))))

;;; ---- LET / LET* -----------------------------------------------------------------

(defspecial "LET" (args env whole)
  (declare (ignore whole))
  (let* ((bindings (car args)) (body (wrap-progn (cdr args)))
         (let-env (make-child-env env))
         dyn-pairs)
    (dolist (b bindings)
      (let ((sym (car b)) (v (leval (cadr b) env)))
        (if (dynamic-sym-p sym) (push (cons sym v) dyn-pairs) (env-set-local let-env sym v))))
    (if dyn-pairs
        (done (progv (mapcar #'car dyn-pairs) (mapcar #'cdr dyn-pairs) (leval body let-env)))
        (tail body let-env))))

(defun let*-run (bindings let-env body)
  "Bind LET* bindings sequentially, each in the extent established by the
previous ones (so a later binding referencing an earlier DYNAMIC one sees
its shallow-bound value), then evaluate BODY."
  (if (null bindings)
      (leval body let-env)
      (destructuring-bind (sym val-form) (car bindings)
        (let ((v (leval val-form let-env)))
          (if (dynamic-sym-p sym)
              (progv (list sym) (list v) (let*-run (cdr bindings) let-env body))
              (progn (env-set-local let-env sym v) (let*-run (cdr bindings) let-env body)))))))

(defspecial "LET*" (args env whole)
  (declare (ignore whole))
  (let* ((bindings (car args)) (body (wrap-progn (cdr args))) (let-env (make-child-env env)))
    (if (some (lambda (b) (dynamic-sym-p (car b))) bindings)
        (done (let*-run bindings let-env body))
        (progn (dolist (b bindings) (env-set-local let-env (car b) (leval (cadr b) let-env)))
               (tail body let-env)))))

;;; ---- BLOCK / RETURN-FROM / CATCH / THROW / UNWIND-PROTECT / HANDLER-CASE -------

(defspecial "BLOCK" (args env whole)
  (declare (ignore whole))
  (done (catch (car args) (progn-eval (cdr args) env))))

(defspecial "RETURN-FROM" (args env whole)
  (declare (ignore whole))
  (throw (car args) (leval (cadr args) env)))

(defspecial "CATCH" (args env whole)
  (declare (ignore whole))
  (done (catch (leval (car args) env) (progn-eval (cdr args) env))))

(defspecial "THROW" (args env whole)
  (declare (ignore whole))
  (throw (leval (car args) env) (leval (cadr args) env)))

(defspecial "UNWIND-PROTECT" (args env whole)
  (declare (ignore whole))
  (done (unwind-protect (leval (car args) env) (dolist (f (cdr args)) (leval f env)))))

(defun lamedh-condition-lisp-value (c)
  (lamedh-condition-value c))

(defspecial "HANDLER-CASE" (args env whole)
  (declare (ignore whole))
  (let* ((protected (car args)) (clause (cadr args)))
    (unless (and clause (eq (car clause) (lsym "ERROR")))
      (lamedh-error "handler-case: only an (error (var) ...) clause is supported"))
    (let ((var (caadr clause)) (handler-body (cddr clause)))
      (done
       (handler-case (leval protected env)
         (lamedh-unbound-variable (c)
           (let ((henv (make-child-env env)))
             (env-set-local henv var
                             (make-lamedh-error-obj
                              :message (format nil "Unbound variable: ~A" (lamedh-unbound-variable-name c))))
             (progn-eval handler-body henv)))
         (lamedh-condition (c)
           (let ((henv (make-child-env env)))
             (env-set-local henv var (lamedh-condition-lisp-value c))
             (progn-eval handler-body henv)))
         (error (c)
           ;; Catch-all: any other CL condition of type ERROR that escapes a
           ;; builtin (division by zero, a wrong-type argument, ...) is
           ;; still catchable as a Lamedh error, carrying its report string.
           (let ((henv (make-child-env env)))
             (env-set-local henv var (make-lamedh-error-obj :message (princ-to-string c)))
             (progn-eval handler-body henv))))))))

;;; ============================================================================
;;; Records (DEFRECORD/DEFSTRUCT-TYPED's runtime representation)
;;; ============================================================================
;;;
;;; One runtime representation for every record, matching the reference
;;; implementation's StructObj: LAMEDH-STRUCT (defined in reader.lisp, ahead
;;; of the #S(...) literal reader) holds a brand (type-name symbol) and a
;;; values vector. A global schema registry maps a brand to its ordered
;;; field-name list so RECORD-REF/RECORD-WITH can resolve a field name to a
;;; vector index. This port has no separate "compiled tier": every record,
;;; whether DEFRECORD chose the compiled or dynamic tier in the reference
;;; implementation, is represented and accessed identically here -- see
;;; sbcl/README.md.

(defvar *record-schemas* (make-hash-table :test 'eq)
  "Brand (symbol) -> ordered list of field-name symbols.")

(defun record-declare* (name field-specs)
  (setf (gethash name *record-schemas*) (mapcar #'car field-specs))
  name)

(defun record-field-index (brand field)
  (or (position field (gethash brand *record-schemas*))
      (lamedh-error (format nil "record ~A has no field ~A" brand field))))

(defun record-new* (brand &rest values)
  (make-lamedh-struct :type-name brand :values (coerce values 'simple-vector)))

(defun record-ref* (self field)
  (unless (lamedh-struct-p self) (lamedh-error (format nil "RECORD-REF: not a record: ~A" (lprint-to-string self))))
  (svref (lamedh-struct-values self) (record-field-index (lamedh-struct-type-name self) field)))

(defun record-set!* (self field val)
  (setf (svref (lamedh-struct-values self) (record-field-index (lamedh-struct-type-name self) field)) val))

(defun record-with* (self &rest kvs)
  (let ((new (copy-seq (lamedh-struct-values self))) (brand (lamedh-struct-type-name self)))
    (loop for (k v) on kvs by #'cddr do (setf (svref new (record-field-index brand k)) v))
    (make-lamedh-struct :type-name brand :values new)))

(defun record-brand* (v) (and (lamedh-struct-p v) (lamedh-struct-type-name v)))
(defun record-fields* (v) (and (lamedh-struct-p v) (coerce (lamedh-struct-values v) 'list)))

;;; ============================================================================
;;; The (approximated) type-checker surface
;;; ============================================================================
;;;
;;; This port does not implement the reference implementation's HM type
;;; checker (src/check.rs): the declared-scheme axiom system it works
;;; alongside (DECLARE-TYPE!/SEE-TYPE) is preserved honestly instead of
;;; faked as fully verified -- see sbcl/README.md. DECLARE-TYPE! is exactly
;;; what its name says even in the reference implementation: an axiom
;;; trusted at call sites, not derived from the body, so reporting every
;;; declared symbol as DECLARED (never TYPED/CHECKED, which promise
;;; body-derived verification this port cannot perform) is accurate, not
;;; optimistic.

(defvar *declared-types* (make-hash-table :test 'eq))

(defun declare-type!* (name scheme) (setf (gethash name *declared-types*) scheme) name)

(defun see-type* (name)
  (let ((scheme (gethash name *declared-types*)))
    (if scheme
        (list (lsym "DECLARED") scheme)
        (list (lsym "DYNAMIC") "not statically checked in this port (no HM checker implemented)"))))

;;; ============================================================================
;;; The evaluator
;;; ============================================================================

(declaim (inline charge-kernel-fuel))
(defun charge-kernel-fuel ()
  (when *kernel-fuel*
    (if (<= *kernel-fuel* 0)
        (progn (setf *kernel-fuel* nil) (lamedh-error "fuel exhausted (kernel step budget)"))
        (decf *kernel-fuel*))))

(defun expand-macro-call (m arg-forms)
  (let* ((menv (make-child-env (macro-obj-env m)))
         (dyn (bind-params menv (macro-obj-params m) (macro-obj-rest m) arg-forms "macro")))
    (with-dyn-pairs dyn (leval (macro-obj-body m) menv))))

(defun leval (form env)
  (loop
    (charge-kernel-fuel)
    (cond
      ((null form) (return nil))
      ((symbolp form)
       (return (if (self-evaluating-symbol-p form) form (env-resolve env form))))
      ((not (consp form)) (return form))
      (t
       (let ((op (car form)) (rest (cdr form)))
         (let ((sf (and (symbolp op) (gethash op *special-forms*))))
           (if sf
               (multiple-value-bind (kind a b) (funcall sf rest env form)
                 (if (eq kind :tail)
                     (setf form a env b)
                     (return a)))
               (let ((fn (leval op env)))
                 (cond
                   ((macro-obj-p fn) (setf form (expand-macro-call fn rest)))
                   ((vau-obj-p fn)
                    (let ((new-env (make-child-env (vau-obj-env fn))))
                      (env-set-local new-env (vau-obj-operands-sym fn) rest)
                      (env-set-local new-env (vau-obj-env-sym fn) env)
                      (setf form (vau-obj-body fn) env new-env)))
                   ((fexpr-obj-p fn)
                    (let ((new-env (make-child-env (fexpr-obj-env fn)))
                          (params (fexpr-obj-params fn)))
                      (cond
                        ((and params (null (cdr params)))
                         (if (dynamic-sym-p (car params))
                             (return (progv (list (car params)) (list rest)
                                       (leval (fexpr-obj-body fn) new-env)))
                             (progn (env-set-local new-env (car params) rest)
                                    (setf form (fexpr-obj-body fn) env new-env))))
                        (t
                         (let ((dyn (bind-params new-env params nil rest "fexpr")))
                           (if dyn
                               (return (with-dyn-pairs dyn (leval (fexpr-obj-body fn) new-env)))
                               (setf form (fexpr-obj-body fn) env new-env)))))))
                   (t
                    (let ((eval-args (mapcar (lambda (a) (leval a env)) rest)))
                      (cond
                        ((lambda-obj-p fn)
                         (let ((new-env (make-child-env (lambda-obj-env fn))))
                           (let ((dyn (bind-params new-env (lambda-obj-params fn) (lambda-obj-rest fn)
                                                    eval-args "lambda")))
                             (if dyn
                                 (return (with-dyn-pairs dyn (leval (lambda-obj-body fn) new-env)))
                                 (setf form (lambda-obj-body fn) env new-env)))))
                        ((functionp fn) (return (let ((*current-env* env)) (apply fn eval-args))))
                        (t (lamedh-error
                            (format nil "not a function: ~A" (lprint-to-string fn))))))))))))))))
