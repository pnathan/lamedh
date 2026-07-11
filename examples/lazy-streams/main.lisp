;;; lazy-streams -- infinite sequences via closures (SICP 3.5).
;;; Shows: thunks as delayed cdrs, memoized force, infinite integers,
;;; a lazy sieve of Eratosthenes, and stream-take as the observer.
;;; Run: cargo run -- examples/lazy-streams/main.lisp

;; A stream is (head . thunk); delay memoizes via a one-slot array.
(defun s-cons (head thunk) (cons head thunk))
(defun s-head (s) (car s))

(defun memo-thunk (thunk)
  (let ((cell (list->array (list ()))))
    (lambda ()
      (if (ref cell 0)
          (ref cell 0)
          (put! cell 0 (funcall thunk))))))

(defun s-tail (s) (funcall (cdr s)))

(defun integers-from (n)
  (s-cons n (memo-thunk (lambda () (integers-from (1+ n))))))

(defun s-filter (pred s)
  (if (funcall pred (s-head s))
      (s-cons (s-head s) (memo-thunk (lambda () (s-filter pred (s-tail s)))))
      (s-filter pred (s-tail s))))

(defun s-map (f s)
  (s-cons (funcall f (s-head s))
          (memo-thunk (lambda () (s-map f (s-tail s))))))

(defun s-take (s n)
  (if (= n 0) () (cons (s-head s) (s-take (s-tail s) (- n 1)))))

;; The lazy sieve: primes defined in terms of themselves.
(defun sieve (s)
  (s-cons (s-head s)
          (memo-thunk
           (lambda ()
             (sieve (s-filter
                     (let ((p (s-head s)))
                       (lambda (n) (not (= 0 (mod n p)))))
                     (s-tail s)))))))

(def $primes (sieve (integers-from 2)))
(format t "first 15 primes: ~a~%" (s-take $primes 15))

(def $squares (s-map (lambda (n) (* n n)) (integers-from 1)))
(format t "first 8 squares: ~a~%" (s-take $squares 8))

;; self-check: primes and squares from the infinite streams.
(if (and (equal (s-take $primes 10) (list 2 3 5 7 11 13 17 19 23 29))
         (equal (s-take $squares 5) (list 1 4 9 16 25))
         (= (s-head (s-tail (s-tail $primes))) 5))
    (print 'ok)
    (error "lazy-streams self-check failed"))
