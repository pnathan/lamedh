;;; Functional list toolkit (issue #143, epic #141).
;;;
;;; CONVENTION: function-first argument order, matching Common Lisp and the
;;; map* family (mapcar/maplist/mapc/mapcon) — the function or predicate comes
;;; first, the collection last. This nests cleanly: (filter #'evenp (mapcar ...)).
;;; The item-search helpers FIND and POSITION take (item list), like MEMBER.
;;; Functions that take only a count/index (take, drop, iota, ...) keep their
;;; collection/natural order.
;;;
;;; All functions are non-mutating.

;;; ---- folding / reduction -------------------------------------------------

(defun foldl-aux (fn acc lst)
  (if (null lst)
      acc
      (foldl-aux fn (funcall fn acc (car lst)) (cdr lst))))

(defun reduce (fn lst &rest init)
  "Reduce LST left-to-right with binary FN.
With no INIT: seeds from the first element; an empty LST calls (FN) with no args.
With INIT supplied: folds starting from INIT (so an empty LST returns INIT)."
  (if init
      (foldl-aux fn (car init) lst)
      (cond ((null lst) (funcall fn))
            ((null (cdr lst)) (car lst))
            (t (foldl-aux fn (car lst) (cdr lst))))))

(defun foldr (fn lst init)
  "Right fold: (FN e0 (FN e1 (... (FN eN INIT))))."
  (if (null lst)
      init
      (funcall fn (car lst) (foldr fn (cdr lst) init))))

;;; ---- filtering -----------------------------------------------------------

(defun filter (pred lst)
  "Return the elements of LST for which PRED is non-NIL."
  (cond ((null lst) nil)
        ((funcall pred (car lst)) (cons (car lst) (filter pred (cdr lst))))
        (t (filter pred (cdr lst)))))

(defun remove-if-not (pred lst)
  "Alias for FILTER: keep elements satisfying PRED."
  (filter pred lst))

(defun remove-if (pred lst)
  "Return the elements of LST for which PRED is NIL."
  (filter (lambda (x) (not (funcall pred x))) lst))

;;; ---- searching -----------------------------------------------------------

(defun find-if (pred lst)
  "Return the first element of LST satisfying PRED, or NIL."
  (cond ((null lst) nil)
        ((funcall pred (car lst)) (car lst))
        (t (find-if pred (cdr lst)))))

(defun find (item lst)
  "Return ITEM if it is EQUAL to some element of LST, else NIL."
  (find-if (lambda (x) (equal x item)) lst))

(defun position-if-aux (pred lst i)
  (cond ((null lst) nil)
        ((funcall pred (car lst)) i)
        (t (position-if-aux pred (cdr lst) (+ i 1)))))

(defun position-if (pred lst)
  "Return the 0-based index of the first element satisfying PRED, or NIL."
  (position-if-aux pred lst 0))

(defun position (item lst)
  "Return the 0-based index of the first element EQUAL to ITEM, or NIL."
  (position-if (lambda (x) (equal x item)) lst))

(defun count-if (pred lst)
  "Count the elements of LST satisfying PRED."
  (cond ((null lst) 0)
        ((funcall pred (car lst)) (+ 1 (count-if pred (cdr lst))))
        (t (count-if pred (cdr lst)))))

;;; ---- quantifiers ---------------------------------------------------------

(defun every (pred lst)
  "Non-NIL iff PRED holds for every element of LST."
  (cond ((null lst) t)
        ((funcall pred (car lst)) (every pred (cdr lst)))
        (t nil)))

(defun exists (pred lst)
  "Return the first non-NIL (PRED element), or NIL."
  (if (null lst)
      nil
      (let ((r (funcall pred (car lst))))
        (if r r (exists pred (cdr lst))))))

(defun notany (pred lst)
  "Non-NIL iff PRED holds for no element of LST."
  (not (exists pred lst)))

(defun notevery (pred lst)
  "Non-NIL iff PRED fails for at least one element of LST."
  (not (every pred lst)))

;;; ---- mapping that concatenates -------------------------------------------

(defun mapcan (fn lst)
  "Map FN over LST and APPEND the resulting lists."
  (if (null lst)
      nil
      (append (funcall fn (car lst)) (mapcan fn (cdr lst)))))

;;; ---- slicing -------------------------------------------------------------

(defun take (lst n)
  "Return the first N elements of LST (fewer if LST is shorter)."
  (if (or (null lst) (< n 1))
      nil
      (cons (car lst) (take (cdr lst) (- n 1)))))

(defun drop (lst n)
  "Return LST with its first N elements removed."
  (if (or (null lst) (< n 1))
      lst
      (drop (cdr lst) (- n 1))))

(defun take-while (pred lst)
  "Return the leading elements of LST that satisfy PRED."
  (cond ((null lst) nil)
        ((funcall pred (car lst)) (cons (car lst) (take-while pred (cdr lst))))
        (t nil)))

(defun drop-while (pred lst)
  "Return LST after dropping its leading elements that satisfy PRED."
  (cond ((null lst) nil)
        ((funcall pred (car lst)) (drop-while pred (cdr lst)))
        (t lst)))

(defun butlast (lst)
  "Return LST without its last element."
  (if (or (null lst) (null (cdr lst)))
      nil
      (cons (car lst) (butlast (cdr lst)))))

;;; ---- combining / generating ----------------------------------------------

(defun zip (a b)
  "Pair up elements of A and B into a list of two-element lists; stops at the
shorter input."
  (if (or (null a) (null b))
      nil
      (cons (list (car a) (car b)) (zip (cdr a) (cdr b)))))

(defun unzip (pairs)
  "Inverse of ZIP: given a list of two-element lists, return a two-element list
holding the list of firsts and the list of seconds. (unzip (zip a b)) recovers
(list a b) up to the shorter length."
  (list (mapcar (lambda (p) (car p)) pairs)
        (mapcar (lambda (p) (car (cdr p))) pairs)))

(defun iota-aux (n start step acc)
  (if (< n 1)
      (reverse acc)
      (iota-aux (- n 1) (+ start step) step (cons start acc))))

(defun iota (n &rest opts)
  "Return a list of N integers. (iota N) -> 0..N-1; (iota N START) and
(iota N START STEP) shift and scale."
  (let ((start (if opts (car opts) 0))
        (step (if (and opts (cdr opts)) (car (cdr opts)) 1)))
    (iota-aux n start step nil)))

(defun range (start end &rest opts)
  "Return START, START+STEP, ... up to but excluding END (STEP defaults to 1)."
  (let ((step (if opts (car opts) 1)))
    (range-aux start end step nil)))

(defun range-aux (cur end step acc)
  (if (if (> step 0) (< cur end) (> cur end))
      (range-aux (+ cur step) end step (cons cur acc))
      (reverse acc)))

(defun list-tabulate (fn n)
  "Return the list ((fn 0) (fn 1) ... (fn N-1))."
  (mapcar fn (iota n)))

(defun enumerate (lst &optional start)
  "Pair each element with its index: ((0 a) (1 b) ...). START (default 0)
offsets the indices. Same two-element-list shape as ZIP."
  (let ((from (if start start 0)))
    (zip (iota (length lst) from) lst)))

;;; ---- partitioning / grouping ---------------------------------------------

(defun partition (pred lst)
  "Return (MATCHES NON-MATCHES) splitting LST by PRED, order preserved."
  (list (filter pred lst)
        (remove-if pred lst)))

(defun group-by-aux (fn lst acc)
  (if (null lst)
      acc
      (let* ((x (car lst))
             (k (funcall fn x))
             (cell (assoc k acc)))
        (group-by-aux
         fn (cdr lst)
         (if cell
             (group-by-update acc k (append (cdr cell) (list x)))
             (append acc (list (cons k (list x)))))))))

(defun group-by-update (alist key newval)
  (mapcar (lambda (cell)
            (if (equal (car cell) key)
                (cons key newval)
                cell))
          alist))

(defun group-by (fn lst)
  "Group elements of LST by (FN element); return an alist of (key . elements)
with keys in first-seen order."
  (group-by-aux fn lst nil))

(defun frequencies (lst)
  "Count occurrences: an alist of (element . count), keys in first-seen
order (EQUAL comparison, like GROUP-BY)."
  (mapcar (lambda (cell) (cons (car cell) (length (cdr cell))))
          (group-by #'identity lst)))

(defun sort-by (lst keyfn &optional pred)
  "Sort LST ascending by (KEYFN element); collection first, like SORT.
PRED (default #'<) compares the extracted keys."
  (let ((cmp (if pred pred #'<)))
    (sort lst (lambda (a b) (funcall cmp (funcall keyfn a) (funcall keyfn b))))))

;;; ---- de-duplication / flattening -----------------------------------------

(defun remove-duplicates (lst)
  "Return LST with later EQUAL duplicates removed (keeps first occurrence)."
  (cond ((null lst) nil)
        ((member (car lst) (cdr lst))
         (cons (car lst) (remove-duplicates (remove-all (cdr lst) (car lst)))))
        (t (cons (car lst) (remove-duplicates (cdr lst))))))

(defun remove-all (lst item)
  "Return LST with every element EQUAL to ITEM removed."
  (filter (lambda (x) (not (equal x item))) lst))

(defun flatten (tree)
  "Flatten a nested list structure into a single flat list of leaves."
  (cond ((null tree) nil)
        ((atom tree) (list tree))
        (t (append (flatten (car tree)) (flatten (cdr tree))))))

;;; ---- combinators ---------------------------------------------------------

(defun identity (x)
  "Return X unchanged."
  x)

(defun complement (fn)
  "Return a predicate that negates FN."
  (lambda (&rest args) (not (apply fn args))))

(defun constantly (x)
  "Return a function that ignores its arguments and always returns X."
  (lambda (&rest args) x))

(defun compose (f g)
  "Return a function computing (F (G args...))."
  (lambda (&rest args) (funcall f (apply g args))))

(defun curry (fn &rest pre)
  "Return a function that prepends PRE to its arguments before calling FN."
  (lambda (&rest rest) (apply fn (append pre rest))))
