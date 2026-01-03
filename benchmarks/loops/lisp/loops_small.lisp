;;; Loops benchmark for lamedh - SCALED DOWN VERSION
;;; Tests nested loop performance with reduced iterations

(defun random-int (max)
  "Simple pseudo-random number generator"
  (prog (seed a c m)
    (setq seed 42)
    (setq a 1103515245)
    (setq c 12345)
    (setq m 2147483648)
    (return (remainder (+ (* seed a) c) max))))

(defun loops-benchmark-small (divisor)
  "Run nested loops benchmark - 100 outer x 1000 inner (vs 10k x 100k in full version)"
  (prog (random-val arr i j sum)
    ;; Get a random number 0 <= r < 100
    (setq random-val (random-int 100))

    ;; Create array of 100 elements
    (setq arr nil)
    (setq i 0)
    init-loop
    (cond ((< i 100) (go init-continue)))
    (go init-done)
    init-continue
    (setq arr (cons 0 arr))
    (setq i (+ i 1))
    (go init-loop)

    init-done
    ;; Outer loop: 100 iterations (vs 10k in full)
    (setq i 0)
    outer-loop
    (cond ((< i 100) (go outer-continue)))
    (go outer-done)
    outer-continue

    ;; Inner loop: 1000 iterations (vs 100k in full)
    (setq sum 0)
    (setq j 0)
    inner-loop
    (cond ((< j 1000) (go inner-continue)))
    (go inner-done)
    inner-continue
    (setq sum (+ sum (remainder j divisor)))
    (setq j (+ j 1))
    (go inner-loop)

    inner-done
    (setq sum (+ sum random-val))
    (setq i (+ i 1))
    (go outer-loop)

    outer-done
    (return sum)))
