;; Batteries-included standard library coverage (epic #141).

;;; ---- #142 control-flow macros --------------------------------------------

(deftest control-when-unless
  (assert-equal (when t 1 2 3) 3)
  (assert-nil   (when nil 1 2 3))
  (assert-equal (unless nil 42) 42)
  (assert-nil   (unless t 42)))

(deftest control-prog1
  (assert-equal (prog1 1 2 3) 1))

(deftest control-case
  (assert-equal (case 1 (1 'one) (2 'two) (t 'other)) 'one)
  (assert-equal (case 3 (1 'one) ((2 3) 'two-three) (t 'other)) 'two-three)
  (assert-equal (case 9 (1 'one) (t 'other)) 'other))

(deftest control-dotimes-dolist
  (let ((acc nil))
    (dotimes (i 3) (setq acc (cons i acc)))
    (assert-equal acc '(2 1 0)))
  (let ((acc nil))
    (dolist (x '(a b c)) (setq acc (cons x acc)))
    (assert-equal acc '(c b a)))
  (let ((acc nil))
    (dotimes (i 0) (setq acc (cons i acc)))
    (assert-nil acc)))

;;; ---- #143 functional toolkit ---------------------------------------------

(deftest fn-reduce-fold
  (assert-equal (reduce '(1 2 3 4) #'+) 10)
  (assert-equal (reduce '(1 2 3 4) #'+ 100) 110)
  (assert-equal (reduce '() #'+ 0) 0)
  (assert-equal (foldr '(1 2 3) #'cons nil) '(1 2 3)))

(deftest fn-filter-find-position
  (assert-equal (filter '(1 2 3 4 5) #'evenp) '(2 4))
  (assert-equal (remove-if '(1 2 3 4 5) #'evenp) '(1 3 5))
  (assert-equal (find-if '(1 3 4 5) #'evenp) 4)
  (assert-equal (find '(a b c) 'b) 'b)
  (assert-equal (position '(a b c) 'c) 2)
  (assert-nil   (position '(a b c) 'z)))

(deftest fn-quantifiers-count
  (assert-true  (every '(2 4 6) #'evenp))
  (assert-false (every '(2 3 6) #'evenp))
  (assert-true  (some '(1 3 4) #'evenp))
  (assert-false (some '(1 3 5) #'evenp))
  (assert-equal (count-if '(1 2 3 4 5 6) #'evenp) 3))

(deftest fn-slicing
  (assert-equal (take '(1 2 3 4 5) 2) '(1 2))
  (assert-equal (drop '(1 2 3 4 5) 2) '(3 4 5))
  (assert-equal (take-while '(2 4 5 6) #'evenp) '(2 4))
  (assert-equal (drop-while '(2 4 5 6) #'evenp) '(5 6))
  (assert-equal (butlast '(1 2 3)) '(1 2)))

(deftest fn-generate-combine
  (assert-equal (iota 4) '(0 1 2 3))
  (assert-equal (iota 3 10) '(10 11 12))
  (assert-equal (range 2 8 2) '(2 4 6))
  (assert-equal (zip '(1 2 3) '(a b c)) '((1 a) (2 b) (3 c)))
  (assert-equal (mapcan '((1) (2 3) (4)) #'identity) '(1 2 3 4))
  (assert-equal (flatten '(1 (2 (3 4)) 5)) '(1 2 3 4 5))
  (assert-equal (remove-duplicates '(1 2 1 3 2)) '(1 2 3)))

(deftest fn-partition-group
  (assert-equal (partition '(1 2 3 4) #'evenp) '((2 4) (1 3)))
  (assert-equal (group-by '(1 2 3 4) #'evenp)
                '((nil 1 3) (t 2 4))))

(deftest fn-combinators
  (assert-equal (identity 7) 7)
  (assert-true  (funcall (complement #'evenp) 3))
  (assert-equal (funcall (constantly 9) 1 2 3) 9)
  (assert-equal (funcall (compose #'add1 #'add1) 5) 7)
  (assert-equal (funcall (curry #'+ 10) 5) 15))

;;; ---- #144 sort -----------------------------------------------------------

(deftest sort-basic
  (assert-equal (sort '(3 1 4 1 5 9 2 6) #'lessp) '(1 1 2 3 4 5 6 9))
  (assert-equal (sort '() #'lessp) '())
  (assert-equal (sort '(5 4 3 2 1) #'greaterp) '(5 4 3 2 1))
  (assert-equal (sort '(1 2 3) #'greaterp) '(3 2 1)))

;;; ---- #148 math -----------------------------------------------------------

(deftest math-rounding
  (assert-equal (floor 3.7) 3)
  (assert-equal (ceiling 3.2) 4)
  (assert-equal (round 3.5) 4)
  (assert-equal (truncate -3.7) -3))

(deftest math-integer
  (assert-equal (gcd 12 18) 6)
  (assert-equal (lcm 4 6) 12)
  (assert-equal (isqrt 17) 4)
  (assert-equal (signum -8) -1)
  (assert-equal (signum 0) 0)
  (assert-equal (signum 8) 1))

(deftest math-transcendental
  (assert-true (< (abs (- (sqrt 4.0) 2.0)) 0.0001))
  (assert-true (< (abs (- (exp 0.0) 1.0)) 0.0001)))

;;; ---- #147 strings --------------------------------------------------------

(deftest string-kernel
  (assert-equal (string-length "hello") 5)
  (assert-equal (substring "hello world" 0 5) "hello")
  (assert-equal (substring "hello" 2) "llo")
  (assert-equal (char-code "A") 65)
  (assert-equal (code-char 66) "B")
  (assert-equal (string->number "42") 42)
  (assert-nil   (string->number "abc"))
  (assert-equal (number->string 99) "99"))

(deftest string-layer
  (assert-equal (string-upcase "hello") "HELLO")
  (assert-equal (string-downcase "HELLO") "hello")
  (assert-true  (string= "ab" "ab"))
  (assert-true  (string-lessp "abc" "abd"))
  (assert-equal (string-split "a,b,c" ",") '("a" "b" "c"))
  (assert-equal (string-join '("a" "b" "c") "-") "a-b-c")
  (assert-equal (string-trim "  hi  ") "hi")
  (assert-equal (string-replace "aXbXc" "X" "-") "a-b-c")
  (assert-true  (starts-with-p "hello" "he"))
  (assert-true  (ends-with-p "hello" "lo"))
  (assert-true  (contains-p "hello" "ell"))
  (assert-equal (string-index-of "hello" "l") 2))

(deftest string-char-predicates
  (assert-true  (digit-p "7"))
  (assert-false (digit-p "a"))
  (assert-true  (alpha-p "z"))
  (assert-true  (whitespace-p " ")))

;;; ---- #145 sets / alist / hash --------------------------------------------

(deftest sets-list
  (assert-equal (adjoin 1 '(2 3)) '(1 2 3))
  (assert-equal (adjoin 2 '(2 3)) '(2 3))
  (assert-equal (union '(1 2 3) '(2 3 4)) '(1 2 3 4))
  (assert-equal (intersection '(1 2 3) '(2 3 4)) '(2 3))
  (assert-equal (set-difference '(1 2 3) '(2 3 4)) '(1)))

(deftest alist-helpers
  (assert-equal (alist-get '((a . 1) (b . 2)) 'b) 2)
  (assert-nil   (alist-get '((a . 1)) 'z))
  (assert-equal (alist-put '((a . 1) (b . 2)) 'b 9) '((a . 1) (b . 9)))
  (assert-equal (rassoc 2 '((a . 1) (b . 2))) '(b . 2)))

(deftest hash-helpers
  (let ((h (make-hash-table)))
    (set-bang h 'a 1)
    (set-bang h 'b 2)
    (assert-equal (hash-table-count h) 2)
    (assert-true  (has-key-p h 'a))
    (assert-false (has-key-p h 'z))
    (assert-equal (gethash-or h 'a 0) 1)
    (assert-equal (gethash-or h 'z 0) 0)
    (clrhash h)
    (assert-equal (hash-table-count h) 0)))

;;; ---- #149 conditions -----------------------------------------------------

(deftest cond-errorset-ignore
  (assert-equal (errorset '(+ 1 2)) '(3))
  (assert-nil   (errorset '(car 5)))
  (assert-equal (ignore-errors (+ 1 2)) 3)
  (assert-nil   (ignore-errors (car 5)))
  (assert-equal (handler-case (+ 1 2) (error (e) 99)) 3)
  (assert-equal (handler-case (car 5) (error (e) 99)) 99))

(deftest cond-catch-throw
  (assert-equal (catch 'tag (+ 1 (throw 'tag 42))) 42)
  (assert-equal (catch 'tag 7) 7))

(deftest cond-block-return
  (assert-equal (block done (return-from done 5) 99) 5)
  (assert-equal (block done 11) 11))

(deftest cond-unwind-protect
  (let ((cleaned nil))
    (assert-equal (unwind-protect 1 (setq cleaned t)) 1)
    (assert-true cleaned))
  (let ((cleaned nil))
    (assert-nil (ignore-errors (unwind-protect (car 5) (setq cleaned t))))
    (assert-true cleaned)))

;;; ---- #151 arrays ---------------------------------------------------------

(deftest array-helpers
  (let ((a (list->array '(10 20 30))))
    (assert-equal (array-length a) 3)
    (assert-equal (fetch a 1) 20)
    (assert-equal (array->list a) '(10 20 30))
    (assert-equal (array->list (array-map a #'add1)) '(11 21 31))
    (assert-equal (array->list (subarray a 1 3)) '(20 30))
    (assert-equal (array->list (array-fill (array 2) 0)) '(0 0))))

;;; ---- #150 format ---------------------------------------------------------

(deftest format-basic
  (assert-equal (format nil "~a + ~a = ~a" 2 3 5) "2 + 3 = 5")
  (assert-equal (format nil "~s" "hi") "\"hi\"")
  (assert-equal (format nil "~a" "hi") "hi")
  (assert-equal (format nil "100~~") "100~")
  (assert-equal (format nil "~d apples" 7) "7 apples"))
