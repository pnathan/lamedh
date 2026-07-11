;;; primes-sieve -- the Sieve of Eratosthenes.
;;; Shows: arrays as mutable working state, put!/ref (0.3), nested
;;; iteration, and filtering an index space.
;;; Run: cargo run -- examples/primes-sieve/main.lisp

(defun sieve (limit)
  "All primes <= LIMIT."
  (let ((composite (array (1+ limit))))
    (dotimes (i (1+ limit)) (put! composite i ()))
    (dotimes (d (1+ limit))
      (let ((n d))
        (if (and (>= n 2) (not (ref composite n)))
            (let ((m (* n n)))
              (while (<= m limit)
                (put! composite m t)
                (setq m (+ m n))))
            ())))
    (filter (lambda (n) (and (>= n 2) (not (ref composite n))))
            (iota (1+ limit)))))

(def $primes (sieve 100))
(format t "primes to 100: ~a~%" $primes)
(format t "count: ~a~%" (length $primes))

;; self-check: 25 primes below 100; 97 is the largest.
(if (and (= (length $primes) 25)
         (= (ref $primes 24) 97)
         (= (ref $primes 0) 2))
    (print 'ok)
    (error "sieve self-check failed"))
