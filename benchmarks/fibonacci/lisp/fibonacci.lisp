;;; Fibonacci benchmark for lamedh
;;; Calculates fibonacci numbers recursively

(defun fibonacci (n)
  "Calculate the nth Fibonacci number using naive recursion"
  (cond
    ((eq n 0) 0)
    ((eq n 1) 1)
    (t (+ (fibonacci (- n 1)) (fibonacci (- n 2))))))

;;; For benchmark: calculate sum of fibonacci(1) through fibonacci(n-1)
(defun fibonacci-sum (n)
  "Calculate sum of fibonacci numbers from 1 to n-1"
  (prog (result i)
    (setq result 0)
    (setq i 1)
    loop
    (cond ((>= i n) (return result)))
    (setq result (+ result (fibonacci i)))
    (setq i (+ i 1))
    (go loop)))
