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
  ;; Mirrors the reference implementation's STDLIB_SOURCES load order
  ;; exactly (src/lib.rs) -- Prelude files interleaved with every optional
  ;; module with_stdlib() also loads unconditionally. See sbcl/README.md
  ;; for the handful of files not yet in this list and why.
  '("00-core" "01-list" "02-cxr" "03-meta" "04-predicates" "05-math"
    "06-require" "08-vau" "12-control" "13-functional" "14-strings"
    "15-sets-hash" "16-conditions" "17-arrays" "18-format" "21-cl-compat"
    "20-condensation" "27-modules" "11-optimizer-vau" "19-call-graph"
    "07-shell" "09-lisp15" "10-testing" "22-guard" "23-match" "24-rules"
    "25-variants" "26-instrument" "28-types" "29-protocols"
    "30-text" "31-ports" "32-base64" "33-hex" "34-url" "35-json" "36-mime"
    "37-net" "38-tcp" "39-udp" "40-http" "41-os" "42-os-linux" "43-tls"
    "44-regex" "97-doc-renderer" "98-help-system" "99-help-data"))

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
