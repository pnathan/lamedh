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
    "95-stdlib-batteries" "96-format-and-io" "97-common-forms"
    "97-ieee-floats" "97-no-ratios"
    "97-port-regressions" "97-printer"))

(dolist (name *test-files*)
  (let ((path (merge-pathnames (concatenate 'string name ".lisp")
                                (merge-pathnames "./" *load-pathname*))))
    (format t "~&; loading ~A~%" name)
    (run-string (uiop:read-file-string path))))

(load (merge-pathnames "host-regressions.lisp" *load-pathname*))

(defun print-framing-ok ()
  "PRINT writes the readable value then a newline -- no leading newline,
no trailing space -- matching the reference implementation (issue #537).
Checked here because PRINT writes to the host stream, which the Lamedh-level
WITH-OUTPUT-TO-STRING does not capture."
  (let* ((cases '(("(print (list nil))" . "(())")
                  ("(print nil)" . "()")
                  ("(print \"s\")" . "\"s\"")))
         (bad (loop for (src . want) in cases
                    for got = (with-output-to-string (*standard-output*)
                                (leval (lread src) *global-env*))
                    unless (string= got (format nil "~A~%" want))
                      collect (list src got))))
    (format t "~&; print framing: ~:[OK~;FAILED ~:*~S~]~%" bad)
    (null bad)))

(defun typed-array-tags-ok ()
  "Typed arrays print as the reference's <typed-array:elem:n> tag. Built
host-side, since no Lamedh constructor for them exists without #540."
  (let* ((cases (list (cons (make-array 3 :element-type '(signed-byte 64) :initial-element 0)
                            "<typed-array:int64:3>")
                      (cons (make-array 2 :element-type 'double-float :initial-element 0d0)
                            "<typed-array:float64:2>")))
         (bad (loop for (v . want) in cases
                    for got = (lprint-to-string v)
                    unless (string= got want) collect (list want got))))
    (format t "~&; typed-array tags: ~:[OK~;FAILED ~:*~S~]~%" bad)
    (null bad)))

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
      (framing (print-framing-ok))
      (typed-tags (typed-array-tags-ok))
      (cli-ok (run-cli-exit-status-test))
      (factory-ok (run-factory-bound-test)))
  (uiop:quit (if (and (eq ok *t-sym*) host-ok framing typed-tags cli-ok factory-ok) 0 1)))
