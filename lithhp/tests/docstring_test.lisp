(defun test-docstrings ()
  (let ((failed-tests nil))
    (progn
      (defun assert-eq (a b test-name)
        (if (not (eq a b))
            (progn
              (print "Test failed: " test-name)
              (print "  Expected: " b)
              (print "  Got: " a)
              (def failed-tests (cons test-name failed-tests)))
          nil))

      (def my-var 42 "This is my-var")
      (assert-eq (documentation 'my-var) "This is my-var" "def-var-doc")

      (defun my-fun (x) "This is my-fun" (* x x))
      (assert-eq (documentation 'my-fun) "This is my-fun" "defun-doc")

      (defun my-fun2 (x) (* x x))
      (assert-eq (documentation 'my-fun2) nil "defun-no-doc")

      (defmacro my-macro (x) "This is my-macro" `(+ ,x ,x))
      (assert-eq (documentation 'my-macro) "This is my-macro" "defmacro-doc")

      (defexpr my-fexpr (args) "This is my-fexpr" (car args))
      (assert-eq (documentation 'my-fexpr) "This is my-fexpr" "defexpr-doc")

      (if (null failed-tests)
          t
        nil))))

(test-docstrings)
