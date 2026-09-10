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
;;;;     against the lambda's closed-over environment (embedded as a
;;;;     literal %ENV binding), so a later global redefinition is still
;;;;     seen -- nothing is baked in as a compile-time constant except the
;;;;     environment object identity itself
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

(defun lc-compile-and (args bound env)
  (cond
    ((null args) (list 'quote *t-sym*))
    ((null (cdr args)) (lc-compile-form (car args) bound env))
    (t (let ((v (gensym "AND")))
         `(let ((,v ,(lc-compile-form (car args) bound env)))
            (if (lamedh-truthy-p ,v) ,(lc-compile-and (cdr args) bound env) nil))))))

(defun lc-compile-or (args bound env)
  (cond
    ((null args) nil)
    ((null (cdr args)) (lc-compile-form (car args) bound env))
    (t (let ((v (gensym "OR")))
         `(let ((,v ,(lc-compile-form (car args) bound env)))
            (if (lamedh-truthy-p ,v) ,v ,(lc-compile-or (cdr args) bound env)))))))

(defun lc-compile-form (form bound env)
  "Compile FORM (a Lamedh s-expression) to a CL form, or signal
COMPILE-UNSUPPORTED. BOUND is the list of Lamedh symbols currently bound
as native CL lexical variables (this lambda's parameters); every other
symbol reference goes through %ENV, a local the generated lambda binds to
the literal, closed-over ENV object."
  (lc-init-syms)
  (cond
    ((null form) nil)
    ((numberp form) form)
    ((stringp form) form)
    ((characterp form) form)
    ((symbolp form)
     (cond
       ((self-evaluating-symbol-p form) (list 'quote form))
       ((member form bound) form)
       (t (list 'env-resolve '%env (list 'quote form)))))
    ((not (consp form)) (cbail "form ~S is neither a literal nor a cons" form))
    (t
     (let ((op (car form)) (args (cdr form)))
       (cond
         ((eq op *lc-quote*) (list 'quote (car args)))
         ((eq op *lc-if*)
          (destructuring-bind (c th &optional el) args
            (list 'if (list 'lamedh-truthy-p (lc-compile-form c bound env))
                  (lc-compile-form th bound env)
                  (if el (lc-compile-form el bound env) nil))))
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
        (let* ((bound (if rest (cons rest fixed) fixed))
               (body-cl (lc-compile-form body bound env))
               (lambda-form
                 (if rest
                     `(lambda (%args)
                        (let ((%env ,env))
                          (destructuring-bind (,@fixed &rest ,rest) %args
                            (declare (ignorable ,@fixed ,rest %env))
                            ,body-cl)))
                     `(lambda (%args)
                        (let ((%env ,env))
                          (destructuring-bind (,@fixed) %args
                            (declare (ignorable ,@fixed %env))
                            ,body-cl))))))
          (compile nil lambda-form)))
    (compile-unsupported () nil)
    (error () nil)))

(setf *lambda-compile-hook* #'try-compile-lambda)
