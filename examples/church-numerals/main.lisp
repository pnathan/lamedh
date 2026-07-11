;;; church-numerals -- arithmetic with nothing but lambdas.
;;; Shows: higher-order functions all the way down, closures as data,
;;; and converting between Church and native integers.
;;; Run: cargo run -- examples/church-numerals/main.lisp

(defun church (n)
  "The Church numeral for native N."
  (lambda (f) (lambda (x)
    (if (= n 0) x (funcall (funcall (church (- n 1)) f) (funcall f x))))))

(defun unchurch (c)
  "Back to a native integer."
  (funcall (funcall c #'1+) 0))

(defun c-succ (c)
  (lambda (f) (lambda (x) (funcall f (funcall (funcall c f) x)))))

(defun c-add (a b)
  (lambda (f) (lambda (x) (funcall (funcall a f) (funcall (funcall b f) x)))))

(defun c-mul (a b)
  (lambda (f) (funcall a (funcall b f))))

(def $two (church 2))
(def $three (church 3))

(format t "2 + 3 = ~a~%" (unchurch (c-add $two $three)))
(format t "2 * 3 = ~a~%" (unchurch (c-mul $two $three)))
(format t "succ 3 = ~a~%" (unchurch (c-succ $three)))

;; self-check: the arithmetic identities, round-trips, and zero.
(if (and (= (unchurch (c-add $two $three)) 5)
         (= (unchurch (c-mul $two $three)) 6)
         (= (unchurch (c-succ (church 0))) 1)
         (= (unchurch (church 9)) 9)
         (= (unchurch (c-mul (church 4) (c-add $two $two))) 16))
    (print 'ok)
    (error "church self-check failed"))
