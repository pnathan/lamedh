;;; trie -- prefix tree over words, with autocomplete.
;;; Shows: nested hash tables as nodes, recursion over string->list,
;;; collecting completions by tree walk.
;;; Run: cargo run -- examples/trie/main.lisp

(defun trie-make () (make-hash-table))

(defun trie-insert (trie word)
  (trie-insert-aux trie (string->list word)))

(defun trie-insert-aux (node chars)
  (if (null chars)
      (put! node 'end t)
      (progn
        (if (has-key-p node (car chars))
            ()
            (put! node (car chars) (make-hash-table)))
        (trie-insert-aux (gethash node (car chars)) (cdr chars)))))

(defun trie-node (trie prefix)
  "The node reached by PREFIX, as an Option."
  (trie-node-aux trie (string->list prefix)))

(defun trie-node-aux (node chars)
  (cond ((null chars) (some node))
        ((has-key-p node (car chars))
         (trie-node-aux (gethash node (car chars)) (cdr chars)))
        (t (none))))

(defun trie-member-p (trie word)
  (variant-case (trie-node trie word)
    (some (node) (has-key-p node 'end))
    (none () ())))

(defun completions (node prefix)
  "All words below NODE, each prefixed with PREFIX."
  (mapcan (lambda (k)
            (if (equal k 'end)
                (list prefix)
                (completions (gethash node k) (concat prefix k))))
          (keys node)))

(defun autocomplete (trie prefix)
  (variant-case (trie-node trie prefix)
    (some (node) (sort (completions node prefix) #'string-lessp))
    (none () ())))

(def $trie (trie-make))
(for-each '("car" "cart" "carbon" "cat" "dog" "do" "door")
  (lambda (w) (trie-insert $trie w)))

(format t "car...  -> ~a~%" (autocomplete $trie "car"))
(format t "do...   -> ~a~%" (autocomplete $trie "do"))

;; self-check: membership vs prefixes, and complete sets.
(if (and (trie-member-p $trie "car")
         (trie-member-p $trie "do")
         (not (trie-member-p $trie "ca"))
         (equal (autocomplete $trie "car") '("car" "carbon" "cart"))
         (equal (autocomplete $trie "do") '("do" "dog" "door"))
         (null (autocomplete $trie "zebra")))
    (print 'ok)
    (error "trie self-check failed"))
