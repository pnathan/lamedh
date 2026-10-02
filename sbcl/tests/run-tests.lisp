;;;; run-tests.lisp -- load every ported *_test.lisp fixture under sbcl/tests
;;;; (Lamedh source, copied from the reference implementation's tests/lisp/)
;;;; and run the suite via the bootstrapped (run-tests) function from
;;;; lib/10-testing.lisp.

(require :asdf)
(asdf:load-asd (merge-pathnames "../lamedh.asd" *load-pathname*))
(asdf:load-system :lamedh)

(in-package #:lamedh-rt)

(enable-all-features) ; a trusted local test run, like the CLI's default

(defparameter *test-files*
  '("10-arithmetic" "11-mod-euclidean" "20-lists" "30-predicates" "40-list-processing"
    "50-strings-symbols" "51-string-completions" "52-text-module"
    "60-special-forms" "65-loops" "70-hash-and-plist" "80-kernel-conformance"
    "90-bitwise"
    "95-stdlib-batteries" "96-format-and-io" "97-common-forms" "97-no-ratios"
    "97-port-regressions"))

(dolist (name *test-files*)
  (let ((path (merge-pathnames (concatenate 'string name ".lisp")
                                (merge-pathnames "./" *load-pathname*))))
    (format t "~&; loading ~A~%" name)
    (run-string (uiop:read-file-string path))))

(load (merge-pathnames "host-regressions.lisp" *load-pathname*))

;; Shell-level CLI exit-status check (#535): runs the documented script
;; invocation in child SBCL processes and checks their exit codes.
(defun run-cli-exit-status-test ()
  (format t "~&; running cli-exit-status.sh~%")
  (let ((script (namestring (merge-pathnames "cli-exit-status.sh" *load-truename*))))
    (zerop (nth-value 2 (uiop:run-program (list "sh" script)
                                          :output t :error-output t
                                          :ignore-error-status t)))))

;; The user factory table is bounded (#542 review): lambdas EVALed with
;; fresh numeric literals each make a new factory shape, and must not grow
;; *LC-FACTORIES* past *LC-FACTORY-LIMIT*.
(defun run-factory-bound-test ()
  (format t "~&; checking the compiled-factory table bound~%")
  (let* ((*lc-factory-limit* 8)
         (ok (progn
               (run-string "(dotimes (i 20) (eval (list 'lambda '(x) (list '+ 'x i))))")
               (and (<= *lc-factory-count* *lc-factory-limit*)
                    (eql (run-string "(funcall (eval (list 'lambda '(x) (list '+ 'x 41))) 1)")
                         42)))))
    (format t "~&; factory table bound ~:[FAIL~;ok~] (~D forms)~%" ok *lc-factory-count*)
    ok))

(let ((ok (leval (lread "(run-tests)") *global-env*))
      (host-ok (run-host-regressions))
      (cli-ok (run-cli-exit-status-test))
      (factory-ok (run-factory-bound-test)))
  (uiop:quit (if (and (eq ok *t-sym*) host-ok cli-ok factory-ok) 0 1)))
