;;; anagram-groups -- bucket words by their sorted letters.
;;; Shows: group-by with a computed key, sort over characters, hash-free
;;; grouping, and sort-by for a deterministic report.
;;; Run: cargo run -- examples/anagram-groups/main.lisp

(defun signature (word)
  "The word's letters, sorted -- anagrams share it."
  (string-join (sort (string->list (string-downcase word)) #'string-lessp) ""))

(def $words
  '("listen" "silent" "enlist" "google" "gogole" "banana"
    "cat" "act" "tac" "dog"))

(def $groups
  (sort-by (group-by #'signature (mapcar #'princ-to-string $words))
           (lambda (g) (length (cdr g)))
           #'>))

(for-each (lambda (g) (format t "~a: ~a~%" (car g) (cdr g))) $groups)

;; self-check: listen-family has 3, cat-family has 3, dog is alone.
(defun group-of (word)
  (cdr (assoc (signature word) $groups)))
(if (and (= (length (group-of "listen")) 3)
         (= (length (group-of "cat")) 3)
         (= (length (group-of "dog")) 1)
         (member "silent" (group-of "enlist")))
    (print 'ok)
    (error "anagram grouping failed"))
