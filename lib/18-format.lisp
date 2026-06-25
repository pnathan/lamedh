;;; Formatting and string output (issue #150, epic #141).
;;;
;;; FORMAT implements a useful subset of Common Lisp format directives on top of
;;; the string primitives (#147) and the PRIN1-TO-STRING / PRINC-TO-STRING
;;; kernel builtins. Supported directives:
;;;
;;;   ~a / ~A   human (princ) rendering of the next argument
;;;   ~s / ~S   readable (prin1) rendering of the next argument
;;;   ~d / ~D   the next argument rendered as a (decimal) datum
;;;   ~%        newline
;;;   ~~        a literal tilde
;;;
;;; Destination: NIL returns the formatted string; T prints it and returns NIL.
;;;
;;; NOTE: READ-LINE and WITH-OUTPUT-TO-STRING are deferred — they need stdin /
;;; output-stream plumbing the kernel does not expose yet (#150 follow-up).

(defun format-build (ctrl args i n acc)
  (if (>= i n)
      acc
      (let ((c (substring ctrl i (+ i 1))))
        (if (equal c "~")
            (let ((d (substring ctrl (+ i 1) (+ i 2))))
              (cond
                ((equal d "%")
                 (format-build ctrl args (+ i 2) n (concat acc (code-char 10))))
                ((equal d "~")
                 (format-build ctrl args (+ i 2) n (concat acc "~")))
                ((or (equal d "a") (equal d "A"))
                 (format-build ctrl (cdr args) (+ i 2) n
                               (concat acc (princ-to-string (car args)))))
                ((or (equal d "s") (equal d "S"))
                 (format-build ctrl (cdr args) (+ i 2) n
                               (concat acc (prin1-to-string (car args)))))
                ((or (equal d "d") (equal d "D"))
                 (format-build ctrl (cdr args) (+ i 2) n
                               (concat acc (princ-to-string (car args)))))
                (t
                 ;; Unknown directive: pass it through literally.
                 (format-build ctrl args (+ i 2) n (concat acc "~" d)))))
            (format-build ctrl args (+ i 1) n (concat acc c))))))

(defun format (dest ctrl &rest args)
  "Format CTRL with ARGS. DEST NIL returns the string; DEST T prints it.
Directives: ~a ~s ~d ~% ~~ (see lib/18-format.lisp)."
  (let ((out (format-build ctrl args 0 (string-length ctrl) "")))
    (if (null dest)
        out
        (progn (princ out) nil))))
