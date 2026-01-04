;;; Dynamic Variables Test Suite
;;; Tests for DEFDYNAMIC and dynamic scoping

;;; Test 1: Basic DEFDYNAMIC
(defdynamic *test-var* 42)
(print *test-var*)  ; Should print: 42

;;; Test 2: DEFVAR alias
(defvar *another-var* 100)
(print *another-var*)  ; Should print: 100

;;; Test 3: Dynamic binding with LET
(defdynamic *x* 'global)

(defun get-x () *x*)

(print (get-x))  ; Should print: GLOBAL

(let ((*x* 'local))
  (print (get-x)))  ; Should print: LOCAL

(print (get-x))  ; Should print: GLOBAL

;;; Test 4: Nested dynamic bindings
(defdynamic *level* 0)

(defun show-level ()
  (print *level*))

(show-level)  ; Should print: 0

(let ((*level* 1))
  (progn
    (show-level)  ; Should print: 1
    (let ((*level* 2))
      (progn
        (show-level)  ; Should print: 2
        (let ((*level* 3))
          (show-level))))))  ; Should print: 3

(show-level)  ; Should print: 0

;;; Test 5: SETQ on dynamic variable
(defdynamic *counter* 0)

(defun increment ()
  (setq *counter* (+ *counter* 1)))

(increment)
(print *counter*)  ; Should print: 1

(let ((*counter* 10))
  (progn
    (increment)
    (increment)
    (print *counter*)))  ; Should print: 12

(print *counter*)  ; Should print: 1 (not 12)

;;; Test 6: Multiple dynamic variables
(defdynamic *debug* nil)
(defdynamic *verbose* nil)

(defun log-msg (msg)
  (if *debug*
      (if *verbose*
          (print (concat "VERBOSE: " msg))
          (print msg))
      nil))

(log-msg "Test 1")  ; Should print nothing (returns nil)

(let ((*debug* t))
  (log-msg "Test 2"))  ; Should print: Test 2

(let ((*debug* t) (*verbose* t))
  (log-msg "Test 3"))  ; Should print: VERBOSE: Test 3

;;; Test 7: Lexical vs Dynamic - The Classic Test
(def lexical-x 'lexical-global)
(defdynamic *dynamic-x* 'dynamic-global)

(def get-lexical (lambda () lexical-x))
(def get-dynamic (lambda () *dynamic-x*))

(defun test-both ()
  (progn
    (def lexical-x 'lexical-local)  ; Shadows lexically
    (let ((*dynamic-x* 'dynamic-local))  ; Shadows dynamically
      (progn
        (print (get-lexical))  ; Should print: LEXICAL-GLOBAL
        (print (get-dynamic)))))) ; Should print: DYNAMIC-LOCAL

(test-both)

;;; Test 8: Dynamic variable in recursive function
(defdynamic *depth* 0)

(defun recursive-print (n)
  (if (zerop n)
      (print "Done")
      (let ((*depth* (+ *depth* 1)))
        (progn
          (print *depth*)
          (recursive-print (- n 1))))))

(recursive-print 3)
; Should print: 1, 2, 3, Done

;;; Test 9: Documentation string
(defdynamic *documented* 42 "This is a test variable")
(print (getp '*documented* "docstring"))
; Should print: "This is a test variable"

;;; Test 10: Warning for missing earmuffs (output goes to stderr)
(defdynamic bad-name 123)
; Should print warning: Warning: Dynamic variable 'BAD-NAME' does not follow naming convention *NAME*
; Should still work though
(print bad-name)  ; Should print: 123

;;; Test 11: Nested function calls with dynamic binding
(defdynamic *context* 'global)

(defun inner () *context*)

(defun middle ()
  (inner))

(defun outer ()
  (let ((*context* 'outer-bound))
    (middle)))

(print (outer))  ; Should print: OUTER-BOUND
(print (inner))  ; Should print: GLOBAL

;;; Test 12: Dynamic variables with multiple functions
(defdynamic *multiplier* 1)

(defun multiply (x)
  (* x *multiplier*))

(defun compute-with-multiplier (x m)
  (let ((*multiplier* m))
    (multiply x)))

(print (multiply 5))                    ; Should print: 5
(print (compute-with-multiplier 5 10))  ; Should print: 50
(print (multiply 5))                    ; Should print: 5

(print "All tests completed!")
