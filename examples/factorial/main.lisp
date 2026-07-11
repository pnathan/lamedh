;;; factorial -- recursion, reduce, and where int64 ends.
;;; Shows: plain recursion, a fold over iota, and 20! as the last
;;; factorial that fits in int64.
;;; Run: cargo run -- examples/factorial/main.lisp

(defun factorial (n)
  (if (< n 2) 1 (* n (factorial (- n 1)))))

(defun factorial-fold (n)
  (reduce #'* (iota n 1) 1))

(dotimes (i 10)
  (format t "~a! = ~a~%" (1+ i) (factorial (1+ i))))

;; self-check: both agree, and 20! is exactly right at the int64 edge.
(if (and (= (factorial 20) 2432902008176640000)
         (= (factorial-fold 20) 2432902008176640000)
         (= (factorial 0) 1))
    (print 'ok)
    (error "factorial self-check failed"))
