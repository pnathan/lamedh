;; `defun` defines a function and then — the one-door policy (2026-07) —
;; quietly attempts typed compilation via `jit-optimize`: HM inference fires
;; under the hood, and on success the name is rebound to a native membrane
;; that fast-paths typed calls and falls back to the original closure for
;; anything else. Functions that are not a fully-inferable typed island are
;; left exactly as-is, silently. Types are weather, not architecture.
;;
;; This reverses the old deliberate default ("auto-optimizing every defun is
;; not the default") because both recorded objections have since been
;; retired: numeric edge semantics now match across every tier (the #210
;; parity audit, Euclidean MOD #280, and the flag-differential suites), and
;; the membrane keeps the original closure so introspection and fallback
;; behavior are preserved. Opt out per function with a leading
;; `(declare (no-compile))` in the body, or globally with
;; `(declaim (no-compile name...))` — both also disable a later explicit
;; `(jit-optimize name)` (issue #168).
;;
;; Purity and call-graph analyses are computed LAZILY on first query (via
;; `pure-p` and `call-graph-callees`/`call-graph-callers`) rather than eagerly
;; at definition time.  This avoids multi-second startup costs during stdlib
;; loading.  The purity cache is invalidated on every redefinition so queries
;; always reflect the current body.  The call-graph pending list ($cg-pending)
;; accumulates names for `call-graph-callers` to flush on demand.

;; The quiet compile attempt behind one-door `defun`. Defined before `defun`
;; itself because every subsequent stdlib definition routes through it.
(def $defun-auto-compile
  (lambda (name)
    (if (getp name "no-compile")
        name
        ;; JIT-OPTIMIZE is a special form taking its symbol UNevaluated, so
        ;; build the call with the target name spliced in and eval it.
        (progn (eval (list 'jit-optimize name)) name))))

(defmacro defun (name params &rest body)
  ;; Split off an optional leading docstring, and an optional leading
  ;; `(declare (no-compile))` (issue #168) which pins the function to the
  ;; tree-walker: it is stripped from the body and recorded on the plist.
  (let* ((has-doc     (stringp (car body)))
         (doc         (if has-doc (car body) nil))
         (body-1      (if has-doc (cdr body) body))
         ;; Kernel-only structural test: EQUAL is not yet defined when
         ;; 00-core's own first defun expands.
         (first-form  (car body-1))
         (has-nc      (if (atom first-form)
                          nil
                          (if (eq (car first-form) 'declare)
                              (if (atom (car (cdr first-form)))
                                  nil
                                  (eq (car (car (cdr first-form))) 'no-compile))
                              nil)))
         (body-forms  (if has-nc (cdr body-1) body-1))
         (auto        (if has-nc
                          `(putp ',name "no-compile" t)
                          `($defun-auto-compile ',name)))
         (lambda-expr (cons 'lambda (cons params body-forms))))
    (if has-doc
      `(progn
         (def ,name ,lambda-expr ,doc)
         (remprop ',name "pure-checked")
         (remprop ',name "source-form")
         (if (boundp '$cg-pending)
             (setq $cg-pending (cons ',name $cg-pending))
             nil)
         (if (boundp '$call-graph)
             (delete-key $call-graph ',name)
             nil)
         ,auto
         ',name)
      `(progn
         (def ,name ,lambda-expr)
         (remprop ',name "pure-checked")
         (remprop ',name "source-form")
         (if (boundp '$cg-pending)
             (setq $cg-pending (cons ',name $cg-pending))
             nil)
         (if (boundp '$call-graph)
             (delete-key $call-graph ',name)
             nil)
         ,auto
         ',name))))

;; Global compile policy (issue #168): pin named functions to the
;; tree-walker. Takes effect for definitions made AFTER the declaim, and
;; disables any later explicit (jit-optimize name).
(defmacro declaim (spec)
  (if (eq (car spec) 'no-compile)
      (cons 'progn
            (mapcar (lambda (n) (list 'putp (list 'quote n) "no-compile" t))
                    (cdr spec)))
      (list 'error "declaim: only (no-compile name...) is supported")))

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
