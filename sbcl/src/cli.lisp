;;;; cli.lisp -- REPL / script runner entry points.

(in-package #:lamedh-rt)

(defun run-string (source)
  "Evaluate every top-level form in SOURCE, returning the last value."
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

(defun toplevel ()
  (let ((args (uiop:command-line-arguments)))
    (handler-case
        (cond
          ((null args) (run-repl))
          (t (run-file (first args))))
      (error (c) (format *error-output* "~&lamedh: ~A~%" c) (uiop:quit 1))))
  (uiop:quit 0))
