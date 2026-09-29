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
  '("10-arithmetic" "20-lists" "30-predicates" "40-list-processing"
    "50-strings-symbols" "51-string-completions" "52-text-module"
    "60-special-forms" "65-loops" "70-hash-and-plist" "90-bitwise"
    "95-stdlib-batteries" "96-format-and-io" "97-recursion-limit"))

(dolist (name *test-files*)
  (let ((path (merge-pathnames (concatenate 'string name ".lisp")
                                (merge-pathnames "./" *load-pathname*))))
    (format t "~&; loading ~A~%" name)
    (run-string (uiop:read-file-string path))))

;;; ---- host-boundary checks (#532) -----------------------------------------
;;; Run the CLI (TOPLEVEL) in a child SBCL on a script whose recursion is
;;; *not* caught by Lamedh code, and check the process exits cleanly with
;;; status 1 and the error on stderr, instead of dying in SBCL's debugger.

(defparameter *host-failures* 0)

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

(defun host-check (name ok)
  (format t "~&; host check ~A: ~:[FAIL~;ok~]~%" name ok)
  (unless ok (incf *host-failures*)))

(multiple-value-bind (code out err)
    (run-cli-child cl-user::*lamedh-control-stack-size*
                   "(defun rec (n) (+ 1 (rec n)))
                    (print (handler-case (rec 1) (error (e) 'caught)))
                    (rec 1)
                    (print 'unreachable)")
  (host-check "cli-caught-then-uncaught-overflow"
              (and (eql code 1)
                   (search "CAUGHT" out)
                   (not (search "UNREACHABLE" out))
                   (search "lamedh: recursion limit exceeded (10000 eval frames)" err))))

;; STORAGE-CONDITION backstop: with the guard raised out of reach, exhausting
;; a modest control stack is still a catchable Lamedh error, and uncaught it
;; is a clean exit 1 at the CLI boundary.
(multiple-value-bind (code out err)
    (run-cli-child "16MB"
                   "(defun rec (n) (+ 1 (rec n)))
                    (print (handler-case (rec 1) (error (e) 'caught)))
                    (rec 1)"
                   "(setf lamedh-rt::*eval-depth-limit* most-positive-fixnum)")
  (host-check "cli-storage-condition-mapped"
              (and (eql code 1)
                   (search "CAUGHT" out)
                   (search "lamedh: Control stack exhausted" err))))

(let ((ok (leval (lread "(run-tests)") *global-env*)))
  (uiop:quit (if (and (eq ok *t-sym*) (zerop *host-failures*)) 0 1)))
