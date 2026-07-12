;; FORMAT's extended directive set, READ-LINE, WITH-OUTPUT-TO-STRING, and the
;; s-expression file round-trip helpers (issue #150).
;;
;; tests/lisp/95-stdlib-batteries.lisp's `format-basic` covers the original
;; ~a ~s ~d ~% ~~ subset; this file covers everything added on top of it.

(require 'ports)

;;; ---- ~f fixed-point --------------------------------------------------------

(deftest format-fixed-with-digits
  (assert-equal (format nil "~,4f" 3.14159) "3.1416")
  (assert-equal (format nil "~,2f" 3.14159) "3.14")
  (assert-equal (format nil "~,0f" 3.6) "4")
  (assert-equal (format nil "~,2f" 5) "5.00")
  (assert-equal (format nil "~,2f" -3.14159) "-3.14")
  (assert-equal (format nil "~,2f" 0.005) "0.01")
  ;; CL reads a bare leading digit as WIDTH; unimplemented, so it errors.
  (assert-nil (errorset '(format nil "~4f" 3.14159) nil)))

(deftest format-fixed-bare
  (assert-equal (format nil "~f" 3.5) "3.5")
  (assert-equal (format nil "~f" 3) "3.0")
  ;; Lisp's float printer never emits scientific notation (src/printer.rs),
  ;; so even a very large float renders as a (long) fixed-point string.
  (assert-equal (format nil "~f" 1.0e3) "1000.0")
  (assert-nil   (errorset '(format nil "~f" "not-a-number") nil)))

;;; ---- ~x ~o ~b radix ---------------------------------------------------------

(deftest format-radix
  (assert-equal (format nil "~x" 255) "FF")
  (assert-equal (format nil "~X" 255) "FF")
  (assert-equal (format nil "~o" 8) "10")
  (assert-equal (format nil "~b" 5) "101")
  (assert-equal (format nil "~x" 0) "0")
  (assert-equal (format nil "~x" -255) "-FF")
  (assert-nil   (errorset '(format nil "~x" 1.5) nil)))

;;; ---- ~c char -----------------------------------------------------------------

(deftest format-char
  (assert-equal (format nil "~c" "Z") "Z")
  (assert-equal (format nil "~c" (make-char 65)) "A")
  ;; The stdlib char convention (lib/14): an integer code point is a char.
  (assert-equal (format nil "~c" 65) "A")
  (assert-nil   (errorset '(format nil "~c" "ab") nil))
  (assert-nil   (errorset '(format nil "~c" 256) nil)))

;;; ---- ~& fresh-line -----------------------------------------------------------

(deftest format-fresh-line
  (assert-equal (format nil "a~&b") (concat "a" (code-char 10) "b"))
  (assert-equal (format nil "a~%~&b") (concat "a" (code-char 10) "b"))
  (assert-equal (format nil "~&x") "x"))

;;; ---- ~{ ~} iteration and ~^ --------------------------------------------------

(deftest format-iteration
  (assert-equal (format nil "~{~a~^, ~}" (list 1 2 3)) "1, 2, 3")
  (assert-equal (format nil "~{~a~^, ~}" ()) "")
  (assert-equal (format nil "[~{~a~}]" (list 1 2 3)) "[123]")
  (assert-nil   (errorset '(format nil "~{~a~}" 5) nil)))

;;; ---- unknown / unsupported directives are errors, not pass-through ---------

(deftest format-unknown-directive-errors
  (assert-nil (errorset '(format nil "~z") nil))
  (assert-nil (errorset '(format nil "~3a" 1) nil))
  (assert-nil (errorset '(format nil "~:d" 1) nil)))

;;; ---- destinations: nil / t / port -------------------------------------------

(deftest format-destination-nil-returns-string
  (assert-equal (format nil "hi ~a" 1) "hi 1"))

(deftest format-destination-t-prints-and-returns-nil
  (assert-nil (format t "")))

(deftest format-destination-port-writes-bytes
  (let ((p (ports:open-output-bytes)))
    (assert-nil (format p "hi ~a" 42))
    (assert-equal (text:utf8->string-lossy (ports:output-contents p)) "hi 42")))

(deftest format-destination-invalid-errors
  (assert-nil (errorset '(format 'bogus "hi") nil)))

;;; ---- READ-LINE ---------------------------------------------------------------

(deftest read-line-basic
  (let ((p (ports:open-input-bytes (text:string->utf8 (concat "one" (code-char 10) "two")))))
    (assert-equal (read-line p) "one")
    (assert-equal (read-line p) "two")
    (assert-nil   (read-line p))))

(deftest read-line-no-trailing-newline-still-returned-once
  (let ((p (ports:open-input-bytes (text:string->utf8 "only-line"))))
    (assert-equal (read-line p) "only-line")
    (assert-nil   (read-line p))))

;;; ---- WITH-OUTPUT-TO-STRING ---------------------------------------------------

(deftest with-output-to-string-basic
  (assert-equal
   (with-output-to-string (s)
     (ports:write-string! s "hello ")
     (format s "~a" 42))
   "hello 42"))

(deftest with-output-to-string-empty
  (assert-equal (with-output-to-string (s) nil) ""))

(deftest with-output-to-string-propagates-error
  (assert-nil (errorset '(with-output-to-string (s)
                            (ports:write-string! s "partial")
                            (error "boom"))
                         nil)))

;; READ-SEXPR-FILE / WRITE-SEXPR-FILE need READ-FS/CREATE-FS, which this
;; suite's environment does not grant (capabilities are host-only to grant;
;; see CLAUDE.md) -- see tests/test_format_io.rs for their coverage.
