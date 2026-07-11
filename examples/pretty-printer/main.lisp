;;; pretty-printer -- render s-expressions with indentation.
;;; Shows: code-as-data traversal, width-aware layout decisions,
;;; string-pad-left (0.3), and read-back verification (print then
;;; re-read gives an equal structure).
;;; Run: cargo run -- examples/pretty-printer/main.lisp

(def $width 40)

(defun render-flat (e)
  (prin1-to-string e))

(defun pp (e indent)
  "Pretty string for E at INDENT columns."
  (let ((flat (render-flat e)))
    (if (or (atom e) (<= (+ indent (string-length* flat)) $width))
        flat
        ;; Break: head on the first line, args aligned beneath.
        (let ((head (render-flat (car e)))
              (pad (string-repeat " " (+ indent 2))))
          (concat "(" head
                  (string-join
                   (mapcar (lambda (sub)
                             (concat "\n" pad (pp sub (+ indent 2))))
                           (cdr e))
                   "")
                  ")")))))

(defun pretty (e) (pp e 0))

(def $form
  '(defun quicksort (lst)
     (if (null lst)
         ()
         (append (quicksort (filter (lambda (x) (< x (car lst))) (cdr lst)))
                 (list (car lst))
                 (quicksort (filter (lambda (x) (>= x (car lst))) (cdr lst)))))))

(format t "~a~%" (pretty $form))

;; self-check: reading the pretty output back yields the same structure,
;; short forms stay on one line, long forms actually break.
(def $reread (car (read-string (pretty $form))))
(if (and (equal $reread $form)
         (equal (pretty '(+ 1 2)) "(+ 1 2)")
         (contains-p (pretty $form) "\n"))
    (print 'ok)
    (error "pretty-printer self-check failed"))
