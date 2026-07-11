;;; binary-search -- O(log n) lookup in a sorted array.
;;; Shows: arrays with ref (0.3), loop-free tail recursion on an index
;;; range, Option as the result type (found vs absent, no nil ambiguity).
;;; Run: cargo run -- examples/binary-search/main.lisp

(defun bsearch-aux (arr target lo hi)
  (if (> lo hi)
      (none)
      (let* ((mid (/ (+ lo hi) 2))
             (v (ref arr mid)))
        (cond ((= v target) (some mid))
              ((< v target) (bsearch-aux arr target (+ mid 1) hi))
              (t (bsearch-aux arr target lo (- mid 1)))))))

(defun bsearch (arr target)
  "Index of TARGET in sorted ARR as an Option."
  (bsearch-aux arr target 0 (- (array-length* arr) 1)))

(def $arr (list->array (list 2 3 5 7 11 13 17 19 23 29)))

(for-each (list 7 11 4)
  (lambda (x)
    (variant-case (bsearch $arr x)
      (some (i) (format t "~a found at index ~a~%" x i))
      (none () (format t "~a not present~%" x)))))

;; self-check: every element is found at its own index; absentees miss.
(if (and (every (lambda (i) (equal (bsearch $arr (ref $arr i)) (some i)))
                (iota 10))
         (equal (bsearch $arr 1) (none))
         (equal (bsearch $arr 30) (none)))
    (print 'ok)
    (error "binary-search self-check failed"))
