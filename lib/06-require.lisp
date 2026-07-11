;;; 06-require.lisp -- REQUIRE / PROVIDE: the load-once module discipline
;;; (issue #256, epic #253).
;;;
;;; "Module" here means the ticket's sense: a named, dependency-aware,
;;; load-once library UNIT -- not DEFMODULE's namespacing construct in
;;; lib/27-modules.lisp (a naming discipline that happens to also be called
;;; a "module"). The two compose: REQUIRE/PROVIDE decide WHETHER and WHEN a
;;; library's source runs; DEFMODULE/WITH-MODULE decide what its symbols are
;;; called once it does. Do not confuse them; see lib/27-modules.lisp's own
;;; header for the namespacing half.
;;;
;;;   (require 'json)
;;;   (require 'http)
;;;   (defun my-app () ...)
;;;   (provide 'my-app)
;;;
;;; REQUIRE resolves a module NAME through a per-environment registry, in
;;; this order:
;;;   1. sources the host registered directly (Rust embedder API:
;;;      `env.register_module(name, source)`);
;;;   2. sources embedded in the binary at compile time (the numbered
;;;      optional library files -- SHELL, LISP15, TESTING, OPTIMIZER-VAU,
;;;      CALL-GRAPH, CONDENSATION, GUARD, MATCH, RULES, VARIANTS,
;;;      INSTRUMENT, MODULES, TYPES, PROTOCOLS, TEXT, DOC-RENDERER,
;;;      HELP-SYSTEM, HELP-DATA -- see src/lib.rs's OPTIONAL_MODULES);
;;;   3. -- only under the READ-FS capability -- files named
;;;      "<search-path>/<downcased-name>.lisp" under the host-configured
;;;      disk module search paths (Rust-only to set; see
;;;      `Environment::add_module_search_path`. Lisp can only *read* the
;;;      configured list, via $MODULE-SEARCH-PATHS, never set it -- hosts
;;;      constrain disk resolution without exposing that authority to Lisp).
;;;
;;; Embedded and host-registered sources need no capability; a module that
;;; resolves only on disk is unreachable without READ-FS (FILE-EXISTS-P /
;;; READ-FILE enforce this the same way LOAD-FILE always has).
;;;
;;; LOAD-ONCE, HONESTLY. A second REQUIRE of an already-loaded module is a
;;; no-op -- it returns the name without touching the source again. A
;;; module whose source signals an error, or which finishes evaluating
;;; without calling (PROVIDE 'name), is NOT marked loaded: whatever
;;; top-level definitions it already ran before the failure point remain in
;;; the environment (matching the interpreter's ordinary incremental
;;; load/eval semantics -- see LOAD-FILE); this is not a transaction, and a
;;; subsequent REQUIRE retries from scratch. REQUIRE-RELOAD is the explicit
;;; escape hatch for development: it re-resolves and re-evaluates a module
;;; even if already loaded. Ordinary REQUIRE never does this implicitly.
;;;
;;; CYCLES. A REQUIRE for a name that is already on the current loading
;;; stack (this environment's chain of in-progress REQUIRE calls) is a hard
;;; error naming the full chain, e.g. "require: dependency cycle: A -> B ->
;;; C -> A".
;;;
;;; EXPORTS are metadata, not enforcement (no privacy, no reader-level
;;; qualification -- see the epic #253 non-goals). (PROVIDE 'name '(a b c))
;;; declares the names NAME claims; at completion REQUIRE warns if a
;;; declared export is unbound, and warns (or, with
;;; *REQUIRE-STRICT-EXPORTS*, errors) if a declared export was already
;;; claimed by a different module.
;;;
;;; WHY THIS FILE IS PRELUDE, NOT OPTIONAL: an environment built from
;;; `Environment::with_prelude()` must still be able to pull in optional
;;; libraries by name -- that is the entire point of the Prelude/optional
;;; split (see epic #253's "Prelude and optional-library split"). REQUIRE
;;; and PROVIDE are therefore always-available Prelude vocabulary, loaded
;;; right after 05-math.lisp and before any optional library.

(defdynamic *require-stack* nil
  "Names of modules currently mid-REQUIRE in this environment, innermost
(most recently started) first. Used for cycle detection and for recording
which modules a module's own load depends on.")

(defdynamic *require-strict-exports* nil
  "When T, an export collision (see the file header) signals an error
instead of a warning. Off by default; a host can bind this via the Lisp
global or by evaluating `(setq *require-strict-exports* t)` before loading
libraries that must not silently shadow each other's exports.")

(def $require-known-modules nil
  "Every module name REQUIRE/PROVIDE has ever seen in this environment, most
recently seen first. Backs LOADED-MODULES.")

(def $require-export-owners (make-hash-table)
  "Exported name (symbol) -> the module name that first claimed it via
PROVIDE's optional EXPORTS argument. Backs collision warnings.")

(defun $require-canonical-name (x)
  "Coerce X (a symbol or string module name) to its canonical symbol."
  (cond
    ((symbolp x) x)
    ((stringp x) (intern (string-upcase x)))
    (t (error (concat "require: expected a module name (symbol or string), got "
                      (prin1-to-string x))))))

(defun $require-track-known (name)
  (if (member name $require-known-modules)
      nil
      (setq $require-known-modules (cons name $require-known-modules))))

(defun $require-note-dependency (name)
  "Record NAME as a dependency of whichever module is currently loading (the
top of *REQUIRE-STACK*), if any."
  (if (null *require-stack*)
      nil
      (let ((parent (car *require-stack*)))
        (if (member name (getp parent "require.deps"))
            nil
            (putp parent "require.deps"
                  (append (getp parent "require.deps") (list name)))))))

(defun $require-resolve-disk (name paths)
  "Search PATHS (a list of directory strings) for
<path>/<downcased-name>.lisp. Returns (source . origin) or NIL. Each probe
goes through FILE-EXISTS-P / READ-FILE, so this signals the ordinary
READ-FS capability error if the caller's environment lacks it."
  (if (null paths)
      nil
      (let ((candidate (concat (car paths) "/"
                               (string-downcase (princ-to-string name))
                               ".lisp")))
        (if (file-exists-p candidate)
            (cons (read-file candidate) (concat "disk:" candidate))
            ($require-resolve-disk name (cdr paths))))))

(defun $require-resolve (name)
  "(source . origin) for NAME via the resolution order documented in the
file header, or NIL if no tier has it."
  (let ((hit ($module-source-lookup (princ-to-string name))))
    (if hit
        hit
        ($require-resolve-disk name ($module-search-paths)))))

(defun $require-warn (msg)
  (princ (concat "; require: warning: " msg))
  (terpri))

(defun $require-claim-export (owner sym)
  (let ((prior (gethash $require-export-owners sym)))
    (cond
      ((or (null prior) (eq prior owner))
       (sethash $require-export-owners sym owner))
      (*require-strict-exports*
       (error (concat "require: export collision: " (princ-to-string sym)
                      " is claimed by both " (princ-to-string prior)
                      " and " (princ-to-string owner))))
      (t ($require-warn (concat "export collision on " (princ-to-string sym)
                                " -- claimed by both " (princ-to-string prior)
                                " and " (princ-to-string owner)))))))

(defun $require-check-exports (name exports)
  (mapc (lambda (sym)
          (if (boundp sym)
              nil
              ($require-warn (concat (princ-to-string name) " exports "
                                     (princ-to-string sym) " but it is unbound")))
          ($require-claim-export name sym))
        exports))

(defun $require-finish (name)
  (if (getp name "require.provided")
      (progn
        ($require-check-exports name (getp name "require.exports"))
        (putp name "require.state" 'require-loaded)
        name)
      (error (concat "module " (princ-to-string name)
                     " completed loading without calling (provide '"
                     (string-downcase (princ-to-string name)) "')"))))

(defun $require-load (name)
  "Resolve, evaluate, and PROVIDE-check NAME. Assumes NAME is not already
loaded and not already on *REQUIRE-STACK* (REQUIRE checks both)."
  ($require-note-dependency name)
  (putp name "require.state" 'require-loading)
  (putp name "require.provided" nil)
  (putp name "require.error" nil)
  (let ((*require-stack* (cons name *require-stack*)))
    (handler-case
        (let ((resolved ($require-resolve name)))
          (if (null resolved)
              (error (concat "unknown module " (princ-to-string name)
                             " (checked host-registered and embedded sources"
                             (if (null ($module-search-paths))
                                 ""
                                 " and the configured disk search path(s)")
                             ")"))
              (progn
                (putp name "require.source" (cdr resolved))
                ($eval-module-source (princ-to-string name) (car resolved))
                ($require-finish name))))
      (error (c)
        (putp name "require.state" 'require-unloaded)
        (putp name "require.error" (error-message c))
        (error (concat "require: failed to load module " (princ-to-string name)
                       ": " (error-message c)))))))

(defun $require-mark-loaded! (name source-label)
  "INTERNAL. Directly mark NAME REQUIRE-loaded with SOURCE-LABEL, bypassing
resolution/eval/PROVIDE-checking entirely. Used only by the Rust-side
WITH-STDLIB bootstrap (issue #256) to register modules it has already
evaluated inline via the historical STDLIB load order, so that a later
`(require 'name)` reached from a WITH-STDLIB environment -- including from
inside another still-loading stdlib file, e.g. 30-text.lisp's own
`(require 'modules)` -- is a correct no-op instead of a redundant
re-evaluation. Not part of the public REQUIRE surface; do not call this
from ordinary Lisp code."
  ($require-track-known name)
  (putp name "require.state" 'require-loaded)
  (putp name "require.source" source-label)
  (putp name "require.provided" t)
  name)

(defun require (name)
  "(REQUIRE 'name) -- load module NAME (a symbol or string) at most once in
this environment; returns NAME's canonical (uppercase) symbol. See this
file's header comment for the full resolution order, load-once semantics,
cycle detection, and failure handling."
  (let ((canonical ($require-canonical-name name)))
    ($require-track-known canonical)
    (cond
      ((eq (getp canonical "require.state") 'require-loaded) canonical)
      ((member canonical *require-stack*)
       (error (concat "require: dependency cycle: "
                      ($require-chain-string (cons canonical *require-stack*)))))
      (t ($require-load canonical) canonical))))

(defun $require-chain-string (names)
  (string-join (mapcar #'princ-to-string (reverse names)) " -> "))

(defun provide (name &optional exports)
  "(PROVIDE 'name [exports]) -- called from within a module's own source (as
loaded by REQUIRE) to mark NAME complete; conventionally the module's last
top-level form. REQUIRE signals an error if a module's source finishes
evaluating without a matching PROVIDE. EXPORTS, when given, is a list of
symbol names this module claims to define -- metadata only (see the file
header); it enables REQUIRE's unbound/collision diagnostics."
  (let ((canonical ($require-canonical-name name)))
    (putp canonical "require.provided" t)
    (if exports (putp canonical "require.exports" exports) nil)
    canonical))

(defun require-reload (name)
  "(REQUIRE-RELOAD 'name) -- development/debugging operation: force NAME to
be re-resolved and re-evaluated via REQUIRE's normal procedure even though
it is already loaded. Ordinary REQUIRE never does this implicitly. Errors
if NAME is currently mid-load (on *REQUIRE-STACK*)."
  (let ((canonical ($require-canonical-name name)))
    (if (member canonical *require-stack*)
        (error (concat "require-reload: " (princ-to-string canonical)
                       " is currently loading"))
        (progn
          ($require-track-known canonical)
          (putp canonical "require.state" nil)
          ($require-load canonical)
          canonical))))

(defun loaded-modules ()
  "All module names currently REQUIRE-loaded in this environment (in no
particular order)."
  (filter (lambda (n) (eq (getp n "require.state") 'require-loaded))
          $require-known-modules))

(defun module-loaded-p (name)
  "T if NAME is currently REQUIRE-loaded in this environment."
  (eq (getp ($require-canonical-name name) "require.state") 'require-loaded))

(defun module-state (name)
  "'REQUIRE-LOADED, 'REQUIRE-LOADING, 'REQUIRE-UNLOADED, or NIL if NAME has
never been REQUIREd, PROVIDEd, or registered in this environment."
  (getp ($require-canonical-name name) "require.state"))

(defun module-info (name)
  "Alist of diagnostic metadata REQUIRE tracks for NAME: STATE, SOURCE (an
origin string such as \"registered\", \"embedded\", or \"disk:<path>\"),
DEPS (names REQUIREd while NAME itself was loading), EXPORTS, and ERROR (the
last load failure's message, or NIL)."
  (let ((canonical ($require-canonical-name name)))
    (list (cons 'state (getp canonical "require.state"))
          (cons 'source (getp canonical "require.source"))
          (cons 'deps (getp canonical "require.deps"))
          (cons 'exports (getp canonical "require.exports"))
          (cons 'error (getp canonical "require.error")))))
