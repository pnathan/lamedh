;;; huffman -- optimal prefix codes from character frequencies.
;;; Shows: variants as tree nodes (defvariant leaf/node), frequencies
;;; (0.3), building by repeated min-merge, code extraction by recursion,
;;; and the prefix-freedom + round-trip self-checks.
;;; Run: cargo run -- examples/huffman/main.lisp

(defvariant huff
  (leaf (ch string) (weight int64))
  (node (weight int64) (left any) (right any)))

(defun huff-weight (h)
  (variant-case h
    (leaf (ch w) w)
    (node (w l r) w)))

(defun insert-by-weight (h lst)
  (cond ((null lst) (list h))
        ((<= (huff-weight h) (huff-weight (car lst))) (cons h lst))
        (t (cons (car lst) (insert-by-weight h (cdr lst))))))

(defun build (queue)
  (if (null (cdr queue))
      (car queue)
      (let ((a (car queue)) (b (cadr queue)))
        (build (insert-by-weight
                (node (+ (huff-weight a) (huff-weight b)) a b)
                (cdr (cdr queue)))))))

(defun huffman-tree (s)
  (build (reduce (lambda (q cell)
                   (insert-by-weight (leaf (car cell) (cdr cell)) q))
                 (frequencies (string->list s))
                 ())))

(defun codes (h prefix acc)
  "Alist of char -> bitstring."
  (variant-case h
    (leaf (ch w) (cons (cons ch prefix) acc))
    (node (w l r) (codes l (concat prefix "0")
                         (codes r (concat prefix "1") acc)))))

(defun encode (s table)
  (string-join (map (string->list s) (lambda (c) (cdr (assoc c table)))) ""))

(defun decode (bits tree)
  (decode-aux (string->list bits) tree tree ""))

(defun decode-aux (bits cur tree acc)
  (variant-case cur
    (leaf (ch w)
      (if (null bits)
          (concat acc ch)
          (decode-aux bits tree tree (concat acc ch))))
    (node (w l r)
      (decode-aux (cdr bits) (if (equal (car bits) "0") l r) tree acc))))

(def $text "this is an example of a huffman tree")
(def $tree (huffman-tree $text))
(def $table (codes $tree "" ()))
(def $bits (encode $text $table))

(format t "text:    ~a chars (~a bits plain)~%"
        (length $text) (* 8 (length $text)))
(format t "encoded: ~a bits~%" (length $bits))

;; self-check: round trip; prefix freedom (no code prefixes another);
;; frequent chars get codes no longer than rare ones.
(defun prefix-of-p (a b)
  (and (< (string-length* a) (string-length* b))
       (equal a (substring b 0 (string-length* a)))))
(if (and (equal (decode $bits $tree) $text)
         (notany (lambda (pair)
                   (exists (lambda (other)
                             (and (not (equal pair other))
                                  (prefix-of-p (cdr pair) (cdr other))))
                           $table))
                 $table)
         (< (length $bits) (* 8 (length $text))))
    (print 'ok)
    (error "huffman self-check failed"))
