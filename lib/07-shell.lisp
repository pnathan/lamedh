;; SHELL module -- shell helpers built on the SHELL capability primitive
;; (issue #56 namespacing retrofit).
;;
;; WHY A MODULE: these are host-integration TOOLS, not language vocabulary,
;; so per the #253/#56 namespace ruling they live under a module and are
;; called qualified (SHELL:SH, SHELL:SHELL-STDOUT) or bound flat with
;; `(import shell)`. The module system (lib/27-modules.lisp) now loads ahead
;; of this file (see STDLIB_SOURCES in src/lib.rs), so WITH-MODULE is
;; available here even under `with_stdlib`.
;;
;; The SHELL capability is OFF by default. Grant it from the host:
;;   env.enable_feature("SHELL")   ; Rust host code
;;   lamedh --capability SHELL     ; CLI
;;
;; The kernel primitive (shell cmd) returns a list (exit-code stdout stderr).
;; It is NOT a module name -- it stays the flat kernel builtin; only the Lisp
;; helpers below are qualified. These helpers compose on that raw data.

(require 'modules)

(defmodule shell
  (:export sh shell-exit-code shell-stdout shell-stderr shell-ok-p)
  (:requires SHELL))

(with-module shell

  (defun shell-exit-code (result)
    "Exit code of a (shell ...) result."
    (car result))

  (defun shell-stdout (result)
    "Standard output string of a (shell ...) result."
    (cadr result))

  (defun shell-stderr (result)
    "Standard error string of a (shell ...) result."
    (caddr result))

  (defun shell-ok-p (result)
    "True when the command exited zero."
    (zerop (shell-exit-code result)))

  (defun sh (cmd)
    "Run CMD via the shell and return its stdout as a string.
Signals an error if the command exits non-zero. Requires the SHELL feature.
The bare `shell` here is the flat kernel primitive, not a module name."
    (let ((result (shell cmd)))
      (if (shell-ok-p result)
          (shell-stdout result)
          (error (concat "shell command failed: " cmd))))))

;;; REQUIRE-ABLE (issue #256): `(require 'shell)` on a with_prelude()
;;; environment loads exactly this file. with_stdlib() still loads it
;;; unconditionally, unchanged.
(provide 'shell '(shell:sh shell:shell-exit-code shell:shell-stdout
                  shell:shell-stderr shell:shell-ok-p))
