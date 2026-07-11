;;; ninety-nine-bottles -- the song, with correct grammar at 1 and 0.
;;; Shows: countdown recursion, pluralization helpers, format.
;;; Run: cargo run -- examples/ninety-nine-bottles/main.lisp

(defun bottles (n)
  (cond ((= n 0) "no more bottles")
        ((= n 1) "1 bottle")
        (t (concat (number->string n) " bottles"))))

(defun verse (n)
  (format t "~a of beer on the wall, ~a of beer.~%" (bottles n) (bottles n))
  (if (= n 0)
      (format t "Go to the store and buy some more, 99 bottles of beer on the wall.~%")
      (format t "Take one down and pass it around, ~a of beer on the wall.~%~%"
              (bottles (- n 1)))))

;; Three verses from the top, then the pivotal ending.
(for-each #'verse (list 99 98 2 1 0))

;; self-check: grammar boundaries.
(if (and (equal (bottles 2) "2 bottles")
         (equal (bottles 1) "1 bottle")
         (equal (bottles 0) "no more bottles"))
    (print 'ok)
    (error "bottles self-check failed"))
