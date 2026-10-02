;; Typed-JIT edition of kernels/tailsum.lisp.
(defun-typed (tsum int64) ((n int64) (acc int64)) (if (= n 0) acc (tsum (- n 1) (+ acc n))))
(if (eq (compiled-p 'tsum) 'native) nil (error "tsum: not NATIVE"))
(print (tsum 10000000 0))
