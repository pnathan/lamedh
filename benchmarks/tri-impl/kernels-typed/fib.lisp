;; Typed-JIT edition of kernels/fib.lisp (same algorithm, DEFUN-TYPED).
(defun-typed (fib int64) ((n int64)) (if (< n 2) n (+ (fib (- n 1)) (fib (- n 2)))))
(if (eq (compiled-p 'fib) 'native) nil (error "fib: not NATIVE"))
(print (fib 32))
