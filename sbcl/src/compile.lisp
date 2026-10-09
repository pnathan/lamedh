;;;; compile.lisp -- an ahead-of-time Lamedh -> SBCL compiler.
;;;;
;;;; Every LAMBDA/DEFUN creation (see runtime.lisp's LAMBDA special form)
;;;; attempts this compiler unconditionally -- there is no type-checker gate
;;;; to decide "when it is safe" (this port has none; see README's "The
;;;; type checker"), so the compiler itself decides, form by form, whether
;;;; it understands the body well enough to emit native code. On success,
;;;; the resulting SBCL-native closure becomes LAMBDA-OBJ's COMPILED slot;
;;;; LEVAL and LAPPLY-FN (builtins.lisp) call it directly, bypassing
;;;; tree-walking, whenever a call's argument count matches this lambda's
;;;; declared arity exactly (LC-ARITY-OK-P) -- an arity mismatch always
;;;; falls through to the ordinary BIND-PARAMS path, which raises the
;;;; correct Lamedh "too few/too many arguments" error. Anything the
;;;; compiler does not understand aborts the attempt (COMPILE-UNSUPPORTED,
;;;; caught by TRY-COMPILE-LAMBDA) and the lambda runs tree-walked exactly
;;;; as it always has: compiling is a pure execution-strategy optimization
;;;; here, never a semantic change, for every lambda this port can compile.
;;;;
;;;; Supported subset, applied recursively through the body:
;;;;   - self-evaluating literals (numbers, strings, characters) and
;;;;     self-evaluating symbols (T, keywords, ...)
;;;;   - QUOTE
;;;;   - IF / PROGN / AND / OR, with Lamedh's own truthiness (LAMEDH-TRUTHY-P)
;;;;     and short-circuit semantics (matching SF-AND/SF-OR exactly)
;;;;   - variable references: a lambda parameter compiles to a native CL
;;;;     lexical variable; any other symbol compiles to a fresh ENV-RESOLVE
;;;;     against the lambda's closed-over environment (the %ENV parameter
;;;;     of the compiled factory -- see "literals and generated names"
;;;;     below), so a later global redefinition is still seen -- nothing is
;;;;     baked in as a compile-time constant
;;;;   - calls (OP . ARGS), for any operator expression, but ONLY when OP is
;;;;     a symbol that resolves, AT THE MOMENT THIS LAMBDA IS BEING
;;;;     COMPILED, to a native CL function or another LAMBDA-OBJ (never a
;;;;     macro, fexpr, or vau, and never unbound)
;;;;
;;;; That last restriction is what makes the compiler safe without any
;;;; separate self-/mutual-recursion check: DEF and LABEL both bind a
;;;; function's own name only *after* its LAMBDA form has already been
;;;; evaluated (and hence already compiled), so a self-recursive or
;;;; forward-referencing DEFUN always finds its own name unbound at
;;;; compile time and falls back to the tree-walking interpreter -- which
;;;; is exactly where such a function needs to run anyway, since only
;;;; LEVAL's trampoline loop gives self-tail-calls the O(1)-Lamedh-stack
;;;; guarantee documented in runtime.lisp. Calls compile to LAPPLY-FN (the
;;;; same generic dispatch FUNCALL/APPLY and every higher-order stdlib
;;;; helper already go through), re-resolving OP at call time rather than
;;;; baking in the compile-time value, so an ordinary redefinition after
;;;; compilation is still honored correctly. The one narrow residual risk:
;;;; a name redefined *from* an ordinary function *to* a macro/fexpr/vau
;;;; after this lambda was compiled makes LAPPLY-FN signal a clear
;;;; "cannot FUNCALL/APPLY a macro"-style error instead of the
;;;; interpreter's correct macro-expansion behavior -- a loud failure, not
;;;; a silent wrong answer, for a genuinely pathological edit-after-use
;;;; sequence. Any other special form found in operator position (LET,
;;;; SETQ, WHILE, CATCH, a nested LAMBDA, ...) aborts the whole attempt.
;;;;
;;;; This is deliberately conservative, not exhaustive: a real, always-
;;;; attempted ahead-of-time compiler for the subset of Lamedh --
;;;; arithmetic/predicate/control-flow glue built on already-defined
;;;; functions -- where compiling changes nothing but execution strategy.
;;;; Recursive and forward-referencing functions, and anything using
;;;; LET/SETQ/loops/dynamic parameters/nested closures/condition handling,
;;;; remain fully correct via the tree-walking interpreter; they are
;;;; simply not (yet) native-compiled. Widening this subset -- e.g. adding
;;;; LET, or a self-tail-call loop rewrite that preserves O(1) Lamedh
;;;; stack for compiled-to-compiled recursion -- is future work, not a
;;;; correctness gap in what exists today.

(in-package #:lamedh-rt)

(define-condition compile-unsupported (error)
  ((reason :initarg :reason :reader compile-unsupported-reason))
  (:report (lambda (c s) (format s "Lamedh->CL compiler: ~A" (compile-unsupported-reason c)))))

(defun cbail (fmt &rest args)
  (error 'compile-unsupported :reason (apply #'format nil fmt args)))

(defvar *lc-quote* nil)
(defvar *lc-if* nil)
(defvar *lc-progn* nil)
(defvar *lc-and* nil)
(defvar *lc-or* nil)

(defun lc-init-syms ()
  (unless *lc-quote*
    (setf *lc-quote* (lsym "QUOTE") *lc-if* (lsym "IF") *lc-progn* (lsym "PROGN")
          *lc-and* (lsym "AND") *lc-or* (lsym "OR"))))

(defun lc-resolvable-callable-p (op env)
  "T if OP is, right now, bound in ENV to a native function or LAMBDA-OBJ
-- never a macro/fexpr/vau, never unbound."
  (handler-case
      (let ((v (env-resolve env op)))
        (or (functionp v) (lambda-obj-p v)))
    (error () nil)))

;;; ---- literals and generated names ------------------------------------------
;;;
;;; A compiled lambda is built as a FACTORY, (LAMBDA (%ENV %K0 %K1 ...)
;;; (LAMBDA (%ARGS) ...)), whose code depends only on the lambda's source:
;;; its closed-over environment and every literal object in its body that is
;;; not a symbol, number or character (strings, quoted lists, records, ...)
;;; are factory arguments, supplied when a closure is instantiated -- so each
;;; closure sees exactly the objects its own source holds, as before. Two
;;; lambdas with the same generated form share one compiled factory
;;; (*LC-FACTORIES*), so SBCL's COMPILE runs once per distinct lambda shape,
;;; not once per LAMBDA evaluation -- a LAMBDA inside a loop, or produced by
;;; a macro expansion, used to recompile on every iteration (#542). The
;;; same keying lets the stdlib's factories persist across processes in a
;;; fasl (see "The stdlib factory cache" below).

(defvar *lc-literals* nil "Literal objects lifted out of the form being compiled, in reverse order.")
(defvar *lc-literal-count* 0)
(defvar *lc-gensym-count* 0)

(defun lc-lift (obj)
  "Return the factory parameter that will hold OBJ in the generated code."
  (push obj *lc-literals*)
  (prog1 (intern (format nil "%K~D" *lc-literal-count*) '#:lamedh-rt)
    (incf *lc-literal-count*)))

(defun lc-gensym (prefix)
  "A generated local name that is the same on every compile of the same
source (unlike GENSYM), so equal lambdas generate EQUAL forms."
  (prog1 (intern (format nil "%~A~D" prefix *lc-gensym-count*) '#:lamedh-rt)
    (incf *lc-gensym-count*)))

(defun lc-inline-constant-p (x)
  "True for a literal that can appear in the generated code itself: a
number, a character, or an interned symbol (NIL included)."
  (or (numberp x) (characterp x) (and (symbolp x) (or (null x) (symbol-package x)))))

(defun lc-quote (x)
  (if (lc-inline-constant-p x) (list 'quote x) (lc-lift x)))

(defun lc-compile-and (args bound env)
  (cond
    ((null args) (list 'quote *t-sym*))
    ((null (cdr args)) (lc-compile-form (car args) bound env))
    (t (let ((v (lc-gensym "AND")))
         `(let ((,v ,(lc-compile-form (car args) bound env)))
            (if (lamedh-truthy-p ,v) ,(lc-compile-and (cdr args) bound env) nil))))))

(defun lc-compile-or (args bound env)
  (cond
    ((null args) nil)
    ((null (cdr args)) (lc-compile-form (car args) bound env))
    (t (let ((v (lc-gensym "OR")))
         `(let ((,v ,(lc-compile-form (car args) bound env)))
            (if (lamedh-truthy-p ,v) ,v ,(lc-compile-or (cdr args) bound env)))))))

(defun lc-compile-form (form bound env)
  "Compile FORM (a Lamedh s-expression) to a CL form, or signal
COMPILE-UNSUPPORTED. BOUND is the list of Lamedh symbols currently bound
as native CL lexical variables (this lambda's parameters); every other
symbol reference goes through %ENV, the factory parameter holding the
lambda's closed-over environment."
  (lc-init-syms)
  (cond
    ((null form) nil)
    ((numberp form) form)
    ((stringp form) (lc-lift form))
    ((characterp form) form)
    ((symbolp form)
     (cond
       ((self-evaluating-symbol-p form) (lc-quote form))
       ((member form bound) form)
       (t (list 'env-resolve '%env (lc-quote form)))))
    ((not (consp form)) (cbail "form ~S is neither a literal nor a cons" form))
    (t
     (let ((op (car form)) (args (cdr form)))
       (cond
         ((eq op *lc-quote*) (lc-quote (car args)))
         ((eq op *lc-if*)
          ;; KERNEL: exactly three operands. Anything else is left to the
          ;; evaluator, which raises the arity error when the body runs.
          (unless (and (consp args) (consp (cdr args)) (consp (cddr args)) (null (cdddr args)))
            (cbail "IF takes exactly three operands"))
          (destructuring-bind (c th el) args
            (list 'if (list 'lamedh-truthy-p (lc-compile-form c bound env))
                  (lc-compile-form th bound env)
                  (lc-compile-form el bound env))))
         ((eq op *lc-progn*) (cons 'progn (mapcar (lambda (f) (lc-compile-form f bound env)) args)))
         ((eq op *lc-and*) (lc-compile-and args bound env))
         ((eq op *lc-or*) (lc-compile-or args bound env))
         ((and (symbolp op) (gethash op *special-forms*))
          (cbail "special form ~A is not part of the compiled subset" op))
         (t
          (unless (symbolp op) (cbail "non-symbol operator ~S is not supported" op))
          (unless (lc-resolvable-callable-p op env)
            (cbail "~A is not already bound to an ordinary function" op))
          (list 'lapply-fn (lc-compile-form op bound env)
                (cons 'list (mapcar (lambda (a) (lc-compile-form a bound env)) args)))))))))

;;; ---- the factory table --------------------------------------------------------

;;; Factories live in two tables. The stdlib's -- those compiled or looked
;;; up while it bootstraps, and those its cache fasl registers -- are pinned
;;; in *LC-STDLIB-FACTORIES*: a fixed set, bounded by the stdlib's source.
;;; Every other factory goes to *LC-FACTORIES*, which is bounded: numeric
;;; literals are compiled inline and are part of a factory's form, so code
;;; that EVALs lambdas built with fresh numbers (or fresh symbols) makes a
;;; new shape each time. When that table reaches *LC-FACTORY-LIMIT* forms
;;; it is flushed whole; a flushed shape is simply recompiled on its next
;;; LAMBDA, and closures already made keep their compiled code.

(defvar *lc-stdlib-factories* (make-hash-table :test 'eql :synchronized t)
  "Structural hash of a pinned stdlib factory form -> list of (FORM . COMPILED-FACTORY).")

(defvar *lc-factories* (make-hash-table :test 'eql :synchronized t)
  "Structural hash of a user factory form -> list of (FORM . COMPILED-FACTORY).
Holds at most *LC-FACTORY-LIMIT* forms.")

(defparameter *lc-factory-limit* 4096
  "Most user factory forms *LC-FACTORIES* holds before it is flushed.")

(defvar *lc-factory-count* 0 "Forms currently in *LC-FACTORIES*.")

(defvar *lc-recording* nil
  "True while the stdlib bootstraps: factories compiled then are the ones
the stdlib factory cache persists.")
(defvar *lc-recorded* nil
  "Persistable (FORM . FACTORY) pairs used while *LC-RECORDING*, newest first.")
(defvar *lc-recorded-set* (make-hash-table :test 'eq) "Factories already in *LC-RECORDED*.")
(defvar *lc-compiled-while-recording* nil
  "True once a factory had to be compiled while *LC-RECORDING* -- the
cache (if any) did not cover this bootstrap and is worth rewriting.")

(defun lc-form-hash (form)
  (let ((h 0))
    (declare (type fixnum h))
    (labels ((walk (x)
               (if (consp x)
                   (progn (setf h (sb-int:mix h 7)) (walk (car x)) (walk (cdr x)))
                   (setf h (sb-int:mix h (sxhash x))))))
      (walk form))
    h))

(defun lc-find-factory (form hash)
  (cdr (or (assoc form (gethash hash *lc-stdlib-factories*) :test #'equal)
           (assoc form (gethash hash *lc-factories*) :test #'equal))))

(defun lc-add-factory (form hash factory &optional (pinned *lc-recording*))
  "Record FACTORY as FORM's compiled factory: pinned (never evicted) for
the stdlib, otherwise in the bounded user table."
  (if pinned
      (sb-ext:with-locked-hash-table (*lc-stdlib-factories*)
        (push (cons form factory) (gethash hash *lc-stdlib-factories*)))
      (sb-ext:with-locked-hash-table (*lc-factories*)
        (when (>= *lc-factory-count* *lc-factory-limit*)
          (clrhash *lc-factories*)
          (setf *lc-factory-count* 0))
        (incf *lc-factory-count*)
        (push (cons form factory) (gethash hash *lc-factories*)))))

(defun lc-persistable-p (form)
  "True if FORM holds only atoms a fasl can reproduce by value: numbers
(finite floats only, as infinities and NaNs have no readable syntax),
characters, and interned symbols."
  (labels ((ok (x)
             (cond ((consp x) (and (ok (car x)) (ok (cdr x))))
                   ((floatp x) (not (or (sb-ext:float-infinity-p x) (sb-ext:float-nan-p x))))
                   (t (lc-inline-constant-p x)))))
    (ok form)))

(defun lc-compile-factory (form)
  (handler-bind ((warning #'muffle-warning))
    (compile nil form)))

(defun try-compile-lambda (fixed rest body env)
  "Attempt to ahead-of-time compile a LAMBDA/DEFUN body. On success,
returns a native CL function of one argument (the already-evaluated
argument list, fixed params first then the rest-list if any). Returns NIL
if BODY uses anything outside this file's supported subset -- the lambda
then simply runs tree-walked, exactly as if this compiler did not exist."
  (handler-case
      (progn
        (when (or (some #'dynamic-sym-p fixed) (and rest (dynamic-sym-p rest)))
          (cbail "dynamic (special) parameters are not supported"))
        (let* ((*lc-literals* nil) (*lc-literal-count* 0) (*lc-gensym-count* 0)
               (bound (if rest (cons rest fixed) fixed))
               (body-cl (lc-compile-form body bound env))
               (lits (reverse *lc-literals*))
               (lit-vars (loop for i below (length lits)
                               collect (intern (format nil "%K~D" i) '#:lamedh-rt)))
               (factory-form
                 `(lambda (%env ,@lit-vars)
                    (declare (ignorable %env ,@lit-vars))
                    (lambda (%args)
                      (destructuring-bind (,@fixed ,@(and rest `(&rest ,rest))) %args
                        (declare (ignorable ,@fixed ,@(and rest (list rest))))
                        ,body-cl))))
               (hash (lc-form-hash factory-form))
               (factory (or (lc-find-factory factory-form hash)
                            (let ((f (lc-compile-factory factory-form)))
                              (lc-add-factory factory-form hash f)
                              (when *lc-recording* (setf *lc-compiled-while-recording* t))
                              f))))
          (when (and *lc-recording* (not (gethash factory *lc-recorded-set*))
                     (lc-persistable-p factory-form))
            (setf (gethash factory *lc-recorded-set*) t)
            (push (cons factory-form factory) *lc-recorded*))
          (apply factory env lits)))
    (compile-unsupported () nil)
    (error () nil)))

(setf *lambda-compile-hook* #'try-compile-lambda)

;;; ============================================================================
;;; The stdlib factory cache
;;; ============================================================================
;;;
;;; Bootstrapping the stdlib creates ~1,100 lambdas; compiling ~630 of them
;;; with COMPILE was most of the port's startup time (#542). Their factory
;;; forms depend only on the stdlib's source, so after a bootstrap that had
;;; to compile any, every stdlib factory is written out as one COMPILE-FILEd
;;; fasl; the next process loads it and bootstraps without calling the
;;; compiler. The cache lives under the XDG cache directory, keyed by the SBCL
;;; version and a hash of this port's own source (src/*.lisp), so a port or
;;; compiler change starts a fresh file; the stdlib's source needs no key,
;;; since a changed lambda simply generates a different form and misses.
;;; Setting LAMEDH_SBCL_NO_CACHE (to anything) disables both reading and
;;; writing. Any failure to read or write it is ignored: the cache only ever
;;; saves calls to COMPILE, it never changes what a lambda does.

(defun lc-cache-disabled-p () (and (uiop:getenv "LAMEDH_SBCL_NO_CACHE") t))

(defun lc-source-digest ()
  "Hex MD5 over the port's own source -- src/*.lisp and lamedh.asd -- each
file's name and length-prefixed text, in name order, so any edit, rename,
addition or removal changes it."
  (let ((files (sort (cons (asdf:system-relative-pathname :lamedh "lamedh.asd")
                           (directory (asdf:system-relative-pathname :lamedh "src/*.lisp")))
                     #'string< :key #'file-namestring)))
    (with-output-to-string (hex)
      (loop for byte across
            (sb-md5:md5sum-string
             (with-output-to-string (s)
               (dolist (f files)
                 (let ((text (uiop:read-file-string f :external-format :utf-8)))
                   (format s "~A~%~D~%~A" (file-namestring f) (length text) text))))
             :external-format :utf-8)
            do (format hex "~(~2,'0X~)" byte)))))

(defun lc-cache-path ()
  (uiop:xdg-cache-home
   "lamedh-sbcl"
   (format nil "~A-~A-~A" (lisp-implementation-type) (lisp-implementation-version)
           (lc-source-digest))
   "stdlib-lambdas.fasl"))

(defun lc-register-cached-factory (form factory)
  "Called by the cache fasl's top-level forms."
  (lc-add-factory form (lc-form-hash form) factory t))

(defun lc-load-cache ()
  (unless (lc-cache-disabled-p)
    (let ((path (ignore-errors (lc-cache-path))))
      (when (and path (probe-file path))
        (handler-case (handler-bind ((warning #'muffle-warning)) (load path))
          (error () nil))))))

(defun lc-save-cache ()
  "Write every persistable factory this bootstrap used to the cache fasl,
if it had to compile any of them."
  (when (and *lc-compiled-while-recording* (not (lc-cache-disabled-p)))
    (ignore-errors
     (let* ((path (lc-cache-path))
            (tag (format nil "~36R" (random (expt 2 64) (make-random-state t))))
            (src (make-pathname :name (format nil "stdlib-lambdas-~A" tag) :type "lisp" :defaults path))
            (tmp (make-pathname :name (format nil "stdlib-lambdas-~A" tag) :type "fasl" :defaults path)))
       (ensure-directories-exist path)
       (unwind-protect
            (progn
              (with-open-file (out src :direction :output :if-exists :supersede :external-format :utf-8)
                (with-standard-io-syntax
                  (let ((*package* (find-package '#:lamedh-rt)) (*print-circle* t) (*print-readably* t))
                    (format out "(in-package #:lamedh-rt)~%")
                    (dolist (entry (reverse *lc-recorded*))
                      (prin1 `(lc-register-cached-factory ',(car entry) #',(car entry)) out)
                      (terpri out)))))
              (with-standard-io-syntax
                (let ((*print-readably* nil) (*compile-verbose* nil) (*compile-print* nil))
                  (handler-bind ((warning #'muffle-warning))
                    (multiple-value-bind (fasl warnings-p failure-p)
                        (compile-file src :output-file tmp)
                      (declare (ignore warnings-p))
                      (when (and fasl (not failure-p))
                        (rename-file tmp path)))))))
         (ignore-errors (delete-file src))
         (when (probe-file tmp) (ignore-errors (delete-file tmp))))))))
