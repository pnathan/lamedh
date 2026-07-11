;;; rpn-calculator -- reverse Polish notation over a stack, with Result.
;;; Shows: tokenizing with string-split, a stack as a list, Result
;;; (ok/err) threading instead of exceptions, and try-call at the edge.
;;; Run: cargo run -- examples/rpn-calculator/main.lisp

(def $ops (make-hash-table))
(put! $ops "+" #'+)
(put! $ops "-" #'-)
(put! $ops "*" #'*)
(put! $ops "/" #'/)

(defun rpn-step (stack token)
  "Result of pushing TOKEN onto STACK."
  (let ((n (string->number token)))
    (cond (n (ok (cons n stack)))
          ((has-key-p $ops token)
           (if (or (null stack) (null (cdr stack)))
               (err (concat "stack underflow at " token))
               (let ((b (car stack)) (a (cadr stack)))
                 ;; try-call's err payload is the message string itself.
                 (variant-case (try-call (gethash $ops token) a b)
                   (ok (v) (ok (cons v (cdr (cdr stack)))))
                   (err (e) (err (concat token ": " e)))))))
          (t (err (concat "bad token " token))))))

(defun rpn-eval (expr)
  "Result of evaluating EXPR, e.g. \"3 4 + 2 *\"."
  (rpn-aux (string-split expr " ") (list)))

(defun rpn-aux (tokens stack)
  (cond ((null tokens)
         (if (and stack (null (cdr stack)))
             (ok (car stack))
             (err "leftover operands")))
        (t (variant-case (rpn-step stack (car tokens))
             (ok (s) (rpn-aux (cdr tokens) s))
             (err (e) (err e))))))

(for-each (lambda (expr)
            (variant-case (rpn-eval expr)
              (ok (v) (format t "~a = ~a~%" expr v))
              (err (e) (format t "~a => error: ~a~%" expr e))))
          '("3 4 + 2 *" "5 1 2 + 4 * + 3 -" "1 0 /" "2 +" "1 2"))

;; self-check: values, and every failure mode is an err, not a crash.
(if (and (equal (rpn-eval "3 4 + 2 *") (ok 14))
         (equal (rpn-eval "5 1 2 + 4 * + 3 -") (ok 14))
         (err-p (rpn-eval "1 0 /"))
         (err-p (rpn-eval "2 +"))
         (err-p (rpn-eval "1 2"))
         (err-p (rpn-eval "1 banana +")))
    (print 'ok)
    (error "rpn self-check failed"))
