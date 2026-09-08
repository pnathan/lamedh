;;;; bootstrap.lisp -- loads the Lamedh-language standard-library subset
;;;; this port ships (sbcl/lib/*.lisp) into the global environment.
;;;;
;;;; These files are byte-for-byte copies of the reference implementation's
;;;; lib/*.lisp (see sbcl/README.md for the exact list and why the rest are
;;;; out of scope) -- they are Lamedh source, evaluated by THIS evaluator,
;;;; not Rust. Reusing them unmodified is what makes the two implementations
;;;; comparable: the same stdlib source runs on both.

(in-package #:lamedh-rt)

(defparameter *bootstrap-files*
  '("00-core" "01-list" "02-cxr" "03-meta" "04-predicates" "05-math"
    "08-vau" "09-lisp15" "10-testing" "12-control" "13-functional"
    "14-strings" "15-sets-hash" "17-arrays" "21-cl-compat"))

(defun lib-directory ()
  (asdf:system-relative-pathname :lamedh "lib/"))

(defun load-lamedh-file (path)
  (let ((forms (lread-all (uiop:read-file-string path))))
    (dolist (form forms) (leval form *global-env*))))

(defun bootstrap ()
  (dolist (name *bootstrap-files*)
    (let ((path (merge-pathnames (concatenate 'string name ".lisp") (lib-directory))))
      (handler-case (load-lamedh-file path)
        (error (c)
          (format *error-output* "~&; bootstrap error loading ~A: ~A~%" name c)
          (error c))))))

(bootstrap)
