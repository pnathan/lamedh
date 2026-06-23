;; String and symbol operation coverage.

(deftest str-concat
  (assert-equal (concat "ab" "cd") "abcd")
  (assert-equal (concat "he" "llo") "hello"))

(deftest str-index
  (assert-equal (index "hello" 0) "h")
  (assert-equal (index "hello" 1) "e")
  (assert-equal (index "hello" 4) "o"))

(deftest sym-explode
  (assert-equal (explode 'abc) '(A B C))
  (assert-equal (explode 'hi) '(H I)))

(deftest sym-implode
  (assert-equal (implode '(H E L L O)) 'HELLO)
  (assert-equal (implode '(A B)) 'AB))

(deftest sym-maknam
  (assert-equal (maknam '(H E L L O)) 'HELLO)
  (assert-equal (maknam '(F O O)) 'FOO))

(deftest sym-intern
  (assert-true (symbolp (intern "HELLO")))
  (assert-equal (intern "HELLO") 'HELLO))

(deftest sym-symbolp
  (assert-true  (symbolp 'foo))
  (assert-false (symbolp 42))
  (assert-false (symbolp "str"))
  (assert-false (symbolp '(a b))))

(deftest sym-boundp
  (progn
    (def boundp-test-var-xyz 42)
    (assert-true  (boundp 'boundp-test-var-xyz))
    (assert-false (boundp 'undefined-xyz-var-never-set))))

(deftest sym-gensym
  (progn
    (assert-true (symbolp (gensym)))
    (let ((g1 (gensym))
          (g2 (gensym)))
      (progn
        (assert-true  (symbolp g1))
        (assert-true  (symbolp g2))
        (assert-false (equal g1 g2))))))
