;; Builtins the reference implementation provides that the SBCL port once
;; left unbound (#540): rplaca/rplacd, make-array, array-sum/array-dot,
;; typed-array, compiled-p, defun*. Written against the reference's own
;; semantics, so this file passes unmodified on both implementations.

(defmacro err-msg (form)
  `(handler-case (progn ,form "no error") (error (e) (error-message e))))

;; ---- rplaca / rplacd: non-mutating, a NEW cell (#508) ----------------------

(deftest ref-rplaca-new-cell
  (assert-equal (rplaca '(1 2 3) 9) '(9 2 3))
  (let ((c (list 1 2)))
    (rplaca c 5)
    (assert-equal c '(1 2))))

(deftest ref-rplacd-new-cell
  (assert-equal (rplacd '(1 2 3) 9) '(1 . 9))
  (assert-equal (rplacd '(1 2 3) '(7)) '(1 7))
  (let ((c (list 1 2)))
    (rplacd c 5)
    (assert-equal c '(1 2))))

(deftest ref-rplac-errors
  (assert-equal (err-msg (rplaca 5 1))
                "RPLACA: expected a cons cell as its first argument, got 5")
  (assert-true (starts-with-p (err-msg (rplacd () 1)) "RPLACD: expected a cons cell")))

;; ---- make-array ---------------------------------------------------------------

(deftest ref-make-array
  (let ((a (make-array 3)))
    (assert-true (arrayp a))
    (assert-equal (array-length* a) 3)
    (assert-equal (array->list a) '(nil nil nil)))
  (assert-equal (array-length* (make-array 0)) 0)
  (assert-equal (err-msg (make-array -2))
                "ARRAY: size must be a non-negative integer, got -2")
  (assert-equal (err-msg (make-array 20000000))
                "array: size 20000000 exceeds maximum of 16777216"))

;; ---- typed-array ----------------------------------------------------------------

(deftest ref-typed-array-int64
  (let ((a (typed-array 3 'int64)))
    (assert-true (arrayp a))
    (assert-true (typed-array-p a))
    (assert-false (typed-array-p (make-array 3)))
    (assert-equal (array-length* a) 3)
    (assert-equal (array->list a) '(0 0 0))
    (store a 0 7)
    (aset a 2 -3)
    (assert-equal (fetch a 0) 7)
    (assert-equal (aref a 2) -3)
    (assert-equal (array->list a) '(7 0 -3))
    (assert-equal (err-msg (store a 0 1.5)) "typed array of int64: cannot store 1.5")
    (assert-equal (err-msg (store a 5 1)) "typed array: index 5 out of bounds (length 3)")))

(deftest ref-typed-array-float64
  (let ((a (typed-array 2 'float64)))
    (assert-equal (array->list a) '(0.0 0.0))
    (store a 0 2.5)
    (store a 1 3)
    (assert-equal (array->list a) '(2.5 3.0))))

(deftest ref-typed-array-errors
  (assert-equal (err-msg (typed-array 3 'int32))
                "TYPED-ARRAY: unknown element type 'INT32, expected 'int64 or 'float64")
  (assert-equal (err-msg (typed-array 3 5))
                "TYPED-ARRAY: element type must be a symbol, got 5")
  (assert-equal (err-msg (typed-array -1 'int64))
                "TYPED-ARRAY: size must be a non-negative integer, got -1")
  (assert-equal (err-msg (typed-array 20000000 'int64))
                "typed-array: size 20000000 exceeds maximum of 16777216"))

;; ---- array-sum / array-dot --------------------------------------------------------

(deftest ref-array-sum-int64
  (assert-equal (array-sum (list->array '(1 2 3))) 6)
  (assert-equal (array-sum (make-array 0)) 0)
  ;; int64 addition wraps (two's complement)
  (assert-equal (array-sum (list->array '(9223372036854775807 1))) -9223372036854775808)
  (let ((a (typed-array 3 'int64)))
    (store a 0 4) (store a 1 5) (store a 2 -2)
    (assert-equal (array-sum a) 7)))

(deftest ref-array-sum-float64
  ;; Float sums have Fortran-SUM semantics (unspecified order, #392): every
  ;; value here is exact in binary, so any order gives the same answer.
  (assert-equal (array-sum (list->array '(1 2.5))) 3.5)
  (assert-equal (array-sum (list->array '(0.5 0.25 0.125 1 2 4 8 16 32 64 0.5))) 128.375)
  (let ((a (typed-array 2 'float64)))
    (store a 0 1.5) (store a 1 2.25)
    (assert-equal (array-sum a) 3.75)))

(deftest ref-array-dot
  (assert-equal (array-dot (list->array '(1 2 3)) (list->array '(4 5))) 14)
  (assert-equal (array-dot (list->array '(1 2 3)) (list->array '(4.0 5 6))) 32.0)
  ;; int64 multiply-accumulate wraps
  (assert-equal (array-dot (list->array '(4611686018427387904 2)) (list->array '(2 3)))
                -9223372036854775802))

(deftest ref-array-reduce-errors
  (assert-equal (err-msg (array-sum 5)) "array-sum: argument must be an array, got 5")
  (assert-equal (err-msg (array-sum (list->array '(1 a))))
                "array-sum: elements must be int64 or float64, got A")
  (assert-equal (err-msg (array-dot (list->array '(1 2)) 7))
                "array-dot: argument must be an array, got 7"))

;; ---- defun* / compiled-p --------------------------------------------------------

(defun* ref-sq (x int64) int64 (* x x))
(defun* ref-add (a b) "Add two numbers." (+ a b))
(defun* ref-k () 42)
(defun* ref-flat x y (- x y))

(deftest ref-defun-star
  (assert-equal (ref-sq 7) 49)
  (assert-equal (ref-add 2 3) 5)
  (assert-equal (ref-k) 42)
  (assert-equal (ref-flat 10 4) 6)
  (assert-equal (err-msg (defun* ref-nobody (x))) "defun*: REF-NOBODY: no body forms")
  (assert-equal (err-msg (defun* 5 (x) x)) "defun*: name must be a symbol"))

(deftest ref-compiled-p
  (assert-nil (compiled-p 'car))
  (assert-nil (compiled-p 'ref-no-such-function))
  (assert-true (starts-with-p (err-msg (compiled-p 5)) "compiled-p requires a symbol")))
