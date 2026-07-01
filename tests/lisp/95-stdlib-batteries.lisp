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

(deftest control-typecase
  (assert-equal (typecase 5 (string 's) (number 'n) (t 'other)) 'n)
  (assert-equal (typecase "hi" (string 's) (number 'n) (t 'other)) 's)
  (assert-equal (typecase 'foo (string 's) (number 'n) (t 'other)) 'other)
  (assert-equal (typecase 5 (integer 'int) (float 'flt)) 'int)
  (assert-equal (typecase 5.0 (integer 'int) (float 'flt)) 'flt)
  (assert-equal (typecase '(1 2) (cons 'c) (null 'nil-type) (t 'other)) 'c)
  (assert-equal (typecase nil (cons 'c) (null 'nil-type) (t 'other)) 'nil-type)
  (assert-nil   (typecase 5 (string 's))))

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
  (assert-equal (reduce #'+ '(1 2 3 4)) 10)
  (assert-equal (reduce #'+ '(1 2 3 4) 100) 110)
  (assert-equal (reduce #'+ '() 0) 0)
  (assert-equal (foldr #'cons '(1 2 3) nil) '(1 2 3)))

(deftest fn-filter-find-position
  (assert-equal (filter #'evenp '(1 2 3 4 5)) '(2 4))
  (assert-equal (remove-if #'evenp '(1 2 3 4 5)) '(1 3 5))
  (assert-equal (find-if #'evenp '(1 3 4 5)) 4)
  (assert-equal (find 'b '(a b c)) 'b)
  (assert-equal (position 'c '(a b c)) 2)
  (assert-nil   (position 'z '(a b c))))

(deftest fn-quantifiers-count
  (assert-true  (every #'evenp '(2 4 6)))
  (assert-false (every #'evenp '(2 3 6)))
  (assert-true  (some #'evenp '(1 3 4)))
  (assert-false (some #'evenp '(1 3 5)))
  (assert-equal (count-if #'evenp '(1 2 3 4 5 6)) 3))

(deftest fn-slicing
  (assert-equal (take '(1 2 3 4 5) 2) '(1 2))
  (assert-equal (drop '(1 2 3 4 5) 2) '(3 4 5))
  (assert-equal (take-while #'evenp '(2 4 5 6)) '(2 4))
  (assert-equal (drop-while #'evenp '(2 4 5 6)) '(5 6))
  (assert-equal (butlast '(1 2 3)) '(1 2)))

(deftest fn-generate-combine
  (assert-equal (iota 4) '(0 1 2 3))
  (assert-equal (iota 3 10) '(10 11 12))
  (assert-equal (range 2 8 2) '(2 4 6))
  (assert-equal (zip '(1 2 3) '(a b c)) '((1 a) (2 b) (3 c)))
  (assert-equal (unzip '((1 a) (2 b) (3 c))) '((1 2 3) (a b c)))
  (assert-equal (unzip (zip '(1 2 3) '(a b c))) '((1 2 3) (a b c)))
  (assert-equal (unzip nil) '(() ()))
  (assert-equal (mapcan #'identity '((1) (2 3) (4))) '(1 2 3 4))
  (assert-equal (flatten '(1 (2 (3 4)) 5)) '(1 2 3 4 5))
  (assert-equal (remove-duplicates '(1 2 1 3 2)) '(1 2 3)))

(deftest fn-partition-group
  (assert-equal (partition #'evenp '(1 2 3 4)) '((2 4) (1 3)))
  (assert-equal (group-by #'evenp '(1 2 3 4))
                '((nil 1 3) (t 2 4))))

(deftest fn-combinators
  (assert-equal (identity 7) 7)
  (assert-true  (funcall (complement #'evenp) 3))
  (assert-equal (funcall (constantly 9) 1 2 3) 9)
  (assert-equal (funcall (compose #'add1 #'add1) 5) 7)
  (assert-equal (funcall (curry #'+ 10) 5) 15))

;;; ---- char literals -------------------------------------------------------

(deftest char-literals
  ;; Char is a distinct type — not a fixnum.
  (assert-true  (charp 'a'))
  (assert-false (charp 97))
  (assert-false (fixp  'a'))
  ;; Code-point values agree with the corresponding integers.
  (assert-equal (char-code 'A') 65)
  (assert-equal (char-code '0') 48)
  (assert-equal (char-code ' ') 32)
  (assert-equal (char-code '\n') 10)
  (assert-equal (char-code '\'') 39)
  ;; make-char round-trips with char literals.
  (assert-equal (make-char 65) 'A')
  (assert-true  (charp (make-char 65)))
  ;; code-char still returns a one-character string.
  (assert-equal (code-char (char-code 'a')) "a")
  (assert-equal (char-code "a") (char-code 'a'))
  ;; Arithmetic coercion (char promotes to integer like C).
  (assert-equal (+ 'a' 1) 98)
  (assert-equal (- 'z' 'a') 25)
  ;; Numeric comparison works.
  (assert-true  (= 'A' 65))
  (assert-true  (< 'a' 'z'))
  ;; 'a (no closing quote) is still (quote a), not a char.
  (assert-equal 'a 'a)
  (assert-true (consp '(1 2 3))))

(deftest hex-literals
  (assert-equal ffh 255)
  (assert-equal 0ffh 255)
  (assert-equal FFH 255)
  (assert-equal 1ah 26)
  (assert-equal 10h 16)
  (assert-equal (+ ffh 1) 256)
  (assert-true (charp 'A'))    ; 'A' is now a Char value, distinct from Number
  (assert-equal (char-code 'A') 65)  ; char-code extracts the integer
  (assert-equal 0ah 10))

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

(deftest numeric-equals
  ;; Cross-type integer/char/float equality.
  (assert-true  (= 1 1))
  (assert-true  (= 1 1.0))
  (assert-true  (= 'A' 65))
  ;; Float `=` is EXACT (like Common Lisp), not epsilon-fuzzy: two distinct
  ;; floats must never compare equal, even when within f64::EPSILON.
  (assert-true  (= 1.0 1.0))
  (assert-false (= 0.5 0.5000000000000001))
  (assert-false (= 1.0 1.0000000000000002)))

;;; ---- #147 strings --------------------------------------------------------

(deftest string-kernel
  (assert-equal (string-length "hello") 5)
  (assert-equal (substring "hello world" 0 5) "hello")
  (assert-equal (substring "hello" 2) "llo")
  (assert-equal (char-code "A") 65)
  (assert-equal (code-char 66) "B")
  (assert-equal (string->number "42") 42)
  (assert-nil   (string->number "abc"))
  (assert-equal (parse-integer "42") 42)
  (assert-equal (parse-integer "  17  ") 17)
  (assert-nil   (parse-integer "3.14"))
  (assert-nil   (parse-integer "abc"))
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
  ;; one-character strings
  (assert-true  (digit-p "7"))
  (assert-false (digit-p "a"))
  (assert-true  (alpha-p "z"))
  (assert-true  (whitespace-p " "))
  ;; char literals work equally well (polymorphic via char->code)
  (assert-true  (digit-p '0'))
  (assert-true  (alpha-p 'a'))
  (assert-true  (alpha-p 'A'))
  (assert-false (alpha-p '0'))
  (assert-true  (whitespace-p ' '))
  ;; new predicates
  (assert-true  (alphanumeric-p "9"))
  (assert-true  (alphanumeric-p "x"))
  (assert-false (alphanumeric-p " "))
  (assert-true  (char-upper-p "A"))
  (assert-false (char-upper-p "a"))
  (assert-true  (char-lower-p "z"))
  (assert-false (char-lower-p "Z"))
  ;; char-upcase/downcase accept char literals too
  (assert-equal (char-upcase 'a') "A")
  (assert-equal (char-downcase 'A') "a"))

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

(deftest cond-error-values
  (assert-true  (error-p (make-error "boom")))
  (assert-false (error-p 42))
  (assert-equal (error-message (make-error "boom")) "boom")
  (assert-equal (error-data (make-error "boom" '(1 2))) '(1 2))
  (assert-nil   (error-data (make-error "boom")))
  ;; handler-case binds the real condition object
  (assert-equal (handler-case (error "kaboom") (error (e) (error-message e))) "kaboom")
  (assert-equal (handler-case (error "k" '(42)) (error (e) (error-data e))) '(42))
  ;; kernel errors are surfaced as error values too
  (assert-true  (error-p (handler-case (car 5) (error (e) e))))
  ;; success path returns the expression value, ignoring the handler
  (assert-equal (handler-case (+ 1 2) (error (e) 99)) 3))

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

;;; Issue #214: AREF/ASET compiled fine in the typed JIT (elaboration.rs
;;; already treated them as synonyms for FETCH/STORE) but were unbound in
;;; the plain evaluator -- so a function using them broke the moment it ran
;;; interpreted instead of JIT-compiled. Now real Common-Lisp-style aliases.
(deftest aref-aset-aliases
  (let ((a (array 3)))
    (aset a 0 7)
    (aset a 1 8)
    (aset a 2 9)
    (assert-equal (aref a 0) 7)
    (assert-equal (aref a 1) 8)
    (assert-equal (aref a 2) 9)
    ;; AREF/ASET and FETCH/STORE must agree -- they're the same primitive.
    (assert-equal (aref a 1) (fetch a 1))
    (store a 1 99)
    (assert-equal (aref a 1) 99)))

;;; ---- #150 format ---------------------------------------------------------

(deftest format-basic
  (assert-equal (format nil "~a + ~a = ~a" 2 3 5) "2 + 3 = 5")
  (assert-equal (format nil "~s" "hi") "\"hi\"")
  (assert-equal (format nil "~a" "hi") "hi")
  (assert-equal (format nil "100~~") "100~")
  (assert-equal (format nil "~d apples" 7) "7 apples"))
