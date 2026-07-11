;;; monte-carlo-pi -- estimate pi by throwing darts at a unit square.
;;; Shows: random (kernel), float scaling, an accumulation loop, and a
;;; statistical (tolerance-based) self-check.
;;; Run: cargo run -- examples/monte-carlo-pi/main.lisp

(defun random-unit ()
  "A float in [0, 1)."
  (* 0.000001 (random 1000000)))

(defun estimate-pi (samples)
  (let ((hits 0))
    (dotimes (i samples)
      (let ((x (random-unit))
            (y (random-unit)))
        (if (<= (+ (* x x) (* y y)) 1.0)
            (setq hits (1+ hits))
            ())))
    (/ (* 4.0 hits) samples)))

(def $estimate (estimate-pi 20000))
(format t "pi ~~ ~a (20000 samples)~%" $estimate)

;; self-check: a 20k-sample estimate lands within 0.1 of pi with
;; overwhelming probability.
(if (< (abs (- $estimate 3.14159)) 0.1)
    (print 'ok)
    (error "monte-carlo estimate implausibly far from pi"))
