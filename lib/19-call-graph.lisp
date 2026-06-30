;;; Call-graph analysis pass.
;;;
;;; Builds an incremental caller→callees map from DEFUN definitions and
;;; stores it in the global hash table $CALL-GRAPH (symbol → list-of-symbols).
;;;
;;; The DEFUN macro in 00-core.lisp calls defun-update-call-graph! after each
;;; definition (guarded by boundp so early stdlib files 00–18 are safe — they
;;; load before this file).  Functions defined after this file is loaded are
;;; tracked automatically.  For functions defined before (e.g. all stdlib
;;; helpers), use call-graph-add! or call-graph-add-many! to retroactively
;;; populate entries via see-source.
;;;
;;; Algorithm: walk each function body collecting operator-position symbols,
;;; skipping:
;;;   (a) quoted forms               — (quote ...) is data, not code
;;;   (b) locally-bound names        — lambda/let/let*/prog parameters
;;;   (c) structural binding keywords — lambda, let, let*, prog are handled
;;;                                    structurally, not as callees
;;;
;;; Entry points:
;;;   $CALL-GRAPH                         -- the global forward call-graph hash table
;;;   (defun-update-call-graph! n p body) -- called by DEFUN hook
;;;   (call-graph-callees name)           -- list of functions called by NAME
;;;   (call-graph-callers name)           -- list of functions that call NAME
;;;   (call-graph-has-p name)             -- T if NAME is in the graph
;;;   (call-graph-all-known)              -- list of all recorded function names
;;;   (call-graph-add! name)              -- retroactively add NAME via see-source
;;;   (call-graph-add-many! names)        -- retroactively add a list of names

;;; ─── Global store ─────────────────────────────────────────────────────────

;;; Forward call graph: maps each analyzed function name (symbol) to the list
;;; of operator-position symbols it calls (may include builtins/special forms).
(def $call-graph (make-hash-table))

;;; ─── Body-walking helpers ─────────────────────────────────────────────────

(defun cg-collect-callees (form locals result)
  "Accumulate operator-position symbols from FORM into RESULT (a list used as a set).
   LOCALS is the set of locally-bound names to suppress (excludes them from callees).
   Quoted forms are not walked; lambda/let/let*/prog introduce new locals."
  (cond
    ;; Atoms have no sub-applications to collect
    ((atom form) result)
    ;; Never walk inside quoted data
    ((eq (car form) 'quote) result)
    ;; (function sym) or (function (lambda ...)) — treat named sym as a callee
    ((eq (car form) 'function)
     (let ((arg (cadr form)))
       (if (and (symbolp arg) (not (member arg locals)))
           (adjoin arg result)
           result)))
    ;; (lambda (params...) body...) — extend locals with params
    ((eq (car form) 'lambda)
     (let ((new-locals (union (cadr form) locals)))
       (cg-collect-forms (cddr form) new-locals result)))
    ;; (let ((v e)...) body...) — collect from inits with current locals,
    ;;                           then walk body with extended locals.
    ;; Guard: the bindings argument must look like a list of pairs.  When this
    ;; form appears inside a quasiquote template the "bindings" slot may be an
    ;; UNQUOTE form like (UNQUOTE expr), whose elements are symbols not pairs.
    ;; In that case treat the whole thing as a general application to avoid
    ;; calling (car symbol) which raises "car requires a list".
    ((eq (car form) 'let)
     (let* ((bindings (cadr form)))
       (if (or (null bindings) (consp (car bindings)))
           ;; Bindings look like real let-pairs: handle structurally.
           (let* ((vars       (mapcar (lambda (b) (car b)) bindings))
                  (inits      (mapcar (lambda (b) (cadr b)) bindings))
                  (r1         (cg-collect-forms inits locals result))
                  (new-locals (union vars locals)))
             (cg-collect-forms (cddr form) new-locals r1))
           ;; Looks like a quasiquote template — fall through to general case.
           (let* ((op (car form))
                  (r1 (if (and (symbolp op) (not (member op locals)))
                          (adjoin op result)
                          result)))
             (cg-collect-forms (cdr form) locals r1)))))
    ;; (let* ((v e)...) body...) — each binding extends locals sequentially.
    ;; Same quasiquote-template guard as LET above.
    ((eq (car form) 'let*)
     (let* ((bindings (cadr form)))
       (if (or (null bindings) (consp (car bindings)))
           (cg-collect-let*-bindings bindings (cddr form) locals result)
           (let* ((op (car form))
                  (r1 (if (and (symbolp op) (not (member op locals)))
                          (adjoin op result)
                          result)))
             (cg-collect-forms (cdr form) locals r1)))))
    ;; (prog (vars...) stmts...) — extends locals with prog var list
    ((eq (car form) 'prog)
     (let ((new-locals (union (cadr form) locals)))
       (cg-collect-forms (cddr form) new-locals result)))
    ;; General application: car is the operator symbol (or compound)
    (t
     (let* ((op (car form))
            (r1 (if (and (symbolp op) (not (member op locals)))
                    (adjoin op result)
                    result)))
       (cg-collect-forms (cdr form) locals r1)))))

(defun cg-collect-forms (forms locals result)
  "Walk each form in FORMS, accumulating callee symbols into RESULT."
  (cond
    ((null forms) result)
    (t (cg-collect-forms (cdr forms) locals
                          (cg-collect-callees (car forms) locals result)))))

(defun cg-collect-let*-bindings (bindings body locals result)
  "Walk LET* BINDINGS sequentially (each var added to locals before the next
   init expression is walked), then walk BODY with the fully-extended locals."
  (cond
    ((null bindings)
     (cg-collect-forms body locals result))
    (t
     (let* ((b          (car bindings))
            (var        (car b))
            (init       (cadr b))
            (r1         (cg-collect-callees init locals result))
            (new-locals (adjoin var locals)))
       (cg-collect-let*-bindings (cdr bindings) body new-locals r1)))))

;;; ─── Update hook (called by DEFUN macro) ─────────────────────────────────

(defun defun-update-call-graph! (name params body-forms)
  "Record in $CALL-GRAPH the set of operator symbols called by NAME.
   PARAMS is the parameter list (seeds the initial locals set so that
   parameter names are not mistakenly classified as callee function names).
   Called automatically from the DEFUN macro in 00-core.lisp once this
   function is bound (i.e. after this file is loaded)."
  (let* ((initial-locals (if params params nil))
         (callees        (cg-collect-forms body-forms initial-locals nil)))
    (set-bang $call-graph name callees)
    name))

;;; ─── Query helpers ────────────────────────────────────────────────────────

(defun call-graph-callees (name)
  "Return the list of functions called by NAME, or NIL if NAME is not in the graph."
  (gethash-or $call-graph name nil))

(defun call-graph-callers (name)
  "Return the list of functions that call NAME (computed by scanning $CALL-GRAPH).
   O(n) in the number of functions in the graph; intended for one-shot analysis
   passes, not hot paths.
   Uses WHILE for iteration to avoid deep Lisp recursion over large graphs."
  (let ((result nil)
        (alist (hash->alist $call-graph)))
    (while alist
      (let ((entry (car alist)))
        (if (member name (cdr entry))
            (setq result (adjoin (car entry) result))
            nil))
      (setq alist (cdr alist)))
    result))

(defun call-graph-has-p (name)
  "Return T if NAME appears as a key in $CALL-GRAPH (has been analyzed)."
  (has-key-p $call-graph name))

(defun call-graph-all-known ()
  "Return the list of all function names recorded in $CALL-GRAPH."
  (keys $call-graph))

;;; ─── Retroactive entry points ─────────────────────────────────────────────

(defun call-graph-add! (name)
  "Add NAME to $CALL-GRAPH by retrieving its definition via SEE-SOURCE.
   Useful for functions defined before this file was loaded (e.g. all stdlib
   helpers in files 00–18).  Returns NAME on success, NIL if NAME is not
   an inspectable user function."
  (let ((src (errorset (list 'see-source (list 'quote name)))))
    (if (null src)
        nil
        ;; errorset returns a one-element list on success: ((lambda params body...))
        (let* ((lam    (car src))
               (params (cadr lam))
               (body   (cddr lam)))
          (defun-update-call-graph! name params body)
          name))))

(defun call-graph-add-many! (names)
  "Call call-graph-add! for each symbol in NAMES.
   Returns NIL; side-effects populate $CALL-GRAPH."
  (cond
    ((null names) nil)
    (t (call-graph-add! (car names))
       (call-graph-add-many! (cdr names)))))
