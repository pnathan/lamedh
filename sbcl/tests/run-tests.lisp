;;;; run-tests.lisp -- load every ported *_test.lisp fixture under sbcl/tests
;;;; (Lamedh source, copied from the reference implementation's tests/lisp/)
;;;; and run the suite via the bootstrapped (run-tests) function from
;;;; lib/10-testing.lisp.

(require :asdf)
(asdf:load-asd (merge-pathnames "../lamedh.asd" *load-pathname*))
(asdf:load-system :lamedh)

(in-package #:lamedh-rt)

(defparameter *test-files*
  '("10-arithmetic" "20-lists" "30-predicates" "40-list-processing"
    "50-strings-symbols" "51-string-completions" "52-text-module"
    "60-special-forms" "65-loops" "70-hash-and-plist" "90-bitwise"
    "95-stdlib-batteries" "96-format-and-io"))

(dolist (name *test-files*)
  (let ((path (merge-pathnames (concatenate 'string name ".lisp")
                                (merge-pathnames "./" *load-pathname*))))
    (format t "~&; loading ~A~%" name)
    (run-string (uiop:read-file-string path))))

(let ((ok (leval (lread "(run-tests)") *global-env*)))
  (uiop:quit (if (eq ok *t-sym*) 0 1)))
