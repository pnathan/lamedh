;;; state-machine -- a traffic light with a pedestrian button.
;;; Shows: variants as states, exhaustive variant-case transitions (the
;;; checker knows when a state is unhandled), and an event-fold runner.
;;; Run: cargo run -- examples/state-machine/main.lisp

(defvariant light
  (red)
  (green)
  (yellow)
  (red-flashing))                       ; fault state

(defun transition (state event)
  "The next state for STATE on EVENT (tick / button / fault / reset)."
  (if (equal event 'fault)
      (red-flashing)
      (variant-case state
        (red () (if (equal event 'tick) (green) state))
        (green () (cond ((equal event 'tick) (yellow))
                        ((equal event 'button) (yellow))
                        (t state)))
        (yellow () (if (equal event 'tick) (red) state))
        (red-flashing () (if (equal event 'reset) (red) state)))))

(defun light-name (state)
  (variant-case state
    (red () 'red)
    (green () 'green)
    (yellow () 'yellow)
    (red-flashing () 'red-flashing)))

(defun run (state events)
  (reduce (lambda (s e)
            (let ((next (transition s e)))
              (format t "~a --~a--> ~a~%" (light-name s) e (light-name next))
              next))
          events
          state))

(def $end (run (red) '(tick tick button tick fault tick reset tick)))

;; self-check: the trace above ends where the diagram says; the button
;; shortcut works; faults dominate and only reset clears them.
(if (and (equal (light-name $end) 'green)
         (equal (transition (green) 'button) (yellow))
         (equal (transition (yellow) 'fault) (red-flashing))
         (equal (transition (red-flashing) 'tick) (red-flashing))
         (equal (transition (red-flashing) 'reset) (red)))
    (print 'ok)
    (error "state-machine self-check failed"))
