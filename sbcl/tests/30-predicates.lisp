;; Predicates, equality, and boolean / conditional special forms.

(deftest pred-eq-equal
  (assert-true  (eq 'a 'a))
  (assert-false (eq 'a 'b))
  (assert-true  (equal '(a (b c)) '(a (b c))))
  (assert-false (equal '(a b) '(a c)))
  (assert-true  (equal 42 42)))

(deftest pred-not-null
  (assert-true  (not nil))
  (assert-false (not 't))
  (assert-true  (null nil))
  (assert-false (null '(a))))

(deftest pred-atom-cons-list
  (assert-true  (atom 'a))
  (assert-true  (atom nil))
  (assert-false (atom '(a b)))
  (assert-true  (consp '(a)))
  (assert-false (consp nil))
  (assert-true  (listp nil))
  (assert-true  (listp '(a)))
  (assert-false (listp 'a)))

(deftest pred-types
  (assert-true  (numberp 5))
  (assert-false (numberp 'a))
  (assert-true  (stringp "hi"))
  (assert-false (stringp 5))
  (assert-true  (symbolp 'a))
  (assert-false (symbolp 5)))

(deftest bool-and-or
  (assert-true  (and 't 't))
  (assert-false (and 't nil))
  (assert-true  (or nil 't))
  (assert-false (or nil nil))
  ;; AND returns last value; OR returns first truthy value
  (assert-equal (and 1 2 3) 3)
  (assert-equal (or nil 7) 7))

(deftest control-if-cond
  (assert-equal (if 't 1 2) 1)
  (assert-equal (if nil 1 2) 2)
  (assert-equal (cond (nil 1) ('t 2)) 2)
  (assert-equal (cond ('t 1)) 1))
