;;; mandelbrot -- the ASCII classic.
;;; Shows: float-heavy inner loops, character selection by escape time,
;;; and building lines with string-repeat-free concat accumulation.
;;; Run: cargo run -- examples/mandelbrot/main.lisp

(def $width 60)
(def $height 24)
(def $max-iter 30)
(def $palette " .:-=+*#%@")

(defun escape-time (cr ci)
  "Iterations before z escapes |z| > 2, capped at $max-iter."
  (escape-aux 0.0 0.0 cr ci 0))

(defun escape-aux (zr zi cr ci n)
  (cond ((>= n $max-iter) n)
        ((> (+ (* zr zr) (* zi zi)) 4.0) n)
        (t (escape-aux (+ cr (- (* zr zr) (* zi zi)))
                       (+ ci (* 2.0 zr zi))
                       cr ci (1+ n)))))

(defun shade (n)
  (let ((idx (/ (* n (- (string-length* $palette) 1)) $max-iter)))
    (ref $palette idx)))

(defun render-row (y)
  (let ((ci (+ -1.2 (* y (/ 2.4 $height))))
        (line ""))
    (dotimes (x $width)
      (let ((cr (+ -2.1 (* x (/ 3.0 $width)))))
        (setq line (concat line (shade (escape-time cr ci))))))
    line))

(def $interior-shade (ref $palette (- (string-length* $palette) 1)))
(def $rows (map (iota $height) #'render-row))
(for-each $rows (lambda (line) (format t "~a~%" line)))

;; self-check: the main cardioid is in-set (deepest shade) and the top-left
;; corner escapes immediately (background shade).
(if (and (equal (ref (ref $rows 12) 20) $interior-shade)
         (equal (ref (ref $rows 0) 0) " "))
    (print 'ok)
    (error "mandelbrot self-check failed"))
