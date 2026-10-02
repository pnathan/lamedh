;; Printer conformance with the reference implementation (issue #537).
;;
;; Port-specific: unlike the other files here, this one is NOT a copy of
;; ../tests/lisp/. Expected strings were taken from the Rust `lamedh`
;; binary's output for the same expressions.

;;; ---- the empty list is (), never NIL ---------------------------------------

(deftest printer-nil-prints-as-empty-list
  (assert-equal (prin1-to-string nil) "()")
  (assert-equal (princ-to-string nil) "()")
  (assert-equal (prin1-to-string '()) "()"))

(deftest printer-nil-inside-lists
  (assert-equal (prin1-to-string (list nil)) "(())")
  (assert-equal (prin1-to-string '(1 (2 nil) () 3)) "(1 (2 ()) () 3)")
  (assert-equal (prin1-to-string '(nil . nil)) "(())")
  (assert-equal (prin1-to-string '(a . b)) "(A . B)")
  (assert-equal (princ-to-string '(nil)) "(())"))

(deftest printer-nil-through-format
  (assert-equal (format nil "~a ~s" nil '(nil)) "() (())"))

;;; ---- other atoms are unchanged ----------------------------------------------

(deftest printer-atoms
  (assert-equal (prin1-to-string t) "T")
  (assert-equal (prin1-to-string 'foo) "FOO")
  (assert-equal (prin1-to-string "hi \"x\"") "\"hi \\\"x\\\"\"")
  (assert-equal (prin1-to-string 3.5) "3.5")
  (assert-equal (prin1-to-string 100000.0) "100000.0")
  (assert-equal (prin1-to-string -0.25) "-0.25"))

;;; ---- opaque tags match the reference printer --------------------------------

(deftest printer-opaque-tags
  (assert-equal (prin1-to-string (array 3)) "<array:3>")
  (assert-equal (prin1-to-string (list (array 2) "q")) "(<array:2> \"q\")")
  (let ((h (make-hash-table)))
    (sethash h 'a 1)
    (assert-equal (prin1-to-string h) "<hash-table>")))
