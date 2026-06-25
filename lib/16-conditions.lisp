;;; Condition-system ergonomics (issue #149, epic #141).
;;;
;;; ERRORSET, UNWIND-PROTECT, CATCH/THROW, and BLOCK/RETURN-FROM are kernel
;;; special forms (they require non-local control flow that cannot be expressed
;;; in pure Lisp). This file adds the convenience macros on top of ERRORSET.
;;;
;;; ERRORSET is a function taking a QUOTED form (code as data); it evaluates the
;;; form, trapping ordinary errors. It returns (value) on success and NIL on
;;; error. Because success is wrapped in a one-element list, even a NIL result is
;;; distinguishable from an error (the wrapper list is truthy). The macros below
;;; quote the form they hand to ERRORSET.

(defmacro ignore-errors (&rest body)
  "Evaluate BODY; return its value, or NIL if it signals an error."
  (list 'car (list 'errorset (list 'quote (cons 'progn body)))))

;; HANDLER-CASE: (handler-case expr (error (var) handler-body...))
;;
;; Evaluates EXPR. On success returns its value. On error, binds VAR to NIL
;; (the condition object is not yet surfaced by ERRORSET — see #149 follow-up)
;; and evaluates HANDLER-BODY.
(defmacro handler-case (expr clause)
  "Evaluate EXPR; on error run the (error (var) ...) CLAUSE's handler body."
  (let ((var (car (car (cdr clause))))
        (handler-body (cdr (cdr clause)))
        (r (gensym)))
    (list 'let (list (list r (list 'errorset (list 'quote expr))))
          (list 'if r
                (list 'car r)
                (list 'let (list (list var nil))
                      (cons 'progn handler-body))))))
