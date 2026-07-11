;;; levenshtein -- edit distance with a full DP matrix.
;;; Shows: 2D state as an array of arrays, row initialization, min over
;;; three candidates, and known-distance self-checks.
;;; Run: cargo run -- examples/levenshtein/main.lisp

(defun make-row (n init)
  (let ((r (array n)))
    (dotimes (i n) (put! r i (funcall init i)))
    r))

(defun levenshtein (a b)
  (let* ((la (string-length* a))
         (lb (string-length* b))
         (prev (make-row (1+ lb) (lambda (j) j)))
         (cur (array (1+ lb))))
    (dotimes (i la)
      (put! cur 0 (1+ i))
      (dotimes (j lb)
        (let ((cost (if (equal (ref a i) (ref b j)) 0 1)))
          (put! cur (1+ j)
                (min (1+ (ref cur j))              ; insertion
                     (1+ (ref prev (1+ j)))        ; deletion
                     (+ cost (ref prev j))))))     ; substitution
      (dotimes (j (1+ lb)) (put! prev j (ref cur j))))
    (ref prev lb)))

(for-each (lambda (pair)
            (format t "~a -> ~a: ~a~%" (car pair) (cdr pair)
                    (levenshtein (car pair) (cdr pair))))
          '(("kitten" . "sitting") ("flaw" . "lawn") ("lamedh" . "lamedh")))

;; self-check: the textbook distances.
(if (and (= (levenshtein "kitten" "sitting") 3)
         (= (levenshtein "flaw" "lawn") 2)
         (= (levenshtein "" "abc") 3)
         (= (levenshtein "same" "same") 0))
    (print 'ok)
    (error "levenshtein self-check failed"))
