;;; Dynamic Variables Examples for Lamedh
;;;
;;; This file demonstrates practical uses of dynamic variables (special variables).

;;; ============================================================================
;;; EXAMPLE 1: Debugging/Tracing System
;;; ============================================================================

(print "=== Example 1: Debug/Trace System ===")

(defdynamic *trace-enabled* nil "Enable function call tracing")
(defdynamic *trace-depth* 0 "Current trace indentation depth")

(defun trace-enter (name)
  (if *trace-enabled*
      (progn (print "ENTER:") (print name))
      nil))

(defun trace-exit (name result)
  (if *trace-enabled*
      (progn (print "EXIT:") (print name) (print "=>") (print result))
      nil))

;; A traced version of factorial
(defun traced-factorial (n)
  (progn
    (trace-enter "factorial")
    (let ((*trace-depth* (+ *trace-depth* 1)))
      (let ((result (if (zerop n)
                        1
                        (* n (traced-factorial (- n 1))))))
        (progn
          (trace-exit "factorial" result)
          result)))))

;; Normal call - no tracing
(print "Factorial without tracing:")
(print (traced-factorial 5))

;; With tracing enabled
(print "Factorial with tracing:")
(let ((*trace-enabled* t))
  (traced-factorial 3))

;;; ============================================================================
;;; EXAMPLE 2: Configuration Context
;;; ============================================================================

(print "=== Example 2: Configuration Context ===")

(defdynamic *output-format* 'text "Output format: text, json, or xml")
(defdynamic *precision* 2 "Decimal precision for numbers")
(defdynamic *verbose* nil "Include extra information")

(defun format-value (value)
  (cond
    ((eq *output-format* 'text)
     (print value))
    ((eq *output-format* 'json)
     (progn (print "{ value:") (print value) (print "}")))
    (t
     (progn (print "<value>") (print value) (print "</value>")))))

(defun generate-report (data)
  (progn
    (if *verbose*
        (print "=== Detailed Report ===")
        nil)
    (format-value data)
    (if *verbose*
        (progn (print "Format:") (print *output-format*))
        nil)))

;; Default output
(print "Default format:")
(generate-report 42)

;; JSON format
(print "JSON format:")
(let ((*output-format* 'json))
  (generate-report 42))

;; XML format with verbose
(print "XML verbose:")
(let ((*output-format* 'xml) (*verbose* t))
  (generate-report 42))

;;; ============================================================================
;;; EXAMPLE 3: Error Handling Context
;;; ============================================================================

(print "=== Example 3: Error Handling Context ===")

(defdynamic *error-handler* nil "Custom error handler function")
(defdynamic *error-context* nil "Context info for error messages")

(defun safe-divide (a b)
  (if (zerop b)
      (if *error-handler*
          (funcall *error-handler* *error-context*)
          (progn (print "ERROR: Division by zero") 0))
      (quotient a b)))

(defun log-error (context)
  (progn
    (print "ERROR logged in context:")
    (print context)
    0))  ; Return 0 as default

;; With logging handler
(let ((*error-handler* #'log-error) (*error-context* 'calculation-1))
  (print (safe-divide 10 0)))  ; Logs error, returns 0

;; Different context - no error
(let ((*error-handler* #'log-error) (*error-context* 'calculation-2))
  (print (safe-divide 10 2)))  ; Returns 5

;;; ============================================================================
;;; EXAMPLE 4: Accumulator Pattern
;;; ============================================================================

(print "=== Example 4: Accumulator Pattern ===")

(defdynamic *collected-items* nil "List to collect items into")

(defun collect (item)
  (setq *collected-items* (cons item *collected-items*)))

(defun walk-and-collect (tree)
  (cond
    ((null tree) nil)
    ((atom tree) (collect tree))
    (t (progn
         (walk-and-collect (car tree))
         (walk-and-collect (cdr tree))))))

(defun collect-all-atoms (tree)
  (let ((*collected-items* nil))
    (progn
      (walk-and-collect tree)
      (reverse *collected-items*))))

(print "Collecting atoms from (a (b c) d):")
(print (collect-all-atoms '(a (b c) d)))

;;; ============================================================================
;;; EXAMPLE 5: Customizable Comparisons
;;; ============================================================================

(print "=== Example 5: Customizable Comparison ===")

(defdynamic *compare-fn* #'lessp "Comparison function for ordering")

(defun compare-items (a b)
  (funcall *compare-fn* a b))

(defun insert-sorted (item lst)
  (cond
    ((null lst) (list item))
    ((compare-items item (car lst)) (cons item lst))
    (t (cons (car lst) (insert-sorted item (cdr lst))))))

(defun sort-list (lst)
  (if (null lst)
      nil
      (insert-sorted (car lst) (sort-list (cdr lst)))))

;; Ascending sort (default)
(print "Ascending sort:")
(print (sort-list '(3 1 4 1 5)))

;; Descending sort
(print "Descending sort:")
(let ((*compare-fn* #'greaterp))
  (print (sort-list '(3 1 4 1 5))))

;;; ============================================================================
;;; EXAMPLE 6: State Machine with Dynamic Context
;;; ============================================================================

(print "=== Example 6: State Machine ===")

(defdynamic *machine-state* 'idle "Current state machine state")

(defun transition (new-state)
  (progn
    (print "Transition:")
    (print *machine-state*)
    (print "->")
    (print new-state)
    (setq *machine-state* new-state)))

(defun process-event (event)
  (cond
    ((eq *machine-state* 'idle)
     (if (eq event 'start)
         (transition 'running)
         (print "Ignoring event in idle state")))
    ((eq *machine-state* 'running)
     (cond
       ((eq event 'pause) (transition 'paused))
       ((eq event 'stop) (transition 'idle))
       (t (print "Processing..."))))
    ((eq *machine-state* 'paused)
     (if (eq event 'resume)
         (transition 'running)
         (print "Paused, waiting for resume")))
    (t (print "Unknown state"))))

(print "State machine events:")
(process-event 'start)
(process-event 'pause)
(process-event 'resume)
(process-event 'stop)

;; Isolated state machine
(print "Isolated state machine:")
(let ((*machine-state* 'idle))
  (progn
    (process-event 'start)
    (process-event 'pause)))

(print "Outer state after isolated run:")
(print *machine-state*)  ; Still idle!

(print "=== All examples completed! ===")
