(defun $pairlis-aux (keys vals acc)
  (if (or (null keys) (null vals))
      (reverse-aux acc nil)
      ($pairlis-aux (cdr keys) (cdr vals)
                     (cons (cons (car keys) (car vals)) acc))))

(defun pairlis (keys vals)
  ($pairlis-aux keys vals nil))

(defun null (x)
  (eq x nil))


;; APPEND is a variadic kernel builtin as of 0.3 (regularity + hot path).

(defun member (item list)
  (cond ((null list) nil)
        ((equal item (car list)) list)
        (t (member item (cdr list)))))

(defun length (list)
  ($length list))

(defun reverse-aux (lst acc)
  (if (null lst)
      acc
      (reverse-aux (cdr lst) (cons (car lst) acc))))

(defun reverse (list)
  (reverse-aux list nil))

(defun consp (x)
  "Test if x is a cons cell."
  (not (atom x)))

(defun listp (x)
  "Test if x is a list (a cons cell or nil)."
  (or (null x) (consp x)))

;; Lisp 1.5 list functions

(defun nconc (x y)
  "Append X and Y; in this implementation equivalent to APPEND (no destructive modification)."
  (append x y))

(defun $copy-aux (x acc)
  (if (atom x)
      (reverse-aux acc x)
      ($copy-aux (cdr x) (cons (copy (car x)) acc))))

(defun copy (x)
  "Return a copy of list structure X."
  (cond ((atom x) x)
        (t ($copy-aux x nil))))

(defun sassoc (x y fn)
  "Search alist Y for key X; call FN with no args and return its result if not found."
  (cond ((null y) (funcall fn))
        ((equal x (caar y)) (car y))
        (t (sassoc x (cdr y) fn))))

(defun $mapc-aux (fn tail)
  (cond ((null tail) nil)
        (t (funcall fn (car tail))
           ($mapc-aux fn (cdr tail)))))

(defun mapc (fn list)
  "Apply FN to each element of LIST for side effects; return LIST."
  ($mapc-aux fn list)
  list)

;; NCONC the reversed result lists RS right to left in constant stack: the
;; same NCONC calls, in the same order, as the nested
;; (nconc r1 (nconc r2 ... (nconc rn nil))) form performs.
(defun $nconc-reversed-aux (rs acc)
  (if (null rs)
      acc
      ($nconc-reversed-aux (cdr rs) (nconc (car rs) acc))))

(defun $mapcon-aux (fn tail rs)
  (if (null tail)
      ($nconc-reversed-aux rs nil)
      ($mapcon-aux fn (cdr tail) (cons (funcall fn tail) rs))))

(defun mapcon (fn list)
  "Apply FN to successive tails of LIST and NCONC the results."
  ($mapcon-aux fn list nil))
