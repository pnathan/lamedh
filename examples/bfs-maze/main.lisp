;;; bfs-maze -- shortest path through an ASCII maze, breadth-first.
;;; Shows: a grid as a list of strings, coordinates as (pair int64 int64)
;;; hash keys, a functional FIFO queue (front/back lists), path
;;; reconstruction, and rendering the solution.
;;; Run: cargo run -- examples/bfs-maze/main.lisp

(def $maze
  '("#########"
    "#S..#...#"
    "#.#.#.#.#"
    "#.#...#.#"
    "#.#####.#"
    "#.......#"
    "#######E#"
    "#########"))

(defun cell (x y) (ref (ref $maze y) x))
(defun open-p (x y)
  (and (>= x 0) (>= y 0)
       (< y (length $maze)) (< x (string-length* (ref $maze y)))
       (not (equal (cell x y) "#"))))

;; Appending the per-row match lists keeps the shape explicit. (0.3's
;; flatten treats dotted-pair cells as leaves, so it would work too.)
(defun row-matches (row-idx line mark)
  (filter #'consp
          (map (enumerate (string->list line))
               (lambda (c)
                 (if (equal (cadr c) mark) (cons (car c) row-idx) ())))))

(defun find-cell (mark)
  "The (x . y) of the cell containing MARK."
  (car (apply #'append
              (map (enumerate $maze)
                   (lambda (row) (row-matches (car row) (cadr row) mark))))))

(defun neighbors (pos)
  (let ((x (car pos)) (y (cdr pos)))
    (filter (lambda (p) (open-p (car p) (cdr p)))
            (list (cons (1+ x) y) (cons (- x 1) y)
                  (cons x (1+ y)) (cons x (- y 1))))))

;; Functional queue: (front . back); pop from front, push to back.
(defun q-push (q item) (cons (car q) (cons item (cdr q))))
(defun q-pop (q)
  "((item . rest-queue)) or () when empty."
  (cond ((car q) (list (cons (car (car q)) (cons (cdr (car q)) (cdr q)))))
        ((cdr q) (q-pop (cons (reverse (cdr q)) ())))
        (t ())))

(defun bfs (start goal)
  "Hash of position -> predecessor, filled until GOAL is reached."
  (let ((came (make-hash-table)))
    (put! came start 'start)
    (bfs-aux (cons (list start) ()) goal came)
    came))

(defun bfs-aux (q goal came)
  (let ((popped (q-pop q)))
    (cond ((null popped) ())
          (t (let* ((pos (car (car popped)))
                    (rest (cdr (car popped))))
               (if (equal pos goal)
                   ()
                   (bfs-aux
                    (reduce (lambda (qq n)
                              (if (has-key-p came n)
                                  qq
                                  (progn (put! came n pos) (q-push qq n))))
                            (neighbors pos)
                            rest)
                    goal came)))))))

(defun walk-back (came pos acc)
  (if (equal (gethash came pos) 'start)
      (cons pos acc)
      (walk-back came (gethash came pos) (cons pos acc))))

(def $start (find-cell "S"))
(def $goal (find-cell "E"))
(def $came (bfs $start $goal))
(def $path (walk-back $came $goal ()))

(format t "shortest path: ~a steps~%" (- (length $path) 1))

;; Render the maze with the path dotted in.
(def $path-set (make-hash-table))
(for-each $path (lambda (p) (put! $path-set p t)))
(for-each (enumerate $maze)
  (lambda (row)
    (format t "~a~%"
      (string-join
       (map (enumerate (string->list (cadr row)))
            (lambda (c)
              (let ((pos (cons (car c) (car row))))
                (if (and (gethash $path-set pos)
                         (equal (cadr c) "."))
                    "o"
                    (cadr c)))))
       ""))))

;; self-check: known shortest length for this maze, path is connected,
;; and every step is an open cell.
(defun adjacent-p (a b)
  (= 1 (+ (abs (- (car a) (car b))) (abs (- (cdr a) (cdr b))))))
(if (and (= (- (length $path) 1) 11)
         (every (lambda (p) (open-p (car p) (cdr p))) $path)
         (every (lambda (pair) (adjacent-p (car pair) (cadr pair)))
                (zip $path (cdr $path))))
    (print 'ok)
    (error "bfs-maze self-check failed"))
