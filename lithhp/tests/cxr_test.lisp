;;; Tests for extended CAR/CDR compositions

(defun equal (a b)
  (cond ((atom a) (eq a b))
        ((atom b) nil)
        ((equal (car a) (car b)) (equal (cdr a) (cdr b)))
        (t nil)))

(defun assert-eq (a b)
  (if (not (equal a b))
      (error (list 'assertion-failed a b))))

(defun test-cxr ()
  (let ((nested '((1 2 3) (4 5 6) (7 8 9))))
    (progn
      ;; 2-level
      (assert-eq (caar nested) 1)
      (assert-eq (cadr nested) '(4 5 6))
      (assert-eq (cdar nested) '(2 3))
      (assert-eq (cddr nested) '((7 8 9)))

      ;; 3-level
      (assert-eq (caaar '(((1 2) 3) 4)) 1)
      (assert-eq (caadr '((1 (2 3)) 4)) 2)
      (assert-eq (cadar '((1 2) (3 4))) 3)
      (assert-eq (caddr '((1 2) 3 (4 5))) 4)
      (assert-eq (cdaar '((1 2) 3)) '(2))
      (assert-eq (cdadr '((1 2) (3 4) 5)) '(4))
      (assert-eq (cddar '((1 2) (3 4))) '(4))
      (assert-eq (cdddr '((1 2) 3 (4 5))) '(5))

      ;; 4-level
      (assert-eq (caaaar '((((1)))))) 1)
      (assert-eq (caaadr '(((a (b))))) 'b)
      (assert-eq (caadar '((a (b)) c)) 'b)
      (assert-eq (caaddr '((a b) (c (d)))) 'd)
      (assert-eq (cadaar '((a (b c)) d)) 'b)
      (assert-eq (cadadr '((a b) (c (d e)))) 'd)
      (assert-eq (caddar '((a b) (c d))) 'd)
      (assert-eq (cadddr '((a b) c (d (e)))) 'e)
      (assert-eq (cdaaar '(((a b) c) d)) 'b)
      (assert-eq (cdaadr '((a (b c)) d)) 'c)
      (assert-eq (cdadar '((a b) (c d))) 'd)
      (assert-eq (cdaddr '((a b) c (d e))) 'e)
      (assert-eq (cddaar '((a b) (c d))) '(d))
      (assert-eq (cddadr '((a b) c (d e))) '(e))
      (assert-eq (cdddar '((a b) c (d e))) '(e))
      (assert-eq (cddddr '((a b) c (d e))) '()))))

(test-cxr)
