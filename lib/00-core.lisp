;; `defun` defines an ordinary dynamic function. To additionally have the typed
;; JIT infer and natively compile it — HM firing under the hood, with a silent
;; fallback to the dynamic definition for calls whose arguments do not fit the
;; inferred type — wrap it with `jit-optimize`:
;;     (jit-optimize (defun inc (n) (+ n 1)))
;; or call `(jit-optimize name)` after defining. "Play like a Lisp, optimize
;; like Fortran." Functions that are not a fully-inferable typed island are left
;; exactly as-is. (Auto-optimizing *every* defun is deliberately not the default:
;; it would rebind the name to a native membrane, which changes introspection
;; (`see-source`) and the numeric edge semantics (overflow / div-by-zero, #67).)
;;
;; Purity and call-graph analyses are computed LAZILY on first query (via
;; `pure-p` and `call-graph-callees`/`call-graph-callers`) rather than eagerly
;; at definition time.  This avoids multi-second startup costs during stdlib
;; loading.  The purity cache is invalidated on every redefinition so queries
;; always reflect the current body.  The call-graph pending list ($cg-pending)
;; accumulates names for `call-graph-callers` to flush on demand.
(defmacro defun (name params &rest body)
  ;; Split off an optional leading docstring so body-forms holds only code.
  (let* ((has-doc     (stringp (car body)))
         (doc         (if has-doc (car body) nil))
         (body-forms  (if has-doc (cdr body) body))
         (lambda-expr (cons 'lambda (cons params body-forms))))
    (if has-doc
      `(progn
         (def ,name ,lambda-expr ,doc)
         (remprop ',name "pure-checked")
         (if (boundp '$cg-pending)
             (setq $cg-pending (cons ',name $cg-pending))
             nil)
         (if (boundp '$call-graph)
             (delete-key $call-graph ',name)
             nil)
         ',name)
      `(progn
         (def ,name ,lambda-expr)
         (remprop ',name "pure-checked")
         (if (boundp '$cg-pending)
             (setq $cg-pending (cons ',name $cg-pending))
             nil)
         (if (boundp '$call-graph)
             (delete-key $call-graph ',name)
             nil)
         ',name))))

;; Named vau operative with optional docstring as first body form.
;; (defvau name (operands-param env-param) ["docstring"] body...)
;; Expands to (def name ($vau (operands-param env-param) body...) "docstring").
(defmacro defvau (name params &rest body)
  (if (stringp (car body))
    (let ((vau-expr (cons '$vau (cons params (cdr body)))))
      `(def ,name ,vau-expr ,(car body)))
    (let ((vau-expr (cons '$vau (cons params body))))
      `(def ,name ,vau-expr))))

(defun prog2 (first second &rest rest)
  "Evaluate all forms; return the value of the second."
  second)

(defmacro cset (sym val)
  "Set the global value of symbol SYM (unevaluated) to VAL."
  `(setq ,sym ,val))

(defmacro csetq (sym val)
  "Alias for CSET; set the global value of SYM to VAL."
  `(setq ,sym ,val))
