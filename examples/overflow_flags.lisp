;;; Overflow Flag Demonstration
;;; Shows how to use dynamic condition flags for interactive error handling

;; Example 1: Simple overflow detection
(PROGN
  (PRINT "Example 1: Basic overflow detection")
  (CLEAR-ALL-FLAGS)
  (DEF result (PLUS 9223372036854775807 1))
  (PRINT result)
  (IF (FLAG-SET-P 'OVERFLOW)
      (PRINT "Warning: Overflow occurred in addition!")
      (PRINT "No overflow")))

;; Example 2: Conditional computation based on overflow
(PROGN
  (PRINT "Example 2: Conditional logic")
  (CLEAR-ALL-FLAGS)
  (DEF x (TIMES 9223372036854775807 2))
  (COND
    ((FLAG-SET-P 'OVERFLOW)
     (PRINT "Overflow! Using safe fallback value...")
     (DEF x 0))
    (T (PRINT "Computation succeeded")))
  x)

;; Example 3: Multiple operations with flag checking
(PROGN
  (PRINT "Example 3: Chain of operations")
  (CLEAR-ALL-FLAGS)
  (DEF a (PLUS 100 200 300))
  (DEF b (TIMES a 1000000000))
  (DEF c (PLUS b 5000000000000))
  (IF (FLAG-SET-P 'OVERFLOW)
      (PRINT "One or more operations overflowed")
      (PRINT "All operations within range"))
  c)

;; Example 4: Division overflow (MIN / -1)
(PROGN
  (PRINT "Example 4: Division edge case")
  (CLEAR-ALL-FLAGS)
  (DEF result (QUOTIENT -9223372036854775808 -1))
  (PRINT result)
  (FLAG-SET-P 'OVERFLOW))

;; Example 5: Bit shift overflow
(PROGN
  (PRINT "Example 5: Bit shift overflow")
  (CLEAR-ALL-FLAGS)
  (DEF shifted (LEFTSHIFT 1 64))
  (PRINT shifted)
  (FLAG-SET-P 'OVERFLOW))

;; Example 6: Custom error handling with flags
(DEFMACRO WITH-OVERFLOW-CHECK (expr)
  (CONS 'PROGN
    (CONS (CONS 'CLEAR-ALL-FLAGS NIL)
      (CONS expr
        (CONS (CONS 'IF
                (CONS (CONS 'FLAG-SET-P (CONS (CONS 'QUOTE (CONS 'OVERFLOW NIL)) NIL))
                  (CONS (CONS 'ERROR (CONS "Arithmetic overflow detected" NIL))
                    (CONS NIL NIL))))
              NIL)))))

;; Use the macro
(PROGN
  (PRINT "Example 6: Macro-based overflow checking")
  (ERRORSET (WITH-OVERFLOW-CHECK (PLUS 9223372036854775807 1))))

;; Example 7: Multiple custom flags
(PROGN
  (PRINT "Example 7: Custom application flags")
  (CLEAR-ALL-FLAGS)
  (SET-FLAG 'DATA-READY)
  (SET-FLAG 'COMPUTATION-COMPLETE)
  (AND (FLAG-SET-P 'DATA-READY)
       (FLAG-SET-P 'COMPUTATION-COMPLETE)
       (NOT (FLAG-SET-P 'ERROR))))

;; Example 8: Flag persistence across function calls
(PROGN
  (PRINT "Example 8: Flag persistence")
  (DEFUN RISKY-COMPUTATION (x y)
    (TIMES x y))

  (CLEAR-ALL-FLAGS)
  (DEF result (RISKY-COMPUTATION 9223372036854775807 2))
  (PRINT "After function call:")
  (FLAG-SET-P 'OVERFLOW))
