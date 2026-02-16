;;; Exhaustive Dynamic Variables Test Suite
;;; This file contains comprehensive tests for dynamic scoping in Lamedh

;;; ============================================================================
;;; PART 1: FUNDAMENTAL CONCEPTS
;;; ============================================================================

(print "=== Test 1.1: Basic lexical vs dynamic ===")

(def lex-var 'lexical-global)
(defdynamic *dyn-var* 'dynamic-global)

(def get-lex (lambda () lex-var))
(def get-dyn (lambda () *dyn-var*))

;; From global context
(print (get-lex))  ; => LEXICAL-GLOBAL
(print (get-dyn))  ; => DYNAMIC-GLOBAL

;; Shadow the variables and call
(defun test-scoping ()
  (progn
    (def lex-var 'lexical-local)
    (let ((*dyn-var* 'dynamic-local))
      (progn
        (print (get-lex))  ; => LEXICAL-GLOBAL (lexical scoping)
        (print (get-dyn))  ; => DYNAMIC-LOCAL (dynamic scoping)
        ))))

(test-scoping)

;;; Test 1.2: Dynamic variables follow the call stack
(print "=== Test 1.2: Call stack demonstration ===")

(defdynamic *trace* nil)

(defun level-3 ()
  (print *trace*))

(defun level-2 ()
  (level-3))

(defun level-1 ()
  (let ((*trace* 'bound-at-level-1))
    (level-2)))

(level-1)  ; => BOUND-AT-LEVEL-1
(level-3)  ; => NIL (back to global)

;;; ============================================================================
;;; PART 2: SETQ BEHAVIOR
;;; ============================================================================

(print "=== Test 2: SETQ on dynamic variables ===")

(defdynamic *mutable* 0)

(defun modify-dynamic ()
  (setq *mutable* (+ *mutable* 1)))

;; Modify global
(modify-dynamic)
(print *mutable*)  ; => 1

;; Modify local binding - global unchanged
(let ((*mutable* 100))
  (progn
    (modify-dynamic)
    (modify-dynamic)
    (print *mutable*)))  ; => 102

(print *mutable*)  ; => 1 (global was not touched!)

;;; ============================================================================
;;; PART 3: NESTED DYNAMIC BINDINGS
;;; ============================================================================

(print "=== Test 3: Nested bindings form a stack ===")

(defdynamic *stack-demo* 'bottom)

(defun show-stack () *stack-demo*)

(let ((*stack-demo* 'layer-1))
  (progn
    (print (show-stack))  ; => LAYER-1
    (let ((*stack-demo* 'layer-2))
      (progn
        (print (show-stack))  ; => LAYER-2
        (let ((*stack-demo* 'layer-3))
          (print (show-stack)))  ; => LAYER-3
        (print (show-stack))))  ; => LAYER-2 (popped back)
    (print (show-stack))))  ; => LAYER-1 (popped back)

(print (show-stack))  ; => BOTTOM

;;; ============================================================================
;;; PART 4: RECURSION WITH DYNAMIC VARIABLES
;;; ============================================================================

(print "=== Test 4: Recursion with dynamic variables ===")

(defdynamic *recursion-depth* 0)
(defdynamic *max-seen-depth* 0)

(defun recursive-depth-tracker (n)
  (if (zerop n)
      *max-seen-depth*
      (let ((*recursion-depth* (+ *recursion-depth* 1)))
        (progn
          (if (greaterp *recursion-depth* *max-seen-depth*)
              (setq *max-seen-depth* *recursion-depth*)
              nil)
          (recursive-depth-tracker (- n 1))))))

(print (recursive-depth-tracker 5))  ; => 5
(print *recursion-depth*)  ; => 0 (unwound)
(print *max-seen-depth*)   ; => 5

;;; ============================================================================
;;; PART 5: MULTIPLE DYNAMIC VARIABLES
;;; ============================================================================

(print "=== Test 5: Multiple dynamic variables ===")

(defdynamic *width* 80)
(defdynamic *height* 24)
(defdynamic *color* 'white)

(defun describe-screen ()
  (progn
    (print *width*)
    (print *height*)
    (print *color*)))

;; Override all
(let ((*width* 40) (*height* 12) (*color* 'green))
  (describe-screen))  ; 40, 12, GREEN

;; Back to defaults
(describe-screen)  ; 80, 24, WHITE

;;; ============================================================================
;;; PART 6: HIGHER-ORDER FUNCTIONS
;;; ============================================================================

(print "=== Test 6: Higher-order functions ===")

(defdynamic *transform* (lambda (x) x))  ; identity by default

(defun apply-transform (x)
  (funcall *transform* x))

(print (apply-transform 5))  ; => 5

(let ((*transform* (lambda (x) (* x 2))))
  (print (apply-transform 5)))  ; => 10

(let ((*transform* (lambda (x) (+ x 100))))
  (print (apply-transform 5)))  ; => 105

;;; ============================================================================
;;; PART 7: CLOSURES AND DYNAMIC VARIABLES
;;; ============================================================================

(print "=== Test 7: Closures and dynamic variables ===")

(defdynamic *captured-or-not* 'global)

;; Create a closure that references the dynamic variable
(def make-printer
  (lambda ()
    (lambda () *captured-or-not*)))

(def printer1 (make-printer))

(print (funcall printer1))  ; => GLOBAL

(let ((*captured-or-not* 'local))
  (print (funcall printer1)))  ; => LOCAL (dynamic lookup!)

;;; ============================================================================
;;; PART 8: PRACTICAL PATTERNS - Debug flag
;;; ============================================================================

(print "=== Test 8: Debug flag pattern ===")

(defdynamic *debug-mode* nil)

(defun complex-calculation (x)
  (progn
    (if *debug-mode*
        (progn (print "debug: calculating") (print x))
        nil)
    (* x x)))

(print (complex-calculation 5))  ; No debug output, returns 25

(let ((*debug-mode* t))
  (print (complex-calculation 5)))  ; Prints debug info, returns 25

;;; ============================================================================
;;; PART 9: EDGE CASES
;;; ============================================================================

(print "=== Test 9: Edge cases ===")

;;; 9.1: Redefining a dynamic variable
(defdynamic *redefined* 'first)
(print *redefined*)  ; => FIRST
(defdynamic *redefined* 'second)
(print *redefined*)  ; => SECOND

;;; 9.2: Deeply nested dynamic bindings (limited depth to avoid stack overflow)
(defdynamic *deep* 0)

(defun deep-nest (n)
  (if (zerop n)
      *deep*
      (let ((*deep* (+ *deep* 1)))
        (deep-nest (- n 1)))))

(print (deep-nest 10))  ; => 10

;;; ============================================================================
;;; PART 10: GOTCHAS
;;; ============================================================================

(print "=== Test 10: Common gotchas ===")

;;; Gotcha 10.1: Dynamic variables are looked up at call time, not definition time
(defdynamic *gotcha1* 'outer)

;; Define function globally
(defun gotcha1-fn () *gotcha1*)

;; Call from global context
(print (gotcha1-fn))  ; => OUTER

;; Call from inside LET - sees the LET's binding!
(let ((*gotcha1* 'inner))
  (print (gotcha1-fn)))  ; => INNER

;; Back to global context
(print (gotcha1-fn))  ; => OUTER

;;; Gotcha 10.2: Dynamic variable in loop body accumulates per iteration
(defdynamic *loop-var* 0)

(defun loop-test (n)
  (if (zerop n)
      *loop-var*
      (let ((*loop-var* (+ *loop-var* n)))
        (loop-test (- n 1)))))

;; 0 + 3 = 3, then 3 + 2 = 5, then 5 + 1 = 6
(print (loop-test 3))  ; => 6

;;; ============================================================================
;;; FINAL: Summary
;;; ============================================================================

(print "=== All exhaustive tests completed! ===")
