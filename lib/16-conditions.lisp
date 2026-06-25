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

;; HANDLER-CASE is a kernel special form (it binds the handler variable to the
;; first-class condition value — a LispVal::Error — which a Lisp macro over
;; ERRORSET cannot recover). See evaluator.rs and lib/16 docs. Signature:
;;   (handler-case expr (error (var) handler-body...))
