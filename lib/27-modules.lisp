;;; 27-modules.lisp -- the module story: DEFMODULE / WITH-MODULE / IMPORT.
;;;
;;; A module is a NAMING DISCIPLINE plus metadata, not a new evaluator
;;; concept: symbols defined inside (WITH-MODULE m ...) are stored as
;;; M:SYMBOL -- one flat global namespace, qualified names, zero lookup
;;; machinery. `:` is an ordinary symbol constituent (non-initial), so
;;; GEOMETRY:AREA is just a symbol you can also write directly.
;;;
;;;   (defmodule geometry
;;;     (:export area)
;;;     (:provides FAST-MATH))          ; optional custom capability
;;;
;;;   (with-module geometry
;;;     (defun area (r) (* 3 (* r r)))  ; defines GEOMETRY:AREA
;;;     (defun helper (x) x))           ; GEOMETRY:HELPER, unexported
;;;
;;;   (geometry:area 2)                 ; => 12 -- always available
;;;   (import geometry)                 ; binds AREA to GEOMETRY:AREA's value
;;;
;;; WITH-MODULE rewrites the body: definition heads (defun/def/defrecord/
;;; defvariant/defmacro/defvau) and references to module-local names are
;;; qualified; QUOTE and QUASIQUOTE subtrees are left untouched. Every
;;; defined function is associated with its module ((module-of 'geometry:area)
;;; => GEOMETRY) and recorded in (module-functions 'geometry).
;;;
;;; CAPABILITIES. (:requires CAP...) records what the module's operations
;;; need -- introspection via (module-requires m); enforcement stays where
;;; it always was, at the gated builtins. (:provides CAP...) REGISTERS a new
;;; custom capability name. Deliberately conservative semantics (this is the
;;; risky corner): a provided capability extends the capability VOCABULARY
;;; only -- it is held by registration at the outermost level, gates only
;;; explicit (require-capability 'CAP) checks in Lisp code, attenuates
;;; through WITH-CAPABILITIES/SANDBOXED/SPAWN like any built-in capability,
;;; and can never grant kernel abilities (READ-FS and friends remain
;;; host-granted only).

(def $modules (make-hash-table))

(defun $module-qualify (module name)
  "The stored symbol for NAME in MODULE: MODULE:NAME."
  (intern (concat (princ-to-string module) ":" (princ-to-string name))))

(defun $module-qualified-p (sym)
  "T when SYM already contains a colon (already module-qualified)."
  (not (null (string-index-of (princ-to-string sym) ":"))))

(defun module-p (name)
  (not (null (gethash $modules name))))

(defun module-exports (m) (getp m "module.exports"))
(defun module-functions (m) (getp m "module.functions"))
(defun module-requires (m) (getp m "module.requires"))
(defun module-provides (m) (getp m "module.provides"))
(defun module-of (fn-name) (getp fn-name "module"))

(defvau defmodule (x e)
  "(DEFMODULE name (:export a b...) [(:requires CAP...)] [(:provides CAP...)])
-- declare a module: its export list, the capabilities its operations
require (introspection), and any custom capabilities it provides (which
join the attenuable capability vocabulary; see the header comment for the
deliberately conservative semantics)."
  (let* ((name (car x))
         (sections (cdr x))
         (exports (cdr (assoc ':export sections)))
         (requires (cdr (assoc ':requires sections)))
         (provides (cdr (assoc ':provides sections))))
    (sethash $modules name t)
    (putp name "module.exports" exports)
    (putp name "module.requires" requires)
    (putp name "module.provides" provides)
    (if (null (getp name "module.functions"))
        (putp name "module.functions" ())
        ())
    (mapc (lambda (c)
            (if (member c $custom-capabilities)
                ()
                (setq $custom-capabilities (cons c $custom-capabilities))))
          provides)
    name))

(def $module-def-heads '(defun defun* defmacro defexpr defvau def defdynamic
                         defrecord defvariant))

(defun $module-collect-defs (body)
  "Names defined at the top level of BODY forms."
  (reduce #'append
          (mapcar (lambda (form)
                    (if (and (consp form)
                             (member (car form) $module-def-heads)
                             (symbolp (car (cdr form))))
                        (list (car (cdr form)))
                        ()))
                  body)
          nil))

(defun $module-rewrite (form module locals)
  "Qualify references to module-LOCAL names. QUOTE/QUASIQUOTE untouched."
  (cond
    ((symbolp form)
     (if (and (member form locals) (not ($module-qualified-p form)))
         ($module-qualify module form)
         form))
    ((not (consp form)) form)
    ((eq (car form) 'quote) form)
    ((eq (car form) 'quasiquote) form)
    (t (cons ($module-rewrite (car form) module locals)
             ($module-rewrite (cdr form) module locals)))))

(defvau with-module (x e)
  "(WITH-MODULE name form...) -- evaluate FORMs with definitions and
module-local references qualified as NAME:SYMBOL. Local names are the
names defined in THIS body plus those from earlier WITH-MODULE bodies for
the same module. Caveat: qualification is name-based, so avoid reusing a
module function's name as an inner binding inside the module body."
  (let* ((module (car x))
         (body (cdr x))
         (defined ($module-collect-defs body))
         (locals (condense-append-new (getp module "module.locals") defined)))
    (if (module-p module) () (eval (list 'defmodule module) e))
    (putp module "module.locals" locals)
    (let ((result ()))
      (mapc (lambda (form)
              (setq result (eval ($module-rewrite form module locals) e)))
            body)
      (mapc (lambda (n)
              (let ((q ($module-qualify module n)))
                (putp q "module" module)
                (putp module "module.functions"
                      (condense-append-new (getp module "module.functions")
                                           (list q)))))
            defined)
      result)))

(defvau import (x e)
  "(IMPORT module) -- bind each EXPORTED name globally to the module's
current value: (import geometry) makes AREA call GEOMETRY:AREA. Snapshot
semantics: importing binds values, not cells -- re-import after
redefinition. Errors if the module or an exported binding is unknown."
  (let* ((module (car x))
         (exports (module-exports module)))
    (if (module-p module)
        ()
        (error (concat "import: unknown module " (princ-to-string module))))
    (mapc (lambda (n)
            (set n (eval ($module-qualify module n) e)))
          exports)
    module))
