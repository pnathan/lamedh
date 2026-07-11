;;; longest-common-subsequence -- LCS length and the subsequence itself.
;;; Shows: memoized recursion over a hash keyed by dotted pairs (typed as
;;; (pair int64 int64) since 0.3), and reconstruction.
;;; Run: cargo run -- examples/longest-common-subsequence/main.lisp

(def $memo (make-hash-table))

(defun lcs (a b i j)
  "The LCS of a[i..] and b[j..] as a list of chars."
  (let ((key (cons i j)))
    (cond ((has-key-p $memo key) (gethash $memo key))
          ((or (= i (string-length* a)) (= j (string-length* b)))
           ())
          (t (put! $memo key
                   (if (equal (ref a i) (ref b j))
                       (cons (ref a i) (lcs a b (1+ i) (1+ j)))
                       (let ((skip-a (lcs a b (1+ i) j))
                             (skip-b (lcs a b i (1+ j))))
                         (if (>= (length skip-a) (length skip-b))
                             skip-a
                             skip-b))))))))

(defun lcs-string (a b)
  (clrhash $memo)
  (string-join (lcs a b 0 0) ""))

(for-each '(("ABCBDAB" . "BDCABA") ("banana" . "atana") ("abc" . "xyz"))
  (lambda (p)
    (format t "lcs(~a, ~a) = \"~a\"~%" (car p) (cdr p)
            (lcs-string (car p) (cdr p)))))

;; self-check: textbook answers.
(if (and (= (string-length* (lcs-string "ABCBDAB" "BDCABA")) 4)
         (equal (lcs-string "banana" "atana") "aana")
         (equal (lcs-string "abc" "xyz") ""))
    (print 'ok)
    (error "lcs self-check failed"))
