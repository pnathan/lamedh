;;;; Conformance runner for the SBCL port.
;;;;
;;;; Run from the repo root:
;;;;   sbcl --non-interactive --load conformance/ports/run-sbcl.lisp
;;;;
;;;; Set CONFORMANCE_VERBOSE=1 to print src/want/got for known failures too.
;;;; Prints PASS/FAIL per case and a summary. Exit status:
;;;;   0  every case passes or is allowlisted-and-failing
;;;;   1  a case fails and is not in sbcl-known-divergences.txt (regression),
;;;;      or an allowlisted case now passes (stale entry), or the case count
;;;;      or an ID is wrong.

(require :asdf)

(defparameter cl-user::*ports-dir*
  (make-pathname :name nil :type nil :defaults *load-truename*))
(defparameter cl-user::*repo-root*
  (merge-pathnames "../../" cl-user::*ports-dir*))

(asdf:load-asd (truename (merge-pathnames "sbcl/lamedh.asd" cl-user::*repo-root*)))
(handler-bind ((warning #'muffle-warning)) (asdf:load-system :lamedh))
(in-package :lamedh-rt)

(defvar *expected-total* 144)

(defun repo-path (rel)
  (merge-pathnames rel cl-user::*repo-root*))

(defun read-allowlist (path)
  (let ((ids '()))
    (with-open-file (s path)
      (loop for line = (read-line s nil) while line
            do (let ((l (string-trim '(#\Space #\Tab #\Return) line)))
                 (unless (or (string= l "") (char= (char l 0) #\#))
                   (push l ids)))))
    ids))

(defun run-case (src)
  (handler-case (lprint-to-string (run-string src) t)
    ((or error storage-condition) () "ERROR")
    (condition () "ERROR")))

(defun detail (src expect got)
  (format t "  src:  ~A~%  want: ~A~%  got:  ~A~%" src expect got))

(defun main ()
  (let* ((verbose (sb-ext:posix-getenv "CONFORMANCE_VERBOSE"))
         (allow (read-allowlist (repo-path "conformance/ports/sbcl-known-divergences.txt")))
         (files (sort (mapcar #'namestring
                              (directory (repo-path "conformance/cases/*.lisp")))
                      #'string<))
         (seen (make-hash-table :test 'equal))
         (pass 0) (allowed 0) (regress '()) (stale '()) (dups '()) (total 0)
         (skipped 0))
    (dolist (f files)
      (if (search "/99-" f)
          (incf skipped (length (lread-all (uiop:read-file-string f))))
          (dolist (form (lread-all (uiop:read-file-string f)))
            (destructuring-bind (id spec src expect) (cdr form)
              (declare (ignore spec))
              (incf total)
              (when (gethash id seen) (push id dups))
              (setf (gethash id seen) t)
              (let* ((got (run-case src))
                     (ok (string= got expect))
                     (listed (member id allow :test #'string=)))
                (cond ((and ok listed) (push id stale)
                       (format t "STALE ~A (allowlisted but now passes)~%" id))
                      (ok (incf pass) (format t "PASS ~A~%" id))
                      (listed (incf allowed) (format t "KNOWN-FAIL ~A~%" id)
                       (when verbose (detail src expect got)))
                      (t (push id regress) (format t "FAIL ~A~%" id)
                         (detail src expect got))))))))
    (dolist (id allow)
      (unless (gethash id seen)
        (push id stale)
        (format t "STALE ~A (allowlisted but no such case)~%" id)))
    (format t "~&SBCL-CONFORMANCE total=~A pass=~A known-fail=~A regress=~A stale=~A skipped-disputed=~A~%"
            total pass allowed (length regress) (length stale) skipped)
    (when dups (format t "DUPLICATE IDs: ~{~A~^ ~}~%" dups))
    (when (/= total *expected-total*)
      (format t "case count ~A, expected ~A~%" total *expected-total*))
    (finish-output)
    (sb-ext:exit :code (if (or regress stale dups (/= total *expected-total*)) 1 0))))

(main)
