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
;;;
;;; DEPENDS ON CONDENSATION (discovered by issue #259's own require-path
;;; test): WITH-MODULE's bookkeeping calls CONDENSE-APPEND-NEW, defined in
;;; lib/20-condensation.lisp, not this file -- a real gap under
;;; `with_prelude()` (unlike `with_stdlib()`, which happens to eager-load
;;; 20-condensation.lisp before 27-modules.lisp regardless of REQUIRE
;;; order): `(require 'modules)` alone left CONDENSE-APPEND-NEW unbound,
;;; so the FIRST `(with-module ...)` body anywhere -- e.g. simply
;;; `(require 'text)`, since lib/30-text.lisp itself uses WITH-MODULE --
;;; failed with an "unbound variable" error. Declared explicitly here so
;;; every REQUIRE path (not just the eager one) works. No cycle:
;;; 20-condensation.lisp uses no module-system macros itself.
(require 'condensation)

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

(defun $module-zip-renames (from to)
  (if (null from)
      ()
      (cons (cons (car from) (car to))
            ($module-zip-renames (cdr from) (cdr to)))))

(defun $module-form-defs (module form)
  "Rename pairs (local . stored) for every name a top-level FORM defines --
including the names DEFRECORD and DEFVARIANT generate (constructors,
predicates, accessors, validators). Generated names are derived from the
QUALIFIED brand (make-scene is stored as MAKE-SHAPES:SCENE), and the pair
list is what maps the local spelling onto that."
  (if (not (and (consp form)
                (member (car form) $module-def-heads)
                (symbolp (car (cdr form)))))
      ()
      (let* ((name (car (cdr form)))
             (qname ($module-qualify module name)))
        (cond
          ((eq (car form) 'defrecord)
           (let* ((specs (mapcar #'record-normalize-field-spec
                                 (filter (lambda (p) (not (record-section-p p)))
                                         (cdr (cdr form)))))
                  (fields (condense-field-names specs)))
             (cons (cons name qname)
                   ($module-zip-renames (record-generated name fields)
                                        (record-generated qname fields)))))
          ((eq (car form) 'defvariant)
           ;; Constructor names are the spec heads; they qualify too, so the
           ;; stored names derive from the QUALIFIED heads (shapes:circle,
           ;; shapes:circle-p, shapes:circle-r ...).
           (let* ((ctor-specs (mapcar #'$variant-normalize-ctor (cdr (cdr form))))
                  (qspecs (mapcar (lambda (spec)
                                    (cons ($module-qualify module (car spec))
                                          (cdr spec)))
                                  ctor-specs)))
             (cons (cons name qname)
                   ($module-zip-renames ($variant-generated name ctor-specs)
                                        ($variant-generated qname qspecs)))))
          (t (list (cons name qname)))))))

(defun $module-collect-defs (module body)
  "Rename alist for the names BODY defines (generated names included)."
  (reduce #'append
          (mapcar (lambda (f) ($module-form-defs module f)) body)
          nil))

(defun $module-merge-renames (old new)
  "Append NEW rename pairs whose local spelling is not already mapped."
  (append old
          (filter (lambda (p) (null (assoc (car p) old))) new)))

(defun $module-renames-table (renames)
  "Hash-table view of a rename alist: local -> stored, first binding wins
(matching ASSOC's earliest-match rule). Built once per WITH-MODULE body so
$MODULE-REWRITE does an O(1) hash probe per symbol instead of an O(renames)
ASSOC walk per AST node -- the walk made loading a large module body
quadratic (renames x nodes) and dominated stdlib cold start."
  (let ((h (make-hash-table)))
    (mapc (lambda (p)
            (if (gethash h (car p))
                ()
                (sethash h (car p) (cdr p))))
          renames)
    h))

(defun $module-rewrite (form module rmap)
  "Apply the module renames (RMAP: hash table, local -> stored) to
references. QUOTE/QUASIQUOTE untouched. Thin wrapper over the SEXPR-RENAME
kernel builtin -- the walk visits every AST node of every WITH-MODULE body,
and the interpreted edition of it dominated stdlib cold start."
  (sexpr-rename form rmap))

(defvau with-module (x e)
  "(WITH-MODULE name form...) -- evaluate FORMs with definitions and
module-local references qualified as NAME:SYMBOL. Local names are the
names defined in THIS body plus those from earlier WITH-MODULE bodies for
the same module. Caveat: qualification is name-based, so avoid reusing a
module function's name as an inner binding inside the module body."
  (let* ((module (car x))
         (body (cdr x))
         (defined ($module-collect-defs module body))
         (renames ($module-merge-renames (getp module "module.locals") defined))
         (rmap ($module-renames-table renames)))
    (if (module-p module) () (eval (list 'defmodule module) e))
    (putp module "module.locals" renames)
    (let ((result ()))
      (mapc (lambda (form)
              (setq result (eval ($module-rewrite form module rmap) e)))
            body)
      (mapc (lambda (pair)
              ;; Uniform outside spelling: generated names whose stored form
              ;; embeds the qualified brand (MAKE-SHAPES:SCENE) also get the
              ;; MODULE:LOCAL alias (SHAPES:MAKE-SCENE), so callers qualify
              ;; every module name the same way.
              (let ((uniform ($module-qualify module (car pair))))
                (if (eq uniform (cdr pair))
                    ()
                    (set uniform (eval (cdr pair) e))))
              (putp (cdr pair) "module" module))
            defined)
      ;; One merged registry write instead of one quadratic append per name:
      ;; CONDENSE-APPEND-NEW checks each new name against the growing list,
      ;; so a single call over all stored names is exactly equivalent to the
      ;; former per-pair loop (same order, same dedup) at a fraction of the
      ;; cost on large module bodies.
      (putp module "module.functions"
            (condense-append-new (getp module "module.functions")
                                 (mapcar (lambda (p) (cdr p)) defined)))
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

;;; REQUIRE-ABLE (issue #256): `(require 'modules)` on a with_prelude()
;;; environment loads exactly this file. with_stdlib() still loads it
;;; unconditionally, unchanged. Note the terminology overlap: this file's
;;; own "module" (DEFMODULE's namespacing unit) is a different concept from
;;; REQUIRE/PROVIDE's "module" (a load-once library unit); see
;;; lib/06-require.lisp's header.
(provide 'modules)
