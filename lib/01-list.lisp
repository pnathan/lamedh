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
