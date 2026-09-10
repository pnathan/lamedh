;;;; package.lisp -- package definitions for the SBCL port of Lamedh.
;;;;
;;;; LAMEDH-RT (runtime) holds the reader, evaluator, environment, and
;;;; builtins.  Interned Lamedh symbols themselves live in the separate
;;;; LAMEDH package (bare, one symbol per user-visible name) so that a
;;;; symbol such as LAMEDH::IF can carry a value cell via SYMBOL-VALUE
;;;; without colliding with CL::IF.

(defpackage #:lamedh
  (:use)
  (:documentation "Home package for interned Lamedh symbols (bare namespace)."))

(defpackage #:lamedh-rt
  (:use #:cl)
  (:export #:run-repl #:run-file #:run-string #:make-global-environment
           #:lread #:lread-all #:leval #:lprint #:lprint-to-string
           #:*standard-lamedh-environment* #:lamedh-error #:lamedh-error-datum
           #:enable-feature #:disable-feature #:enable-all-features #:toplevel))
