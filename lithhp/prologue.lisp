(defmacro defun (name params body)
  `(def ,name (lambda ,params ,body)))

(defun null (x)
  (eq x nil))

(defun pairlis (keys vals)
  (if (or (null keys) (null vals))
      nil
      (cons (cons (car keys) (car vals))
            (pairlis (cdr keys) (cdr vals)))))

(defun caar (x) (car (car x)))
(defun cadr (x) (car (cdr x)))
(defun cdar (x) (cdr (car x)))
(defun cddr (x) (cdr (cdr x)))

(defun member (item list)
  (if (null list)
      nil
      (if (eq item (car list))
          list
          (member item (cdr list)))))

(defun get-properties (plist indicators)
  (if (null plist)
      nil
      (if (member (car plist) indicators)
          (cons (car plist) (cons (cadr plist) nil))
          (get-properties (cddr plist) indicators))))
