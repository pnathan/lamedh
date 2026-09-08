;;; Condition-system ergonomics (issue #149, epic #141).
;;;
;;; ERRORSET, UNWIND-PROTECT, CATCH/THROW, and BLOCK/RETURN-FROM are kernel
;;; special forms (they require non-local control flow that cannot be expressed
;;; in pure Lisp). This file adds the convenience macros on top of ERRORSET.
;;;
;;; ERRORSET is a function taking a QUOTED form (code as data); it evaluates the
;;; form, trapping ordinary errors. It returns (value) on success and NIL on
;;; error. Because success is wrapped in a one-element list, even a NIL result is
;;; distinguishable from an error (the wrapper list is truthy). The macros below
;;; quote the form they hand to ERRORSET.

(defmacro ignore-errors (&rest body)
  "Evaluate BODY; return its value, or NIL if it signals an error."
  (list 'car (list 'errorset (list 'quote (cons 'progn body)))))

;; HANDLER-CASE is a kernel special form (it binds the handler variable to the
;; first-class condition value — a LispVal::Error — which a Lisp macro over
;; ERRORSET cannot recover). See evaluator.rs and lib/16 docs. Signature:
;;   (handler-case expr (error (var) handler-body...))

;;; ------------------------------------------------------------------
;;; Restarts (build-order #1, 2026-07-13): the recovery half of the
;;; condition system, in pure Lisp on CATCH/THROW + dynamic variables +
;;; HANDLER-CASE.
;;;
;;;   (restart-case EXPR
;;;     (NAME (params...) body...) ...)
;;;
;;; establishes named restarts for EXPR's dynamic extent. Code below —
;;; typically an error handler — can (INVOKE-RESTART 'NAME args...):
;;; control unwinds to the RESTART-CASE, the clause body runs with the
;;; arguments, and its value becomes RESTART-CASE's value. Introspection:
;;; (COMPUTE-RESTARTS) lists the live restarts innermost-first,
;;; (FIND-RESTART 'NAME) finds one, (RESTART-NAME r) names it.
;;;
;;; The canonical shape — recovery chosen by the handler, performed at the
;;; establisher:
;;;
;;;   (restart-case
;;;       (handler-bind ((error (lambda (e) (invoke-restart 'use-value 0))))
;;;         (parse-config path))
;;;     (use-value (v) v))
;;;
;;; Deviation from Common Lisp, documented honestly: HANDLER-BIND's handler
;;; runs AFTER the erring form's own stack has unwound (it is HANDLER-CASE
;;; underneath), so restarts established BETWEEN the signal point and the
;;; HANDLER-BIND are already gone when the handler runs. Restarts
;;; established AROUND the handler — the canonical shape above — work
;;; exactly as in CL. A handler that returns normally DECLINES: the
;;; condition is re-signaled to the next handler out.
;;;
;;; For an agent (human or LLM), a restart is a repair protocol: the error
;;; site offers named recoveries with arguments instead of a dead process,
;;; and the handler chooses one programmatically.

(defdynamic *restarts* nil
  "Innermost-first list of live restart records established by RESTART-CASE.")

(defun restart-name (r)
  "The name (a symbol) of a restart record from COMPUTE-RESTARTS/FIND-RESTART."
  (car (cdr r)))

(defun $restart-id (r) (car (cdr (cdr r))))
(defun $restart-tag (r) (car (cdr (cdr (cdr r)))))
(defun $restart-marker (r) (car (cdr (cdr (cdr (cdr r))))))

(defun compute-restarts ()
  "All live restarts, innermost first."
  *restarts*)

(defun find-restart (name)
  "The innermost live restart named NAME, or NIL."
  ($restart-find name *restarts*))

(defun $restart-find (name rs)
  (cond ((null rs) nil)
        ((eq (restart-name (car rs)) name) (car rs))
        (t ($restart-find name (cdr rs)))))

(defun invoke-restart (designator &rest args)
  "Transfer control to the restart named DESIGNATOR (a symbol, or a record
from FIND-RESTART/COMPUTE-RESTARTS), passing ARGS to its clause. Unwinds to
the establishing RESTART-CASE; does not return. Errors if no such restart
is live."
  (let ((r (if (symbolp designator) (find-restart designator) designator)))
    (if (null r)
        (error (concat "invoke-restart: no live restart named "
                       (prin1-to-string designator)))
        (throw ($restart-tag r)
               (list ($restart-marker r) ($restart-id r) args)))))

(defvau restart-case (x e)
  "(RESTART-CASE expr (name (params...) body...) ...) — evaluate EXPR with
the named restarts established for its dynamic extent. Returns EXPR's
value, or, when a restart is invoked, the value of that clause's body
applied to the invocation's arguments."
  (let* ((expr (car x))
         (clauses (cdr x))
         (tag (gensym))
         (marker (gensym))
         ;; One record and one clause-function per clause. The functions are
         ;; closed in the caller's environment.
         (entries (mapcar
                   (lambda (cl)
                     (let ((id (gensym)))
                       (list
                        (list '$restart (car cl) id tag marker)
                        (cons id (eval (cons 'lambda (cdr cl)) e)))))
                   clauses))
         (records (mapcar (lambda (en) (car en)) entries))
         (fns (mapcar (lambda (en) (car (cdr en))) entries))
         ;; Establish: dynamically extend *RESTARTS* and run EXPR inside the
         ;; catch. The LET on a DEFDYNAMIC variable is a shallow dynamic
         ;; binding, restored on every exit path including the throw.
         (result (eval (list 'let
                             (list (list '*restarts*
                                         (list 'quote (append records *restarts*))))
                             (list 'catch (list 'quote tag) expr))
                       e)))
    (if (and (consp result) (eq (car result) marker))
        (apply (cdr (assoc (car (cdr result)) fns))
               (car (cdr (cdr result))))
        result)))

(defvau handler-bind (x e)
  "(HANDLER-BIND ((error handler-fn) ...) body...) — evaluate BODY; when it
signals, call each handler function in order with the condition. A handler
escapes by non-local transfer (typically INVOKE-RESTART); a handler that
returns normally DECLINES and the condition is re-signaled outward. See
the file header for the one documented deviation from CL."
  (let ((handlers (mapcar (lambda (cl) (eval (car (cdr cl)) e)) (car x)))
        (body (cons 'progn (cdr x))))
    (handler-case (eval body e)
      (error (condition)
        (progn
          ($restart-run-handlers handlers condition)
          ;; Every handler declined: re-signal the same condition value.
          (error condition))))))

(defun $restart-run-handlers (handlers condition)
  (cond ((null handlers) nil)
        (t (funcall (car handlers) condition)
           ($restart-run-handlers (cdr handlers) condition))))

;;; Standard restart invokers (conventions, not kernel magic): each finds
;;; the conventionally-named restart and invokes it, erroring clearly when
;;; none is live.

(defun use-value (v)
  "Invoke the innermost USE-VALUE restart with V."
  (invoke-restart 'use-value v))

(defun store-value (v)
  "Invoke the innermost STORE-VALUE restart with V."
  (invoke-restart 'store-value v))

(defun retry ()
  "Invoke the innermost RETRY restart."
  (invoke-restart 'retry))

(defun abort-to-restart ()
  "Invoke the innermost ABORT restart."
  (invoke-restart 'abort))

(defvau with-retry-restart (x e)
  "(WITH-RETRY-RESTART body...) — evaluate BODY with a RETRY restart
established; invoking it re-evaluates BODY from the top. The loop is
re-armed on every retry."
  (let ((body (cons 'progn x)))
    ($restart-retry-loop body e)))

(defun $restart-retry-loop (body e)
  (let* ((again (gensym))
         (r (eval (list 'restart-case body (list 'retry '() (list 'quote again))) e)))
    (if (eq r again)
        ($restart-retry-loop body e)
        r)))
