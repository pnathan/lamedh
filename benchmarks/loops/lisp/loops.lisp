;;; Loops benchmark for lamedh
;;; Tests nested loop performance

(defun random-int (max)
  "Simple pseudo-random number generator (using LCG algorithm)"
  ;; This is a very simple PRNG for demonstration
  ;; In practice, this should be seeded properly
  (prog (seed a c m)
    (setq seed 42)  ;; Fixed seed for reproducibility
    (setq a 1103515245)
    (setq c 12345)
    (setq m 2147483648)
    (return (% (+ (* seed a) c) max))))

(defun loops-benchmark (divisor)
  "Run nested loops benchmark with given divisor"
  (prog (random-val arr i j sum)
    ;; Get a random number 0 <= r < 10k
    (setq random-val (random-int 10000))

    ;; Create array of 10k elements (using a list in Lisp)
    ;; Initialize to 0
    (setq arr nil)
    (setq i 0)
    init-loop
    (cond ((< i 10000) (go init-continue)))
    (go init-done)
    init-continue
    (setq arr (cons 0 arr))
    (setq i (+ i 1))
    (go init-loop)

    init-done
    ;; Now we have a list of 10000 zeros
    ;; We'll work with the array index-wise

    ;; Outer loop: 10k iterations
    (setq i 0)
    outer-loop
    (cond ((< i 10000) (go outer-continue)))
    (go outer-done)
    outer-continue

    ;; Inner loop: 100k iterations per outer loop
    (setq sum 0)
    (setq j 0)
    inner-loop
    (cond ((< j 100000) (go inner-continue)))
    (go inner-done)
    inner-continue
    (setq sum (+ sum (% j divisor)))
    (setq j (+ j 1))
    (go inner-loop)

    inner-done
    ;; Add random value to sum
    (setq sum (+ sum random-val))
    ;; Store in array (we'll just update the value)
    ;; For simplicity, we'll just accumulate in this implementation

    (setq i (+ i 1))
    (go outer-loop)

    outer-done
    ;; Return the final sum (simplified version)
    (return sum)))
