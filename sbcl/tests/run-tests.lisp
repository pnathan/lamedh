;;;; run-tests.lisp -- load every ported *_test.lisp fixture under sbcl/tests
;;;; (Lamedh source, copied from the reference implementation's tests/lisp/)
;;;; and run the suite via the bootstrapped (run-tests) function from
;;;; lib/10-testing.lisp.

(require :asdf)

;;; The eval-depth guard (10,000 nested LEVAL frames, #532) is only reachable
;;; before SBCL's own control-stack guard page on a large stack; SBCL's default
;;; (2 MB) is far too small, and overflowing it can be fatal rather than a
;;; condition. A thread cannot be given a larger stack than the runtime's
;;; --control-stack-size, so re-run this file under the documented size unless
;;; already doing so.
(defparameter cl-user::*lamedh-control-stack-size* "512MB")
(unless (sb-ext:posix-getenv "LAMEDH_TESTS_LARGE_STACK")
  (let ((p (sb-ext:run-program sb-ext:*runtime-pathname*
                               (list "--control-stack-size" cl-user::*lamedh-control-stack-size*
                                     "--non-interactive" "--load" (namestring *load-truename*))
                               :environment (cons "LAMEDH_TESTS_LARGE_STACK=1" (sb-ext:posix-environ))
                               :input t :output t :error t)))
    (sb-ext:exit :code (sb-ext:process-exit-code p) :abort t)))

(asdf:load-asd (merge-pathnames "../lamedh.asd" *load-pathname*))
(asdf:load-system :lamedh)

(in-package #:lamedh-rt)

(enable-all-features) ; a trusted local test run, like the CLI's default

(defparameter *test-files*
  '("10-arithmetic" "11-mod-euclidean" "20-lists" "30-predicates"
    "40-list-processing" "50-strings-symbols" "51-string-completions"
    "52-text-module" "60-special-forms" "65-loops" "70-hash-and-plist"
    "80-kernel-conformance" "90-bitwise" "95-stdlib-batteries"
    "96-format-and-io" "97-common-forms" "97-ieee-floats" "97-no-ratios"
    "97-port-regressions" "97-printer" "97-recursion-limit"
    "97-reference-builtins"))

(dolist (name *test-files*)
  (let ((path (merge-pathnames (concatenate 'string name ".lisp")
                                (merge-pathnames "./" *load-pathname*))))
    (format t "~&; loading ~A~%" name)
    (run-string (uiop:read-file-string path))))

;;; ---- host-boundary checks (#532) -----------------------------------------
;;; Run the CLI (TOPLEVEL) in a child SBCL on a script whose recursion is
;;; *not* caught by Lamedh code, and check the process exits cleanly with
;;; status 1 and the error on stderr, instead of dying in SBCL's debugger.

(defun run-cli-child (stack-size script-source &rest evals)
  "Returns (VALUES EXIT-CODE STDOUT STDERR)."
  (let ((script (uiop:with-temporary-file (:pathname p :stream s :keep t :type "lisp")
                  (write-string script-source s) p))
        (asd (namestring (merge-pathnames "../lamedh.asd" *load-truename*))))
    (unwind-protect
         (multiple-value-bind (out err code)
             (uiop:run-program
              (append (list (namestring sb-ext:*runtime-pathname*) "--control-stack-size" stack-size
                            "--noinform" "--non-interactive"
                            "--eval" "(require :asdf)"
                            "--eval" (format nil "(asdf:load-asd ~S)" asd)
                            "--eval" "(asdf:load-system :lamedh)")
                      (loop for e in evals append (list "--eval" e))
                      (list "--eval" "(lamedh-rt:toplevel)" (namestring script)))
              :output :string :error-output :string :ignore-error-status t)
           (values code out err))
      (delete-file script))))

(defun recursion-limit-check (name ok)
  (format t "~&; recursion-limit check ~A: ~:[FAIL~;ok~]~%" name ok)
  ok)

(defun run-recursion-limit-checks ()
  "True when both CLI host-boundary checks pass."
  (let ((results '()))
    (multiple-value-bind (code out err)
        (run-cli-child cl-user::*lamedh-control-stack-size*
                       "(defun rec (n) (+ 1 (rec n)))
                        (print (handler-case (rec 1) (error (e) 'caught)))
                        (rec 1)
                        (print 'unreachable)")
      (push (recursion-limit-check
             "cli-caught-then-uncaught-overflow"
             (and (eql code 1)
                  (search "CAUGHT" out)
                  (not (search "UNREACHABLE" out))
                  (search "lamedh: recursion limit exceeded (10000 eval frames)" err)))
            results))
    ;; STORAGE-CONDITION backstop: with the guard raised out of reach,
    ;; exhausting a modest control stack is still a catchable Lamedh error,
    ;; and uncaught it is a clean exit 1 at the CLI boundary.
    (multiple-value-bind (code out err)
        (run-cli-child "16MB"
                       "(defun rec (n) (+ 1 (rec n)))
                        (print (handler-case (rec 1) (error (e) 'caught)))
                        (rec 1)"
                       "(setf lamedh-rt::*eval-depth-limit* most-positive-fixnum)")
      (push (recursion-limit-check
             "cli-storage-condition-mapped"
             (and (eql code 1)
                  (search "CAUGHT" out)
                  (search "lamedh: Control stack exhausted" err)))
            results))
    (every #'identity results)))

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
  "Typed arrays print as the reference's <typed-array:elem:n> tag."
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
      (recursion-ok (run-recursion-limit-checks))
      (factory-ok (run-factory-bound-test))
      (cli-ok (run-cli-exit-status-test)))
  (uiop:quit (if (and (eq ok *t-sym*) host-ok framing typed-tags recursion-ok factory-ok cli-ok)
                 0 1)))
