;; Ackermann A(3,8) = 2045: ~2.8M calls, recursion depth ~2k.
(defun ack (m n)
  (if (= m 0) (+ n 1)
      (if (= n 0) (ack (- m 1) 1)
          (ack (- m 1) (ack m (- n 1))))))
(print (ack 3 8))
;; scaled: (print (ack 3 11))
