;; Common forms that CL-trained hands reach for (issue #526): LABELS, EQL,
;; TYPE-OF, MAKE-LIST, STRING-LENGTH, PARSE-INTEGER :JUNK-ALLOWED, and the
;; #\c character syntax.

;;; ---- labels ----------------------------------------------------------------

(deftest labels-self-recursion
  (assert-equal (labels ((fact (n) (if (= n 0) 1 (* n (fact (- n 1))))))
                  (fact 10))
                3628800))

(deftest labels-mutual-recursion
  (labels ((ev (n) (if (= n 0) t (od (- n 1))))
           (od (n) (if (= n 0) nil (ev (- n 1)))))
    (assert-true (ev 10))
    (assert-true (od 7))
    (assert-nil (ev 7))))

(deftest labels-closes-over-enclosing-scope
  (let ((k 3))
    (assert-equal (labels ((scale (xs) (if (null xs) nil
                                           (cons (* k (car xs)) (scale (cdr xs))))))
                    (scale '(1 2 3)))
                  '(3 6 9))))

(deftest labels-inside-defun
  (defun $labels-sum-to (n)
    (labels ((lp (i acc) (if (> i n) acc (lp (+ i 1) (+ acc i)))))
      (lp 1 0)))
  (assert-equal ($labels-sum-to 100) 5050))

(deftest labels-escaping-closure-and-empty-body
  (let ((f (labels ((down (n) (if (= n 0) 'done (down (- n 1))))) down)))
    (assert-equal (f 5) 'done))
  (assert-nil (labels ((f (x) x)))))

(deftest labels-shadow-only-in-body
  (defun $labels-outer () 'global)
  (assert-equal (labels (($labels-outer () 'local)) ($labels-outer)) 'local)
  (assert-equal ($labels-outer) 'global))

;;; ---- eql -------------------------------------------------------------------

(deftest eql-semantics
  (assert-true (eql 1 1))
  (assert-nil (eql 1 1.0))
  (assert-true (eql 2.5 2.5))
  (assert-true (eql 'a 'a))
  (assert-true (eql 'a' 'a'))
  (assert-nil (eql 'a' 'b'))
  (assert-nil (eql 'a' 97))
  (assert-nil (eql (list 1) (list 1)))
  (let ((x (list 1)))
    (assert-true (eql x x))))

;;; ---- type-of ---------------------------------------------------------------

(deftest type-of-basic-values
  (assert-equal (type-of nil) 'null)
  (assert-equal (type-of '(1 2)) 'cons)
  (assert-equal (type-of 'a') 'character)
  (assert-equal (type-of 1.5) 'float)
  (assert-equal (type-of 42) 'integer)
  (assert-equal (type-of "s") 'string)
  (assert-equal (type-of 'foo) 'symbol)
  (assert-equal (type-of t) 'symbol)
  (assert-equal (type-of (list->array '(1 2))) 'array)
  (assert-equal (type-of (make-hash-table)) 'hash-table)
  (assert-equal (type-of (make-error 'oops "m")) 'error))

(deftest type-of-operators
  (assert-equal (type-of (lambda (x) x)) 'function)
  (assert-equal (type-of #'car) 'function)
  (assert-equal (type-of (macro (x) x)) 'macro)
  (assert-equal (type-of (fexpr (x) x)) 'function))

;;; ---- make-list -------------------------------------------------------------

(deftest make-list-forms
  (assert-equal (make-list 3) '(nil nil nil))
  (assert-equal (make-list 3 :initial-element 0) '(0 0 0))
  (assert-equal (make-list 0) nil)
  (assert-equal (make-list -1) nil)
  (assert-equal (length (make-list 1000 :initial-element 'x)) 1000))

;;; ---- string-length ---------------------------------------------------------

(deftest string-length-alias
  (assert-equal (string-length "hello") 5)
  (assert-equal (string-length "") 0)
  (assert-equal (string-length "héllo") (string-length* "héllo")))

(deftest string->list-yields-strings
  (assert-equal (string->list "ab") '("a" "b"))
  (assert-true (stringp (car (string->list "a"))))
  (assert-nil (charp (car (string->list "a")))))

;;; ---- parse-integer ---------------------------------------------------------

(deftest parse-integer-strict-unchanged
  (assert-equal (parse-integer "42") 42)
  (assert-equal (parse-integer " -42 ") -42)
  (assert-nil (parse-integer "12x"))
  (assert-nil (parse-integer "3.14"))
  (assert-nil (parse-integer ""))
  (assert-nil (parse-integer "12x" :junk-allowed nil)))

(deftest parse-integer-junk-allowed
  (assert-equal (parse-integer "12x" :junk-allowed t) 12)
  (assert-equal (parse-integer "  -42abc" :junk-allowed t) -42)
  (assert-equal (parse-integer "+7 apples" :junk-allowed t) 7)
  (assert-equal (parse-integer "3.14" :junk-allowed t) 3)
  (assert-equal (parse-integer "99" :junk-allowed t) 99)
  (assert-nil (parse-integer "abc" :junk-allowed t))
  (assert-nil (parse-integer "" :junk-allowed t))
  (assert-nil (parse-integer "   " :junk-allowed t))
  (assert-nil (parse-integer "-" :junk-allowed t))
  (assert-nil (parse-integer "- 5" :junk-allowed t)))

;;; ---- #\c character syntax --------------------------------------------------

(deftest hash-backslash-char-literals
  (assert-true (charp #\a))
  (assert-true (eq #\a 'a'))
  (assert-equal (char-code #\A) 65)
  (assert-equal (char-code #\Space) 32)
  (assert-equal (char-code #\space) 32)
  (assert-equal (char-code #\Newline) 10)
  (assert-equal (char-code #\Tab) 9)
  (assert-equal (char-code #\() 40)
  (assert-equal (char-code #\)) 41)
  (assert-equal (char-code #\") 34)
  (assert-equal (char-code #\;) 59)
  (assert-equal (list #\a #\b) (list 'a' 'b'))
  (assert-equal (type-of #\z) 'character))
