;;; game-of-life -- Conway's automaton, glider included.
;;; Shows: a sparse world as a hash of live (x . y) cells, neighbor
;;; census via frequencies (0.3), generational stepping as a pure
;;; function, and the glider's period-4 diagonal translation as the check.
;;; Run: cargo run -- examples/game-of-life/main.lisp

(defun neighbors (cell)
  (let ((x (car cell)) (y (cdr cell)))
    (list (cons (- x 1) (- y 1)) (cons x (- y 1)) (cons (1+ x) (- y 1))
          (cons (- x 1) y)                        (cons (1+ x) y)
          (cons (- x 1) (1+ y)) (cons x (1+ y)) (cons (1+ x) (1+ y)))))

(defun step-world (live)
  "LIVE is a list of cells; returns the next generation. mapcan is the
one-level splice for per-cell neighbor lists."
  (let ((census (frequencies (mapcan #'neighbors live))))
    (filter
     (lambda (cell)
       ;; assoc misses () for an isolated live cell; default the count.
       (let* ((hit (assoc cell census))
              (n (if hit (cdr hit) 0)))
         (if (member cell live)
             (or (= n 2) (= n 3))     ; survival
             (= n 3))))               ; birth
     (remove-duplicates (append live (mapcar #'car census))))))

(defun render (live width height)
  (dotimes (y height)
    (format t "~a~%"
      (string-join
       (mapcar (lambda (x) (if (member (cons x y) live) "#" "."))
               (iota width))
       ""))))

(def $glider (list (cons 1 0) (cons 2 1) (cons 0 2) (cons 1 2) (cons 2 2)))

(defun run (live n)
  (if (= n 0) live (run (step-world live) (- n 1))))

(format t "generation 0:~%")
(render $glider 6 6)
(format t "generation 4:~%")
(def $gen4 (run $glider 4))
(render $gen4 6 6)

;; self-check: after 4 generations a glider is itself translated by
;; (+1, +1); a block is a still life; a blinker has period 2.
(defun translate (live dx dy)
  (mapcar (lambda (c) (cons (+ dx (car c)) (+ dy (cdr c)))) live))
(defun cell-key (c) (+ (* 100000 (car c)) (cdr c)))
(defun same-world-p (a b)
  (equal (sort-by (copy a) #'cell-key) (sort-by (copy b) #'cell-key)))
(def $block (list (cons 0 0) (cons 1 0) (cons 0 1) (cons 1 1)))
(def $blinker (list (cons 0 1) (cons 1 1) (cons 2 1)))
(if (and (same-world-p $gen4 (translate $glider 1 1))
         (same-world-p (step-world $block) $block)
         (same-world-p (run $blinker 2) $blinker)
         (not (same-world-p (step-world $blinker) $blinker)))
    (print 'ok)
    (error "game-of-life self-check failed"))
