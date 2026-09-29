;;; 26-instrument.lisp -- TRACE / UNTRACE / TIME / STEP-COUNT.
;;;
;;; The unit of work in Lamedh is the KERNEL STEP: one trampoline iteration
;;; (every eval/exec entry and every TCO tail step). It is the same unit
;;; WITH-FUEL budgets, so the two facilities are one ruler:
;;;
;;;   (step-count expr)          ; => (steps . value)
;;;   (with-fuel N expr)         ; errors when the SAME counter passes N
;;;
;;; A form measured at S steps runs to completion under (with-fuel S+k ...)
;;; for the fence's small bookkeeping overhead k. STEP-COUNT is implemented
;;; BY arming the kernel fuel counter and reading the difference -- fuel and
;;; step count cannot drift apart, because they are the same cell.

;;; ---- step-count and time -------------------------------------------------

(def $step-count-sentinel 1000000000000000)

(defvau step-count (x e)
  "(STEP-COUNT form...) -- evaluate FORMs, returning (steps . value): the
kernel steps consumed (the same unit WITH-FUEL budgets) and the result.
Nests inside an armed fuel fence (steps still charge the fence)."
  (let ((body (if (null (cdr x)) (car x) (cons 'progn x)))
        (before (kernel-fuel-remaining)))
    (if before
        ;; Already armed (inside a fence): read the live counter around it.
        (let* ((v (eval body e))
               (after (kernel-fuel-remaining)))
          (cons (- before after) v))
        ;; Unarmed: arm a sentinel budget, measure, disarm -- even on error.
        (unwind-protect
            (progn
              (kernel-fuel-set! $step-count-sentinel)
              (let* ((v (eval body e))
                     (after (kernel-fuel-remaining)))
                (cons (- $step-count-sentinel after) v)))
          (kernel-fuel-set! ())))))

(defvau time (x e)
  "(TIME form...) -- evaluate FORMs, print the elapsed wall time of their
normal execution as (TIME-MS ms), and return the value. TIME does not arm
fuel: armed fuel forces the interpreted path (compiled code never returns
to the metered trampoline), so metering here would time the tree-walker
instead of the code that normally runs. Use STEP-COUNT for kernel steps.
Inside an already-armed fuel fence execution is metered regardless, so
the steps are read off the live counter for free and printed as
(TIME-MS ms STEPS n)."
  (let* ((body (if (null (cdr x)) (car x) (cons 'progn x)))
         (before (kernel-fuel-remaining))
         (t0 (monotonic-micros))
         (v (eval body e))
         (t1 (monotonic-micros))
         (ms (/ (- t1 t0) 1000)))
    (print (if before
               (list 'time-ms ms 'steps (- before (kernel-fuel-remaining)))
               (list 'time-ms ms)))
    v))

;;; ---- trace / untrace -------------------------------------------------------

(def $trace-depth (array 1))
(store $trace-depth 0 0)
(def $trace-originals (make-hash-table))

(defun $trace-indent ()
  (let ((n (fetch $trace-depth 0)))
    (if (< n 1) "" (concat "  " ($trace-indent-1 (- n 1))))))

(defun $trace-indent-1 (n)
  (if (< n 1) "" (concat "  " ($trace-indent-1 (- n 1)))))

(defun $trace-line (text)
  (princ (concat ($trace-indent) text))
  (terpri))

(defun trace (name)
  "Instrument the function bound to NAME: every call prints its arguments
and result, indented by call depth. Undo with (UNTRACE name). The wrapper
is installed on the global binding, so direct recursive calls through the
name are traced too; already-inlined tail loops inside compiled bodies
count as one call."
  (let ((original (eval name)))
    (if (gethash $trace-originals name)
        name
        (progn
          (sethash $trace-originals name original)
          ($trace-install name
                (lambda (&rest args)
                  ($trace-line (prin1-to-string (cons name args)))
                  (store $trace-depth 0 (+ 1 (fetch $trace-depth 0)))
                  (let ((result (unwind-protect (apply original args)
                                  (store $trace-depth 0
                                         (- (fetch $trace-depth 0) 1)))))
                    ($trace-line (concat (prin1-to-string name) " => "
                                         (prin1-to-string result)))
                    result)))
          name))))

(defun $trace-install (name fn)
  "Set NAME's global binding to FN (NAME is a computed symbol: value-level
SET, not the quoting CSET macro)."
  (set name fn))

(defun untrace (name)
  "Remove (TRACE name) instrumentation, restoring the original function."
  (let ((original (gethash $trace-originals name)))
    (if (null original)
        name
        (progn
          (remhash $trace-originals name)
          ($trace-install name original)
          name))))

;;; REQUIRE-ABLE (issue #256): `(require 'instrument)` on a with_prelude()
;;; environment loads exactly this file. with_stdlib() still loads it
;;; unconditionally, unchanged.
;;; Registered as a module for introspection (issue #56). The instrumentation
;;; vocabulary stays FLAT (`trace`/`time`/`untrace`/`step-count` are a debug
;;; DSL, ergonomically flat); this DEFMODULE only records metadata.
(require 'modules)
(defmodule instrument
  (:export step-count time trace untrace))
(provide 'instrument)
