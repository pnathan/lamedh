;;;; host-regressions.lisp -- host-level (Common Lisp) regression checks for
;;;; the port's embedding entry points, which the Lamedh-level fixtures
;;;; cannot reach. Loaded by run-tests.lisp; RUN-HOST-REGRESSIONS returns
;;;; true when every check passes.

(in-package #:lamedh-rt)

(defparameter *late-parse-error-script*
  (format nil "(def $late-a 1)~%(princ \"one\") (terpri)~%(princ (+ 1 2)) (terpri)~%(princ (+ 3~%")
  "Two well-formed forms' worth of output, then a truncated third form.")

(defun capture-run (thunk)
  "Returns (VALUES STDOUT READER-ERROR-P) for THUNK."
  (let ((errored nil))
    (values (with-output-to-string (*standard-output*)
              (handler-case (funcall thunk)
                (lamedh-reader-error () (setf errored t))))
            errored)))

(defun host-check (name ok)
  (format t "~&; host ~:[FAIL~;ok  ~] ~A~%" ok name)
  ok)

(defun run-host-regressions ()
  (let ((expected (format nil "one~%3~%")) (results nil))
    ;; #541: forms before a late parse error run (and print) before it is signalled.
    (multiple-value-bind (out errored)
        (capture-run (lambda () (run-string *late-parse-error-script*)))
      (push (host-check "run-string: late parse error still signalled" errored) results)
      (push (host-check "run-string: earlier forms' output precedes the parse error"
                        (string= out expected))
            results)
      (push (host-check "run-string: earlier forms' effects persist"
                        (eql (leval (lread "$late-a") *global-env*) 1))
            results))
    (uiop:with-temporary-file (:stream s :pathname path :type "lisp")
      (write-string *late-parse-error-script* s)
      :close-stream
      (multiple-value-bind (out errored) (capture-run (lambda () (run-file path)))
        (push (host-check "run-file: late parse error still signalled" errored) results)
        (push (host-check "run-file: earlier forms' output precedes the parse error"
                          (string= out expected))
              results)))
    (push (host-check "run-string: returns the last form's value"
                      (eql (run-string "(+ 1 1) (+ 2 3)") 5))
          results)
    (push (host-check "run-string: empty source returns NIL" (null (run-string "  ; nothing"))) results)
    (every #'identity results)))
