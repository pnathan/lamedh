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
  (let ((h (make-hash-table)))
    (sethash h 'a 1)
    (assert-equal (prin1-to-string h) "<hash-table>")))

;;; ---- arrays print their contents (KERNEL Part III, #527/#594/#607) ---------------
;;; Expected strings mirror ../../tests/test_array_printing.rs (Rust reference).

(deftest array-prints-its-elements
  (assert-equal (prin1-to-string (list->array (list 1 2 3))) "#(1 2 3)")
  (assert-equal (prin1-to-string (array 3)) "#(() () ())")
  (assert-equal (prin1-to-string (array 0)) "#()")
  (assert-equal (prin1-to-string (list->array (list "a" 'b 'c' 1.5 (list 1 2) (list->array (list 4)))))
                "#(\"a\" B 'c' 1.5 (1 2) #(4))")
  (assert-equal (prin1-to-string (list (array 2) "q")) "(#(() ()) \"q\")")
  ;; elements stay readable even under PRINC, as in the Rust printer
  (assert-equal (princ-to-string (list->array (list "a" 'c'))) "#(\"a\" 'c')"))

(deftest array-literal-reads-and-self-evaluates
  (assert-equal (prin1-to-string '#(1 (a b) "s")) "#(1 (A B) \"s\")")
  (assert-equal (prin1-to-string #(1 2 3)) "#(1 2 3)")
  (assert-true (arrayp #()))
  (assert-equal (array-length* #(a b c d)) 4)
  ;; elements are read, not evaluated
  (assert-equal (prin1-to-string (fetch #((+ 1 2)) 0)) "(+ 1 2)")
  (assert-true (arrayp (fetch #(#(1)) 0)))
  (assert-equal (array-length* #()) 0))

(deftest array-print-read-round-trips
  (let* ((a (list->array (list 1 "x" (list 2 3) (list->array (list 4)))))
         (b (read-from-string (prin1-to-string a))))
    (assert-true (arrayp b))
    (assert-equal (array->list (fetch b 3)) '(4))
    (assert-equal (prin1-to-string b) "#(1 \"x\" (2 3) #(4))")))

(deftest array-literal-rejects-malformed-input
  (dolist (bad '("'#(1 . 2)" "'# (1 2)" "'#(1 2" "#(1 . 2)"))
    (assert-true (handler-case (progn (read-from-string bad) nil) (error (e) t)))))

(deftest array-long-is-abridged-with-unreadable-marker
  (assert-equal (prin1-to-string (list->array (iota 150)))
                (string-join (list "#(" (string-join (map prin1-to-string (iota 100)) " ") " #<...50 more>)") ""))
  ;; exactly at the limit: nothing abridged
  (assert-equal (prin1-to-string (list->array (iota 100)))
                (string-join (list "#(" (string-join (map prin1-to-string (iota 100)) " ") ")") ""))
  ;; the marker never reads back as a shorter array
  (assert-true (handler-case (progn (read-from-string (prin1-to-string (list->array (iota 150)))) nil)
                 (error (e) t))))

(deftest array-circular-prints-back-reference
  (assert-equal (prin1-to-string (let ((a (array 2))) (store a 0 a) a))
                "#(#<circular-array> ())")
  (assert-equal (prin1-to-string (let ((a (array 2))) (store a 1 (list 1 a)) a))
                "#(() (1 #<circular-array>))")
  ;; the same array twice, but not nested in itself, is not circular
  (assert-equal (prin1-to-string (let ((a #(1))) (list->array (list a a))))
                "#(#(1) #(1))")
  ;; the in-progress marker is released after a circular print
  (let ((a (array 1)))
    (store a 0 a)
    (prin1-to-string a)
    (assert-equal (prin1-to-string a) "#(#<circular-array>)")))

(deftest typed-array-prints-element-type-and-contents
  (assert-equal (prin1-to-string (let ((a (typed-array 3 'int64))) (store a 1 7) a))
                "#<typed-array:int64 0 7 0>")
  (assert-equal (prin1-to-string (typed-array 2 'float64)) "#<typed-array:float64 0.0 0.0>")
  (assert-equal (prin1-to-string (typed-array 0 'int64)) "#<typed-array:int64>")
  (assert-equal (prin1-to-string (typed-array 0 'float64)) "#<typed-array:float64>")
  (let ((long (prin1-to-string (typed-array 101 'int64))))
    (assert-equal (subseq long (- (length long) 16)) " 0 #<...1 more>>")))
