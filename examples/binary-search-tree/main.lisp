;;; binary-search-tree -- immutable BST: insert, member, in-order walk.
;;; Shows: recursive generic records ((node a), 0.3 HM generics) with
;;; Option for the empty tree, persistent (copy-on-insert) structure.
;;; Run: cargo run -- examples/binary-search-tree/main.lisp

(defvariant (bst a)
  (bst-empty)
  (bst-node (value a) (left any) (right any)))

(defun bst-insert (tree x)
  (variant-case tree
    (bst-empty () (bst-node x (bst-empty) (bst-empty)))
    (bst-node (v l r)
      (cond ((< x v) (bst-node v (bst-insert l x) r))
            ((> x v) (bst-node v l (bst-insert r x)))
            (t tree)))))

(defun bst-member-p (tree x)
  (variant-case tree
    (bst-empty () ())
    (bst-node (v l r)
      (cond ((< x v) (bst-member-p l x))
            ((> x v) (bst-member-p r x))
            (t t)))))

(defun bst-inorder (tree)
  (variant-case tree
    (bst-empty () ())
    (bst-node (v l r) (append (bst-inorder l) (list v) (bst-inorder r)))))

(defun bst-from (xs) (reduce #'bst-insert xs (bst-empty)))

(random-seed! 5)
(def $xs (mapcar (lambda (i) (random 1000)) (iota 200)))
(def $tree (bst-from $xs))
(def $walk (bst-inorder $tree))
(format t "inserted ~a values, ~a distinct, first ten in order: ~a~%"
        (length $xs) (length $walk) (take $walk 10))

;; self-check: in-order walk is sorted-and-deduplicated; membership
;; agrees with the list; persistence (the old tree is untouched).
(def $t1 (bst-from (list 5 3 8)))
(def $t2 (bst-insert $t1 4))
(if (and (equal $walk (sort (remove-duplicates (copy $xs)) #'<))
         (every (lambda (x) (bst-member-p $tree x)) (take $xs 50))
         (not (bst-member-p $tree 1001))
         (equal (bst-inorder $t1) (list 3 5 8))       ; unchanged
         (equal (bst-inorder $t2) (list 3 4 5 8)))
    (print 'ok)
    (error "bst self-check failed"))
