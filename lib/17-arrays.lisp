;;; Array helpers — Lisp layer (issue #151, epic #141).
;;;
;;; Built on the array primitives ARRAY (= MAKE-ARRAY), FETCH (= ARRAY-FETCH),
;;; STORE (= ARRAY-STORE), and ARRAY-LENGTH. Growable vectors with push/pop are
;;; intentionally out of scope for now (deferred with the mutation decision,
;;; #114). STORE on an existing array is an allowed in-place mutation of the
;;; array cell (arrays are mutable; only cons aliasing is the concern).

(defun array->list-aux (arr i n)
  (if (< i n)
      (cons (fetch arr i) (array->list-aux arr (+ i 1) n))
      nil))

(defun array->list (arr)
  "Return the elements of ARR as a list."
  (array->list-aux arr 0 (array-length arr)))

(defun list->array (lst)
  "Build a fresh array holding the elements of LST."
  (let ((arr (array (length lst)))
        (i 0))
    (mapc (lambda (x)
                (store arr i x)
                (setq i (+ i 1))) lst)
    arr))

(defun array-map (arr fn)
  "Return a new array with FN applied to each element of ARR."
  (list->array (mapcar fn (array->list arr))))

(defun array-fill (arr val)
  "Set every element of ARR to VAL; return ARR."
  (let ((n (array-length arr)))
    (for (i 0 (- n 1)) (store arr i val))
    arr))

(defun array-copy (arr)
  "Return a fresh array with the same elements as ARR."
  (list->array (array->list arr)))

(defun subarray (arr start end)
  "Return a fresh array holding ARR[start] .. ARR[end-1]."
  (let ((out (array (- end start))))
    (for (i start (- end 1))
         (store out (- i start) (fetch arr i)))
    out))
