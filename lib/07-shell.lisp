;; Shell helpers built on the SHELL capability primitive.
;;
;; The SHELL capability is OFF by default.  Grant it from the host:
;;   env.enable_feature("SHELL")   ; Rust host code
;;   lamedh --capability SHELL     ; CLI
;;
;; The primitive (shell cmd) returns a list: (exit-code stdout stderr).
;; These helpers compose on that raw data -- code is data is code.

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
Signals an error if the command exits non-zero. Requires the SHELL feature."
  (let ((result (shell cmd)))
    (if (shell-ok-p result)
        (shell-stdout result)
        (error (concat "shell command failed: " cmd)))))

;;; REQUIRE-ABLE (issue #256): `(require 'shell)` on a with_prelude()
;;; environment loads exactly this file. with_stdlib() still loads it
;;; unconditionally, unchanged.
(provide 'shell '(sh shell-exit-code shell-stdout shell-stderr shell-ok-p))
