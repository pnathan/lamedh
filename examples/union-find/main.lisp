;;; union-find -- disjoint sets with path compression and union by rank.
;;; Shows: mutable array state, in-place path compression, and using the
;;; structure for its classic application: connected components.
;;; Run: cargo run -- examples/union-find/main.lisp

(defun uf-make (n)
  (let ((parent (array n)) (rank (array n)))
    (dotimes (i n) (put! parent i i) (put! rank i 0))
    (cons parent rank)))

(defun uf-find (uf x)
  (let ((p (ref (car uf) x)))
    (if (= p x)
        x
        (let ((root (uf-find uf p)))
          (put! (car uf) x root)   ; path compression
          root))))

(defun uf-union (uf a b)
  (let ((ra (uf-find uf a)) (rb (uf-find uf b)))
    (cond ((= ra rb) ())
          ((< (ref (cdr uf) ra) (ref (cdr uf) rb))
           (put! (car uf) ra rb))
          ((> (ref (cdr uf) ra) (ref (cdr uf) rb))
           (put! (car uf) rb ra))
          (t (put! (car uf) rb ra)
             (put! (cdr uf) ra (1+ (ref (cdr uf) ra)))))))

;; Ten nodes, edges forming three components: {0..3} {4 5} {6 7 8 9}.
(def $uf (uf-make 10))
(for-each '((0 . 1) (1 . 2) (2 . 3) (4 . 5) (6 . 7) (7 . 8) (8 . 9))
  (lambda (e) (uf-union $uf (car e) (cdr e))))

(def $components
  (length (remove-duplicates (mapcar (lambda (i) (uf-find $uf i)) (iota 10)))))
(format t "components: ~a~%" $components)

;; self-check: 3 components; connectivity queries agree.
(if (and (= $components 3)
         (= (uf-find $uf 0) (uf-find $uf 3))
         (= (uf-find $uf 6) (uf-find $uf 9))
         (not (= (uf-find $uf 0) (uf-find $uf 4))))
    (print 'ok)
    (error "union-find self-check failed"))
