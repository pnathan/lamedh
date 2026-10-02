;; Self tail-recursive accumulator loop: sum 1..10^7.
(defun tsum (n acc) (if (= n 0) acc (tsum (- n 1) (+ acc n))))
(print (tsum 10000000 0))
;; scaled: (print (tsum 300000000 0))
