;;; caesar-cipher -- shift letters, leave everything else alone.
;;; Shows: char->code/code-char, the map protocol on strings (0.3,
;;; string -> string), and an inverse-function self-check.
;;; Run: cargo run -- examples/caesar-cipher/main.lisp

(defun shift-char (c k)
  "Shift letter C by K places; non-letters pass through."
  (let ((code (char->code c)))
    (cond ((and (>= code 65) (<= code 90))
           (code-char (+ 65 (mod (+ (- code 65) k) 26))))
          ((and (>= code 97) (<= code 122))
           (code-char (+ 97 (mod (+ (- code 97) k) 26))))
          (t c))))

(defun caesar (s k)
  (map (lambda (c) (shift-char c k)) s))

(def $plain "Attack at dawn, Lamedh!")
(def $cipher (caesar $plain 13))
(format t "plain:  ~a~%" $plain)
(format t "rot13:  ~a~%" $cipher)
(format t "back:   ~a~%" (caesar $cipher 13))

;; self-check: decrypt inverts encrypt for every shift.
(if (every (lambda (k) (equal (caesar (caesar $plain k) (- 26 k)) $plain))
           (iota 26))
    (print 'ok)
    (error "caesar round-trip failed"))
