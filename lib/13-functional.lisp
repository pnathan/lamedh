;;; Functional list toolkit (issue #143, epic #141).
;;;
;;; CONVENTION: like the existing MAPCAR/MAPC/MAPCON in this codebase, every
;;; higher-order function here takes the COLLECTION FIRST and the function or
;;; predicate LAST. This is the documented lamedh deviation from Common Lisp
;;; argument order; we keep it consistent across the whole toolkit.
;;;
;;; All functions are non-mutating.

;;; ---- folding / reduction -------------------------------------------------

(defun foldl-aux (fn acc lst)
  (if (null lst)
      acc
      (foldl-aux fn (funcall fn acc (car lst)) (cdr lst))))

(defun reduce (lst fn &rest init)
  "Reduce LST left-to-right with binary FN.
With no INIT: seeds from the first element; an empty LST calls (FN) with no args.
With INIT supplied: folds starting from INIT (so an empty LST returns INIT)."
  (if init
      (foldl-aux fn (car init) lst)
      (cond ((null lst) (funcall fn))
            ((null (cdr lst)) (car lst))
            (t (foldl-aux fn (car lst) (cdr lst))))))

(defun foldr (lst fn init)
  "Right fold: (FN e0 (FN e1 (... (FN eN INIT))))."
  (if (null lst)
      init
      (funcall fn (car lst) (foldr (cdr lst) fn init))))

;;; ---- filtering -----------------------------------------------------------

(defun filter (lst pred)
  "Return the elements of LST for which PRED is non-NIL."
  (cond ((null lst) nil)
        ((funcall pred (car lst)) (cons (car lst) (filter (cdr lst) pred)))
        (t (filter (cdr lst) pred))))

(defun remove-if-not (lst pred)
  "Alias for FILTER: keep elements satisfying PRED."
  (filter lst pred))

(defun remove-if (lst pred)
  "Return the elements of LST for which PRED is NIL."
  (filter lst (lambda (x) (not (funcall pred x)))))

;;; ---- searching -----------------------------------------------------------

(defun find-if (lst pred)
  "Return the first element of LST satisfying PRED, or NIL."
  (cond ((null lst) nil)
        ((funcall pred (car lst)) (car lst))
        (t (find-if (cdr lst) pred))))

(defun find (lst item)
  "Return ITEM if it is EQUAL to some element of LST, else NIL."
  (find-if lst (lambda (x) (equal x item))))

(defun position-if-aux (lst pred i)
  (cond ((null lst) nil)
        ((funcall pred (car lst)) i)
        (t (position-if-aux (cdr lst) pred (+ i 1)))))

(defun position-if (lst pred)
  "Return the 0-based index of the first element satisfying PRED, or NIL."
  (position-if-aux lst pred 0))

(defun position (lst item)
  "Return the 0-based index of the first element EQUAL to ITEM, or NIL."
  (position-if lst (lambda (x) (equal x item))))

(defun count-if (lst pred)
  "Count the elements of LST satisfying PRED."
  (cond ((null lst) 0)
        ((funcall pred (car lst)) (+ 1 (count-if (cdr lst) pred)))
        (t (count-if (cdr lst) pred))))

;;; ---- quantifiers ---------------------------------------------------------

(defun every (lst pred)
  "Non-NIL iff PRED holds for every element of LST."
  (cond ((null lst) t)
        ((funcall pred (car lst)) (every (cdr lst) pred))
        (t nil)))

(defun some (lst pred)
  "Return the first non-NIL (PRED element), or NIL."
  (if (null lst)
      nil
      (let ((r (funcall pred (car lst))))
        (if r r (some (cdr lst) pred)))))

(defun notany (lst pred)
  "Non-NIL iff PRED holds for no element of LST."
  (not (some lst pred)))

(defun notevery (lst pred)
  "Non-NIL iff PRED fails for at least one element of LST."
  (not (every lst pred)))

;;; ---- mapping that concatenates -------------------------------------------

(defun mapcan (lst fn)
  "Map FN over LST and APPEND the resulting lists."
  (if (null lst)
      nil
      (append (funcall fn (car lst)) (mapcan (cdr lst) fn))))

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

(defun take-while (lst pred)
  "Return the leading elements of LST that satisfy PRED."
  (cond ((null lst) nil)
        ((funcall pred (car lst)) (cons (car lst) (take-while (cdr lst) pred)))
        (t nil)))

(defun drop-while (lst pred)
  "Return LST after dropping its leading elements that satisfy PRED."
  (cond ((null lst) nil)
        ((funcall pred (car lst)) (drop-while (cdr lst) pred))
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

(defun list-tabulate (n fn)
  "Return the list ((fn 0) (fn 1) ... (fn N-1))."
  (mapcar (iota n) fn))

;;; ---- partitioning / grouping ---------------------------------------------

(defun partition (lst pred)
  "Return (MATCHES NON-MATCHES) splitting LST by PRED, order preserved."
  (list (filter lst pred)
        (remove-if lst pred)))

(defun group-by-aux (lst fn acc)
  (if (null lst)
      acc
      (let* ((x (car lst))
             (k (funcall fn x))
             (cell (assoc k acc)))
        (group-by-aux
         (cdr lst) fn
         (if cell
             (group-by-update acc k (append (cdr cell) (list x)))
             (append acc (list (cons k (list x)))))))))

(defun group-by-update (alist key newval)
  (mapcar alist (lambda (cell)
                  (if (equal (car cell) key)
                      (cons key newval)
                      cell))))

(defun group-by (lst fn)
  "Group elements of LST by (FN element); return an alist of (key . elements)
with keys in first-seen order."
  (group-by-aux lst fn nil))

;;; ---- de-duplication / flattening -----------------------------------------

(defun remove-duplicates (lst)
  "Return LST with later EQUAL duplicates removed (keeps first occurrence)."
  (cond ((null lst) nil)
        ((member (car lst) (cdr lst))
         (cons (car lst) (remove-duplicates (remove-all (cdr lst) (car lst)))))
        (t (cons (car lst) (remove-duplicates (cdr lst))))))

(defun remove-all (lst item)
  "Return LST with every element EQUAL to ITEM removed."
  (filter lst (lambda (x) (not (equal x item)))))

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
