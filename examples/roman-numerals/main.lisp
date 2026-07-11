;;; roman-numerals -- both directions, checked by round trip.
;;; Shows: alist-driven table code, string building with concat,
;;; string->list for parsing, and property-based self-checking.
;;; Run: cargo run -- examples/roman-numerals/main.lisp

(def $romans
  '((1000 . "M") (900 . "CM") (500 . "D") (400 . "CD")
    (100 . "C") (90 . "XC") (50 . "L") (40 . "XL")
    (10 . "X") (9 . "IX") (5 . "V") (4 . "IV") (1 . "I")))

(defun to-roman-aux (n table acc)
  (cond ((null table) acc)
        ((>= n (car (car table)))
         (to-roman-aux (- n (car (car table))) table
                       (concat acc (cdr (car table)))))
        (t (to-roman-aux n (cdr table) acc))))

(defun to-roman (n) (to-roman-aux n $romans ""))

(def $values (make-hash-table))
(for-each '(("I" . 1) ("V" . 5) ("X" . 10) ("L" . 50)
            ("C" . 100) ("D" . 500) ("M" . 1000))
  (lambda (cell) (put! $values (car cell) (cdr cell))))

(defun from-roman (s)
  "Sum the letters; subtract instead when a smaller value precedes a larger."
  (let ((vals (map (string->list s) (lambda (c) (ref $values c)))))
    (from-roman-aux vals 0)))

(defun from-roman-aux (vals acc)
  (cond ((null vals) acc)
        ((and (cdr vals) (< (car vals) (cadr vals)))
         (from-roman-aux (cdr vals) (- acc (car vals))))
        (t (from-roman-aux (cdr vals) (+ acc (car vals))))))

(for-each (list 4 9 14 40 90 1994 2026)
  (lambda (n) (format t "~a = ~a~%" n (to-roman n))))

;; self-check: round trip over 1..2026.
(if (every (lambda (n) (= n (from-roman (to-roman n)))) (iota 2026 1))
    (print 'ok)
    (error "roman round-trip failed"))
