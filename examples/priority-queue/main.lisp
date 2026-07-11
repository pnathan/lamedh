;;; priority-queue -- a binary min-heap on a growable array region.
;;; Shows: index arithmetic (parent/child), sift-up/sift-down with put!/
;;; ref, heapsort as the acceptance test.
;;; Run: cargo run -- examples/priority-queue/main.lisp

(defun heap-make (capacity)
  "(array . size-cell) -- size lives in a one-slot array."
  (cons (array capacity) (list->array (list 0))))

(defun heap-size (h) (ref (cdr h) 0))

(defun heap-push (h x)
  (let ((a (car h)) (n (heap-size h)))
    (put! a n x)
    (put! (cdr h) 0 (1+ n))
    (sift-up a n)))

(defun sift-up (a i)
  (if (= i 0)
      ()
      (let ((p (/ (- i 1) 2)))
        (if (< (ref a i) (ref a p))
            (progn (heap-swap a i p) (sift-up a p))
            ()))))

(defun heap-swap (a i j)
  (let ((tmp (ref a i)))
    (put! a i (ref a j))
    (put! a j tmp)))

(defun heap-pop (h)
  (let* ((a (car h)) (n (heap-size h)) (top (ref a 0)))
    (put! a 0 (ref a (- n 1)))
    (put! (cdr h) 0 (- n 1))
    (sift-down a 0 (- n 1))
    top))

(defun sift-down (a i n)
  (let* ((l (+ 1 (* 2 i))) (r (+ 2 (* 2 i))) (m i))
    (if (and (< l n) (< (ref a l) (ref a m))) (setq m l) ())
    (if (and (< r n) (< (ref a r) (ref a m))) (setq m r) ())
    (if (= m i)
        ()
        (progn (heap-swap a i m) (sift-down a m n)))))

(defun heapsort (xs)
  (let ((h (heap-make (length xs))))
    (for-each (lambda (x) (heap-push h x)) xs)
    (mapcar (lambda (i) (heap-pop h)) (iota (length xs)))))

(format t "~a~%" (heapsort (list 5 3 8 1 9 2 7)))

;; self-check: heapsort agrees with sort on 100 random lists.
(random-seed! 13)
(if (every (lambda (i)
             (let ((xs (mapcar (lambda (j) (random 500)) (iota (random 30)))))
               (equal (heapsort xs) (sort (copy xs) #'<))))
           (iota 100))
    (print 'ok)
    (error "heapsort disagrees with sort"))
