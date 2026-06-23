;; Error handling via ERRORSET.
;;
;; errorset evaluates its first argument:
;;   - on success  → wraps result in a one-element list
;;   - on error    → returns nil

(deftest errorset-success
  ;; A successful expression is wrapped in a list
  (assert-equal (errorset '(car '(a)) nil) '(A)))

(deftest errorset-error
  ;; An expression that signals an error returns nil
  (assert-nil (errorset '(error "boom") nil)))

(deftest errorset-arithmetic
  ;; Ordinary successful computation also gets wrapped
  (assert-equal (errorset '(+ 1 1) nil) '(2)))

(deftest errorset-returns-list
  ;; The result of a successful errorset is always a cons (one-element list)
  (let ((r (errorset '(+ 10 20) nil)))
    (progn
      (assert-true  (consp r))
      (assert-equal (car r) 30)
      (assert-nil   (cdr r)))))
