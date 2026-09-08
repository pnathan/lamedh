;;;; cli.lisp -- REPL / script runner entry points.

(in-package #:lamedh-rt)

(defun run-string (source)
  "Evaluate every top-level form in SOURCE, returning the last value.
Grants no capabilities of its own -- the library/embedding default (see
ENABLE-FEATURE); the CLI entry points below grant capabilities themselves
before calling this."
  (let ((result nil))
    (dolist (form (lread-all source) result) (setf result (leval form *global-env*)))))

(defun run-file (path)
  (run-string (uiop:read-file-string path)))

(defun run-repl ()
  (format t "~&Lamedh (SBCL port) -- Ctrl-D to exit.~%")
  (loop
    (format t "~&lamedh> ")
    (force-output)
    (let ((line (read-line *standard-input* nil :eof)))
      (when (eq line :eof) (format t "~%") (return))
      (handler-case
          (dolist (form (lread-all line))
            (format t "~A~%" (lprint-to-string (leval form *global-env*) t)))
        (lamedh-unbound-variable (c) (format t "~&error: ~A~%" c))
        (lamedh-condition (c) (format t "~&error: ~A~%" (lamedh-condition-value-string c)))
        (error (c) (format t "~&error: ~A~%" c))))))

;;; ---- argv parsing: --sandbox / --capability NAME / a script path -----------
;;;
;;; Matches the reference `lamedh` CLI: every capability is granted by
;;; default; --sandbox grants none; one or more --capability NAME grants
;;; exactly those (repeatable). A remaining bare argument is the script path.

(defun parse-cli-args (args)
  "Returns (VALUES SANDBOX-P EXPLICIT-CAPABILITIES SCRIPT-PATH)."
  (let (sandbox-p caps script)
    (loop while args do
      (let ((a (pop args)))
        (cond
          ((string= a "--sandbox") (setf sandbox-p t))
          ((string= a "--capability")
           (unless args (lamedh-error "--capability requires a NAME argument"))
           (push (pop args) caps))
          (t (setf script a)))))
    (values sandbox-p (nreverse caps) script)))

(defun toplevel ()
  (let ((args (uiop:command-line-arguments)))
    (multiple-value-bind (sandbox-p caps script) (parse-cli-args args)
      (cond
        (caps (dolist (c caps) (enable-feature c)))
        (sandbox-p nil)
        (t (enable-all-features)))
      (handler-case
          (if script (run-file script) (run-repl))
        (error (c) (format *error-output* "~&lamedh: ~A~%" c) (uiop:quit :unix-status 1)))))
  (uiop:quit :unix-status 0))
