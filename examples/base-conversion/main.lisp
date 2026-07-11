;;; base-conversion -- render integers in any base 2..36 and read them back.
;;; Shows: digit alphabet indexing with ref, accumulation both directions,
;;; and the reader's own radix literals as the cross-check.
;;; Run: cargo run -- examples/base-conversion/main.lisp

(def $digits "0123456789abcdefghijklmnopqrstuvwxyz")

(defun to-base (n base)
  (cond ((< n 0) (concat "-" (to-base (- 0 n) base)))
        ((< n base) (ref $digits n))
        (t (concat (to-base (/ n base) base)
                   (ref $digits (mod n base))))))

(defun digit-value (c)
  (let ((idx (string-index-of $digits (string-downcase c))))
    (if idx idx (error (concat "bad digit " c)))))

(defun from-base (s base)
  (reduce (lambda (acc c) (+ (* acc base) (digit-value c)))
          (string->list s)
          0))

(for-each (lambda (b) (format t "255 in base ~a: ~a~%" b (to-base 255 b))) '(2 8 12 16 36))

;; self-check: agree with reader radix literals, and round-trip randoms.
(random-seed! 3)
(if (and (equal (to-base 255 16) "ff")
         (= (from-base "ff" 16) 255)
         (= (from-base "177" 8) 127)   ; the reader's 177Q
         (every (lambda (i)
                  (let ((n (random 100000)) (b (+ 2 (random 35))))
                    (= n (from-base (to-base n b) b))))
                (iota 200)))
    (print 'ok)
    (error "base conversion round-trip failed"))
