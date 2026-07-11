;;; dijkstra -- single-source shortest paths.
;;; Shows: a graph as a hash of adjacency alists, a sorted-list frontier
;;; (fine at this scale), hash-based distance relaxation.
;;; Run: cargo run -- examples/dijkstra/main.lisp

(def $graph (make-hash-table))
(for-each (lambda (entry) (put! $graph (car entry) (cdr entry)))
          '((a . ((b . 7) (c . 9) (f . 14)))
            (b . ((a . 7) (c . 10) (d . 15)))
            (c . ((a . 9) (b . 10) (d . 11) (f . 2)))
            (d . ((b . 15) (c . 11) (e . 6)))
            (e . ((d . 6) (f . 9)))
            (f . ((a . 14) (c . 2) (e . 9)))))

(defun insert-sorted (item frontier)
  "FRONTIER is ((dist . node) ...) ascending by dist."
  (cond ((null frontier) (list item))
        ((<= (car item) (car (car frontier))) (cons item frontier))
        (t (cons (car frontier) (insert-sorted item (cdr frontier))))))

(defun dijkstra (start)
  "Hash of node -> shortest distance from START."
  (let ((dist (make-hash-table)))
    (dijkstra-aux (list (cons 0 start)) dist)
    dist))

(defun dijkstra-aux (frontier dist)
  (if (null frontier)
      ()
      (let* ((head (car frontier))
             (d (car head))
             (node (cdr head))
             (rest (cdr frontier)))
        (if (has-key-p dist node)
            (dijkstra-aux rest dist)
            (progn
              (put! dist node d)
              (dijkstra-aux
               (reduce (lambda (fr edge)
                         (if (has-key-p dist (car edge))
                             fr
                             (insert-sorted (cons (+ d (cdr edge)) (car edge)) fr)))
                       (gethash $graph node)
                       rest)
               dist))))))

(def $dist (dijkstra 'a))
(for-each (lambda (n) (format t "a -> ~a: ~a~%" n (gethash $dist n))) '(a b c d e f))

;; self-check: the classic wikipedia-instance distances.
(if (and (= (gethash $dist 'e) 20)
         (= (gethash $dist 'd) 20)
         (= (gethash $dist 'f) 11)
         (= (gethash $dist 'a) 0))
    (print 'ok)
    (error "dijkstra self-check failed"))
