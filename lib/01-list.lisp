(defun pairlis (keys vals)
  (if (or (null keys) (null vals))
      nil
      (cons (cons (car keys) (car vals))
            (pairlis (cdr keys) (cdr vals)))))

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

(defun copy (x)
  "Return a copy of list structure X."
  (cond ((atom x) x)
        (t (cons (copy (car x)) (copy (cdr x))))))

(defun sassoc (x y fn)
  "Search alist Y for key X; call FN with no args and return its result if not found."
  (cond ((null y) (funcall fn))
        ((equal x (caar y)) (car y))
        (t (sassoc x (cdr y) fn))))

(defun mapc (fn list)
  "Apply FN to each element of LIST for side effects; return LIST."
  (cond ((null list) list)
        (t (funcall fn (car list))
           (mapc fn (cdr list))
           list)))

(defun mapcon (fn list)
  "Apply FN to successive tails of LIST and NCONC the results."
  (cond ((null list) nil)
        (t (nconc (funcall fn list) (mapcon fn (cdr list))))))
