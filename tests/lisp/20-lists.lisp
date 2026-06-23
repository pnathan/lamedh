;; Core list construction, access, and the CAR/CDR compositions.

(deftest list-cons-car-cdr
  (assert-equal (car '(a b c)) 'a)
  (assert-equal (cdr '(a b c)) '(b c))
  (assert-equal (cons 'a '(b c)) '(a b c))
  (assert-equal (cons 1 2) '(1 . 2)))

(deftest list-construct
  (assert-equal (list 1 2 3) '(1 2 3))
  (assert-equal (list) nil)
  (assert-equal (append '(a b) '(c d)) '(a b c d))
  (assert-equal (append nil '(a)) '(a))
  (assert-equal (append '(a) nil) '(a)))

(deftest list-length-reverse
  (assert-equal (length '()) 0)
  (assert-equal (length '(a b c)) 3)
  (assert-equal (reverse '(1 2 3)) '(3 2 1))
  (assert-equal (reverse nil) nil))

(deftest list-member
  (assert-equal (member 'b '(a b c)) '(b c))
  (assert-false (member 'z '(a b c)))
  (assert-equal (member 2 '(1 2 3)) '(2 3)))

(deftest list-pairlis
  (assert-equal (pairlis '(a b) '(1 2)) '((a . 1) (b . 2))))

(deftest list-indexing
  (assert-equal (nth 0 '(a b c)) 'a)
  (assert-equal (nth 1 '(a b c)) 'b)
  (assert-equal (nthcdr 1 '(a b c)) '(b c))
  (assert-equal (last '(a b c)) '(c)))

(deftest list-cxr
  (assert-equal (caar '((1 2) 3)) 1)
  (assert-equal (cadr '(1 2 3)) 2)
  (assert-equal (cddr '(1 2 3)) '(3))
  (assert-equal (caddr '(1 2 3)) 3)
  (assert-equal (cdar '((1 2) 3)) '(2)))
