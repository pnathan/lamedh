;; Iterative WHILE/SETQ loop over 5*10^6 with integer MOD in the body.
(defun wsum (n)
  (let ((i 0) (s 0))
    (while (< i n)
      (setq s (+ s (mod (* i i) 7)))
      (setq i (+ i 1)))
    s))
(print (wsum 5000000))
;; scaled: (print (wsum 200000000))
