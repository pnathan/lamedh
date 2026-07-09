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
  "(TIME form...) -- evaluate FORMs, print elapsed wall time and kernel
steps (the WITH-FUEL unit), and return the value."
  (let* ((t0 (monotonic-micros))
         (measured (eval (cons 'step-count x) e))
         (t1 (monotonic-micros))
         (micros (- t1 t0)))
    (print (list 'time-ms (/ micros 1000) 'steps (car measured)))
    (cdr measured)))

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
    (if (gethash name $trace-originals)
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
  "Set NAME's global binding to FN (NAME is a computed symbol, so the
quoting CSET macro does not apply)."
  (eval (list 'setq name (list 'quote fn))))

(defun untrace (name)
  "Remove (TRACE name) instrumentation, restoring the original function."
  (let ((original (gethash name $trace-originals)))
    (if (null original)
        name
        (progn
          (remhash name $trace-originals)
          ($trace-install name original)
          name))))
