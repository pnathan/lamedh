;; Port-only regressions: behaviour pinned for this port that has no
;; byte-for-byte counterpart under the reference implementation's
;; tests/lisp/ (every other fixture here is a verbatim copy of one).

(deftest port-funcall-symbol-designator
  ;; #533: a symbol in function position names its binding
  (assert-equal (funcall 'car '(1 2)) 1)
  (assert-equal (apply '+ '(1 2)) 3)
  (assert-equal (apply '+ 1 '(2)) 3)
  (assert-equal (mapcar 'car '((1))) '(1))
  (assert-equal (sort '(3 1 2) '<) '(1 2 3))
  ;; resolved in the calling environment, so a lexical binding is seen
  (assert-equal (let ((g (lambda (x) (* x 10)))) (funcall 'g 5)) 50))

(deftest port-funcall-symbol-designator-errors
  ;; #533: unbound names fail the way the reference FUNCALL/APPLY do
  (assert-equal (handler-case (funcall 'no-such-fn-533 1) (error (e) (error-message e)))
                "Function not found: NO-SUCH-FN-533")
  (assert-equal (handler-case (apply 'no-such-fn-533 '(1)) (error (e) (error-message e)))
                "Function not found: NO-SUCH-FN-533")
  ;; T names itself, which is not a function
  (assert-true (handler-case (progn (funcall 't 1) nil) (error (e) t))))
