;;; run-length-encoding -- compress runs, expand them back.
;;; Shows: string->list / list->string, run grouping by recursion,
;;; string-repeat (0.3), and a round-trip self-check.
;;; Run: cargo run -- examples/run-length-encoding/main.lisp

(defun runs (chars)
  "((char . count) ...) for consecutive runs in CHARS."
  (if (null chars)
      ()
      (let ((run (take-while (lambda (c) (equal c (car chars))) chars)))
        (cons (cons (car chars) (length run))
              (runs (drop chars (length run)))))))

(defun rle-encode (s)
  (string-join
   (map (lambda (r) (concat (number->string (cdr r)) (car r))) (runs (string->list s)))
   ""))

(defun rle-decode (s)
  (rle-decode-aux (string->list s) ""))

(defun rle-decode-aux (chars acc)
  "Parse digits then a char, repeatedly."
  (if (null chars)
      acc
      (let* ((digits (take-while #'digit-p chars))
             (n (parse-integer (string-join digits "")))
             (c (ref chars (length digits))))
        (rle-decode-aux (drop chars (1+ (length digits)))
                        (concat acc (string-repeat c n))))))

(def $input "aaabbbcccdWWWWWWWWWWWWx")
(def $encoded (rle-encode $input))
(format t "~a -> ~a~%" $input $encoded)

;; self-check: round trip, and the run grouping is right.
(if (and (equal (rle-decode $encoded) $input)
         (equal (rle-encode "aab") "2a1b"))
    (print 'ok)
    (error "rle round-trip failed"))
