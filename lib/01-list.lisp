(defun pairlis (keys vals)
  (if (or (null keys) (null vals))
      nil
      (cons (cons (car keys) (car vals))
            (pairlis (cdr keys) (cdr vals)))))

(defun null (x)
  (eq x nil))


;; Essential list functions
(defun append (x y)
  (cond ((null x) y)
        (t (cons (car x) (append (cdr x) y)))))

(defun member (item list)
  (cond ((null list) nil)
        ((equal item (car list)) list)
        (t (member item (cdr list)))))

(defun length (list)
  (cond ((null list) 0)
        (t (+ 1 (length (cdr list))))))

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

;; List access functions
(defun nth (n list)
  "Return the nth element of list (0-indexed)."
  (if (or (null list) (< n 0))
      nil
      (if (= n 0)
          (car list)
          (nth (- n 1) (cdr list)))))

(defun nthcdr (n list)
  "Return the nth cdr of list."
  (if (or (null list) (< n 1))
      list
      (nthcdr (- n 1) (cdr list))))

(defun last (list)
  "Return the last cons cell of list."
  (cond ((null list) nil)
        ((null (cdr list)) list)
        (t (last (cdr list)))))

;; Association list functions
(defun assoc (key alist)
  "Find the first pair in alist whose car equals key."
  (cond ((null alist) nil)
        ((and (consp (car alist)) (eq key (car (car alist)))) (car alist))
        (t (assoc key (cdr alist)))))

;; Tree substitution
(defun subst (new old tree)
  "Substitute new for old in tree."
  (cond ((eq tree old) new)
        ((atom tree) tree)
        (t (cons (subst new old (car tree))
                 (subst new old (cdr tree))))))

;; Mapping functions
(defun mapcar (list fn)
  "Apply fn to each element of list, return list of results."
  (if (null list)
      nil
      (cons (funcall fn (car list))
            (mapcar (cdr list) fn))))

(defun maplist (list fn)
  "Apply fn to successive cdrs of list, return list of results."
  (if (null list)
      nil
      (cons (funcall fn list)
            (maplist (cdr list) fn))))
