;; Higher-order and substitution/association list processing.

(deftest proc-mapcar
  ;; argument order is (mapcar fn list) — function first, like Common Lisp
  (assert-equal (mapcar (lambda (x) (* x x)) '(1 2 3)) '(1 4 9))
  (assert-equal (mapcar (lambda (x) (+ x 1)) '(1 2 3)) '(2 3 4)))

(deftest proc-maplist
  ;; maplist applies fn to successive cdrs
  (assert-equal (maplist (lambda (l) l) '(1 2 3)) '((1 2 3) (2 3) (3))))

(deftest proc-assoc
  (assert-equal (assoc 'b '((a . 1) (b . 2))) '(b . 2))
  (assert-false (assoc 'z '((a . 1) (b . 2)))))

(deftest proc-subst
  (assert-equal (subst 'x 'b '(a b c)) '(a x c))
  (assert-equal (subst 'x 'b '(a (b) c)) '(a (x) c)))

(deftest proc-sublis
  (assert-equal (sublis '((a . 1) (b . 2)) '(a b c)) '(1 2 c)))

(deftest proc-apply-funcall
  (assert-equal (apply (function +) '(1 2 3)) 6)
  (assert-equal (funcall (function +) 1 2 3) 6)
  (assert-equal (apply (function cons) '(1 2)) '(1 . 2)))
