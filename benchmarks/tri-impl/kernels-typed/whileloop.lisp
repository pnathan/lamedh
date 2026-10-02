;; Typed-JIT edition of kernels/whileloop.lisp.
(defun-typed (wsum int64) ((n int64))
  (let ((i 0) (s 0))
    (while (< i n)
      (setq s (+ s (mod (* i i) 7)))
      (setq i (+ i 1)))
    s))
(if (eq (compiled-p 'wsum) 'native) nil (error "wsum: not NATIVE"))
(print (wsum 5000000))
