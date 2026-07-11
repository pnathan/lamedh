;;; fizzbuzz -- the classic screening question.
;;; Shows: dotimes, cond, mod, format, and a self-check.
;;; Run: cargo run -- examples/fizzbuzz/main.lisp

(defun fizzbuzz-word (n)
  (cond ((= 0 (mod n 15)) "FizzBuzz")
        ((= 0 (mod n 3)) "Fizz")
        ((= 0 (mod n 5)) "Buzz")
        (t (number->string n))))

(dotimes (i 20)
  (format t "~a~%" (fizzbuzz-word (1+ i))))

;; self-check
(if (not (equal (mapcar #'fizzbuzz-word (list 1 3 5 15))
                (list "1" "Fizz" "Buzz" "FizzBuzz")))
    (error "fizzbuzz self-check failed")
    (print 'ok))
