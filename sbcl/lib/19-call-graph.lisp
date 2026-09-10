;;; Call-graph analysis pass.
;;;
;;; Builds a caller→callees map from DEFUN definitions and stores it in the
;;; global hash table $CALL-GRAPH (symbol → list-of-symbols).
;;;
;;; The analysis is now LAZY: DEFUN no longer triggers eager body-walking.
;;; Instead, each DEFUN call pushes the function name onto $CG-PENDING.
;;; Query functions populate entries on demand:
;;;   - call-graph-callees / call-graph-has-p   populate the single queried name
;;;   - call-graph-callers                       flushes all $CG-PENDING names first
;;;     (so reverse lookup always sees every function defined via DEFUN)
;;;
;;; For functions defined before this file was loaded (e.g. stdlib helpers in
;;; files 00–18), use call-graph-add! or call-graph-add-many! explicitly.
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
;;;   $CG-PENDING                         -- names awaiting call-graph population
;;;   (defun-update-call-graph! n p body) -- eagerly add an entry (still callable)
;;;   (call-graph-callees name)           -- callees of NAME; lazy-populates NAME
;;;   (call-graph-callers name)           -- callers of NAME; flushes $CG-PENDING first
;;;   (call-graph-has-p name)             -- T if NAME is (or can be) in the graph
;;;   (call-graph-all-known)              -- list of all recorded function names
;;;   (call-graph-add! name)              -- retroactively add NAME via see-source
;;;   (call-graph-add-many! names)        -- retroactively add a list of names
;;;   (cg-flush-pending!)                 -- flush $CG-PENDING into the graph

;;; ─── Global store ─────────────────────────────────────────────────────────

;;; Forward call graph: maps each analyzed function name (symbol) to the list
;;; of operator-position symbols it calls (may include builtins/special forms).
(def $call-graph (make-hash-table))

;;; Pending list: function names pushed by DEFUN that have not yet been added
;;; to $CALL-GRAPH.  Flushed by call-graph-callers before scanning.
(def $cg-pending nil)

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
    ;; FUNCALL / APPLY with a quoted or #'-referenced symbol — record
    ;; the target as a callee edge (#231).
    ((and (symbolp (car form))
          (member (car form) '(funcall apply))
          (consp (cdr form))
          (consp (cadr form))
          (member (car (cadr form)) '(quote function))
          (symbolp (cadr (cadr form)))
          (not (member (cadr (cadr form)) locals)))
     (let* ((target (cadr (cadr form)))
            (r1 (adjoin (car form) (adjoin target result))))
       (cg-collect-forms (cddr form) locals r1)))
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

;;; ─── Update hook ──────────────────────────────────────────────────────────

(defun defun-update-call-graph! (name params body-forms)
  "Record in $CALL-GRAPH the set of operator symbols called by NAME.
   PARAMS is the parameter list (seeds the initial locals set so that
   parameter names are not mistakenly classified as callee function names).
   No longer called automatically from DEFUN (call-graph is now lazy).
   Still callable directly for forced/retroactive population."
  (let* ((initial-locals (if params params nil))
         (callees        (cg-collect-forms body-forms initial-locals nil)))
    (set-bang $call-graph name callees)
    name))

;;; ─── Pending-list flush ───────────────────────────────────────────────────

(defun cg-flush-pending! ()
  "Add all functions in $CG-PENDING to $CALL-GRAPH, then clear the list.
   Called by call-graph-callers before scanning so every function defined
   via DEFUN is visible to the reverse lookup."
  (call-graph-add-many! $cg-pending)
  (setq $cg-pending nil))

;;; ─── Query helpers (lazy) ─────────────────────────────────────────────────

(defun call-graph-callees (name)
  "Return the callees of NAME, lazily populating the graph entry if absent."
  (if (has-key-p $call-graph name)
      (gethash-or $call-graph name nil)
      (progn
        (call-graph-add! name)
        (gethash-or $call-graph name nil))))

(defun call-graph-callers (name)
  "Return the list of functions that call NAME.
   Flushes $CG-PENDING first so all DEFUN-defined functions are in the graph.
   O(n) in the graph size; intended for one-shot passes, not hot paths."
  ;; Ensure all pending (DEFUN-defined) functions are analysed before scanning.
  (cg-flush-pending!)
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
  "Return T if NAME is (or can be added to) $CALL-GRAPH.
   Lazily populates the entry via see-source if NAME is not yet in the graph."
  (if (has-key-p $call-graph name)
      t
      (not (null (call-graph-add! name)))))

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

;;; REQUIRE-ABLE (issue #256): `(require 'call-graph)` on a with_prelude()
;;; environment loads exactly this file. with_stdlib() still loads it
;;; unconditionally, unchanged.
;;; Registered as a module for introspection (issue #56). The query API
;;; stays FLAT: `$cg-pending` and the update hook are referenced by the
;;; kernel (special_forms.rs) and the DEFUN macro (lib/00-core.lisp), so
;;; the whole surface is pinned flat. This DEFMODULE only records metadata.
(require 'modules)
(defmodule call-graph
  (:export call-graph-callees call-graph-callers call-graph-has-p
           call-graph-all-known call-graph-add! call-graph-add-many!
           defun-update-call-graph! cg-flush-pending!))
(provide 'call-graph)
