;;; quicksort -- the two-line functional classic, plus a random torture test.
;;; Shows: filter-based partitioning, append (variadic, 0.3), recursion,
;;; and checking against the builtin sort with a seeded PRNG.
;;; Run: cargo run -- examples/quicksort/main.lisp

(defun quicksort (lst)
  (if (null lst)
      ()
      (let ((pivot (car lst)) (rest (cdr lst)))
        (append (quicksort (filter (lambda (x) (< x pivot)) rest))
                (list pivot)
                (quicksort (filter (lambda (x) (>= x pivot)) rest))))))

(format t "~a~%" (quicksort (list 3 1 4 1 5 9 2 6 5 3 5)))

;; self-check: agrees with the builtin on 200 random lists.
(random-seed! 7)
(defun random-list (n) (mapcar (lambda (i) (random 100)) (iota n)))
(if (every (lambda (i)
             (let ((xs (random-list (random 20))))
               (equal (quicksort xs) (sort (copy xs) #'<))))
           (iota 200))
    (print 'ok)
    (error "quicksort disagrees with sort"))
