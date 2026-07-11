;;; palindromes -- string cleanup + reversal, and the largest palindromic
;;; product of two 2-digit numbers (Project Euler #4, scaled down).
;;; Shows: filter over string->list, reverse on strings, nested search
;;; with reduce, and char classification.
;;; Run: cargo run -- examples/palindromes/main.lisp

(defun letters-only (s)
  (string-join (filter #'alpha-p (string->list (string-downcase s))) ""))

(defun palindrome-p (s)
  (let ((clean (letters-only s)))
    (equal clean (reverse clean))))

(for-each '("racecar"
            "A man, a plan, a canal: Panama"
            "Was it a car or a cat I saw?"
            "definitely not")
  (lambda (s) (format t "~a :: ~a~%" (palindrome-p s) s)))

;; Largest palindromic product of two 2-digit factors.
(defun numeric-palindrome-p (n)
  (let ((s (number->string n))) (equal s (reverse s))))

(def $best
  (reduce (lambda (best a)
            (reduce (lambda (b2 b)
                      (let ((p (* a b)))
                        (if (and (numeric-palindrome-p p) (> p b2)) p b2)))
                    (iota 90 10)
                    best))
          (iota 90 10)
          0))
(format t "largest 2-digit palindromic product: ~a~%" $best)

;; self-check: known answer 9009 = 91 * 99.
(if (and (palindrome-p "A man, a plan, a canal: Panama")
         (not (palindrome-p "definitely not"))
         (= $best 9009))
    (print 'ok)
    (error "palindromes self-check failed"))
