;; Test file: tests/list_functions_test.lisp
(defun test-list-functions ()
  (and
    ;; Test APPEND
    (equal (append '(a b) '(c d)) '(a b c d))
    (equal (append '() '(a b)) '(a b))
    (equal (append '(a b) '()) '(a b))

    ;; Test MEMBER
    (equal (member 'b '(a b c)) '(b c))
    (null (member 'x '(a b c)))

    ;; Test LENGTH
    (= (length '(a b c d)) 4)
    (= (length '()) 0)

    ;; Test REVERSE
    (equal (reverse '(a b c d)) '(d c b a))
    (equal (reverse '()) '())))