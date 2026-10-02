;; Call-heavy doubly recursive fib. Result: fib(32) = 2178309.
(defun fib (n) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2)))))
(print (fib 32))
;; scaled: (print (fib 38))
