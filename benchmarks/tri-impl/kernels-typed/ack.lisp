;; Typed-JIT edition of kernels/ack.lisp.
(defun-typed (ack int64) ((m int64) (n int64))
  (if (= m 0) (+ n 1)
      (if (= n 0) (ack (- m 1) 1)
          (ack (- m 1) (ack m (- n 1))))))
(if (eq (compiled-p 'ack) 'native) nil (error "ack: not NATIVE"))
(print (ack 3 8))
