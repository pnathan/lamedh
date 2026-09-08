;;;; reader.lisp -- hand-written recursive-descent reader for Lamedh syntax.
;;;;
;;;; Deliberately independent of the CL reader (Lamedh has its own token
;;;; grammar: octal Q-suffix and hex H-suffix integers, a `'c'` character
;;;; literal distinct from the quote reader macro, earmuff/keyword symbol
;;;; classes, etc.) -- see src/reader.rs in the reference implementation
;;;; for the grammar this mirrors.

(in-package #:lamedh-rt)

;;; A Lamedh record/struct value (DEFRECORD/DEFSTRUCT-TYPED's runtime
;;; representation -- see runtime.lisp's "records" section for the
;;; constructor/accessor primitives). Defined here, ahead of RUNTIME.LISP,
;;; because the #S(...) reader literal below constructs one directly.
(defstruct lamedh-struct type-name values)

(defstruct (cursor (:constructor %make-cursor (text len &optional (pos 0))))
  (text "" :type simple-string)
  (len 0 :type fixnum)
  (pos 0 :type fixnum))

(defun make-cursor (string)
  (let ((s (coerce string 'simple-string)))
    (%make-cursor s (length s))))

(declaim (inline cur-eof-p cur-peek cur-peek2 cur-advance))
(defun cur-eof-p (c) (>= (cursor-pos c) (cursor-len c)))
(defun cur-peek (c &optional (offset 0))
  (let ((i (+ (cursor-pos c) offset)))
    (if (< i (cursor-len c)) (char (cursor-text c) i) nil)))
(defun cur-advance (c) (incf (cursor-pos c)))

(define-condition lamedh-reader-error (error)
  ((message :initarg :message :reader lamedh-reader-error-message))
  (:report (lambda (c s) (format s "~A" (lamedh-reader-error-message c)))))

(defun reader-error* (fmt &rest args)
  (error 'lamedh-reader-error :message (apply #'format nil fmt args)))

;;; ---- whitespace and comments -----------------------------------------------

(defun skip-ws (c)
  (loop
    (cond
      ((cur-eof-p c) (return))
      ((member (cur-peek c) '(#\Space #\Tab #\Newline #\Return #\Linefeed #\Page))
       (cur-advance c))
      ((eql (cur-peek c) #\;)
       (loop until (or (cur-eof-p c) (eql (cur-peek c) #\Newline)) do (cur-advance c)))
      ((and (eql (cur-peek c) #\#) (eql (cur-peek c 1) #\|))
       (cur-advance c) (cur-advance c)
       (let ((depth 1))
         (loop while (> depth 0) do
           (cond
             ((cur-eof-p c) (reader-error* "unterminated block comment"))
             ((and (eql (cur-peek c) #\#) (eql (cur-peek c 1) #\|))
              (incf depth) (cur-advance c) (cur-advance c))
             ((and (eql (cur-peek c) #\|) (eql (cur-peek c 1) #\#))
              (decf depth) (cur-advance c) (cur-advance c))
             (t (cur-advance c))))))
      (t (return)))))

(defun strip-shebang (string)
  (if (and (>= (length string) 2) (char= (char string 0) #\#) (char= (char string 1) #\!))
      (let ((nl (position #\Newline string)))
        (if nl (subseq string nl) ""))
      string))

;;; ---- symbol/atom character classes -----------------------------------------

(defun sym-initial-p (ch)
  (and ch (or (alpha-char-p ch) (member ch '(#\& #\$ #\?)))))

(defun sym-continue-p (ch)
  (and ch (or (alphanumericp ch)
              (member ch '(#\- #\* #\? #\! #\+ #\= #\< #\> #\: #\_)))))

(defun operator-char-p (ch)
  (and ch (member ch '(#\+ #\- #\* #\/ #\= #\< #\> #\! #\~))))

(defun intern-lamedh (name)
  (intern name (find-package "LAMEDH")))

;;; ---- numbers ----------------------------------------------------------------

(defun try-read-number (c)
  "Attempt to read a number at the cursor; return (values value t) on
success, leaving the cursor advanced, or (values nil nil) with the cursor
unchanged. NOTE: does not chain the sub-readers with OR -- OR only
propagates multiple values from its LAST subform, and every non-winning
attempt here needs its own (values nil nil) preserved as a real two-value
result, not collapsed to a single value by a short-circuiting OR."
  (let ((start (cursor-pos c)))
    (dolist (reader (list #'try-read-float #'try-read-radix #'try-read-hex
                          #'try-read-octal #'try-read-decimal))
      (setf (cursor-pos c) start)
      (multiple-value-bind (v ok) (funcall reader c)
        (when ok (return-from try-read-number (values v t)))))
    (setf (cursor-pos c) start)
    (values nil nil)))

(defun read-digits (c)
  (let ((start (cursor-pos c)))
    (loop while (and (cur-peek c) (digit-char-p (cur-peek c))) do (cur-advance c))
    (subseq (cursor-text c) start (cursor-pos c))))

(defun try-read-float (c)
  (let ((start (cursor-pos c)))
    (when (eql (cur-peek c) #\-) (cur-advance c))
    (let ((int-part (read-digits c)))
      (when (and (plusp (length int-part)) (eql (cur-peek c) #\.))
        (cur-advance c)
        (let ((frac-part (read-digits c)))
          (when (plusp (length frac-part))
            (when (member (cur-peek c) '(#\e #\E))
              (cur-advance c)
              (when (member (cur-peek c) '(#\+ #\-)) (cur-advance c))
              (read-digits c))
            (return-from try-read-float
              (values (let ((*read-default-float-format* 'double-float))
                        (coerce (read-from-string (subseq (cursor-text c) start (cursor-pos c))) 'double-float))
                      t)))))
      (setf (cursor-pos c) start)
      (values nil nil))))

(defun try-read-radix (c)
  (unless (eql (cur-peek c) #\#) (return-from try-read-radix (values nil nil)))
  (let ((marker (cur-peek c 1)))
    (unless (member marker '(#\x #\X #\b #\B #\o #\O)) (return-from try-read-radix (values nil nil)))
    (let ((radix (case (char-downcase marker) (#\x 16) (#\b 2) (t 8))))
      (cur-advance c) (cur-advance c)
      (let ((neg (eql (cur-peek c) #\-)))
        (when neg (cur-advance c))
        (let ((start (cursor-pos c)))
          (loop while (and (cur-peek c) (digit-char-p (cur-peek c) radix)) do (cur-advance c))
          (when (= start (cursor-pos c)) (reader-error* "malformed radix literal"))
          (when (and (cur-peek c) (or (alphanumericp (cur-peek c)) (eql (cur-peek c) #\-)))
            (reader-error* "malformed radix literal"))
          (let ((n (parse-integer (cursor-text c) :start start :end (cursor-pos c) :radix radix)))
            (values (if neg (- n) n) t)))))))

(defun try-read-hex (c)
  (let ((start (cursor-pos c)) (neg (eql (cur-peek c) #\-)))
    (when neg (cur-advance c))
    (let ((digits-start (cursor-pos c)))
      (unless (and (cur-peek c) (digit-char-p (cur-peek c)))
        (setf (cursor-pos c) start) (return-from try-read-hex (values nil nil)))
      (loop while (and (cur-peek c) (digit-char-p (cur-peek c) 16)) do (cur-advance c))
      (if (member (cur-peek c) '(#\h #\H))
          (let ((digits (subseq (cursor-text c) digits-start (cursor-pos c))))
            (cur-advance c)
            (if (and (cur-peek c) (or (alphanumericp (cur-peek c)) (eql (cur-peek c) #\-)))
                (progn (setf (cursor-pos c) start) (values nil nil))
                (let ((n (parse-integer digits :radix 16)))
                  (values (if neg (- n) n) t))))
          (progn (setf (cursor-pos c) start) (values nil nil))))))

(defun try-read-octal (c)
  (let ((start (cursor-pos c)) (neg (eql (cur-peek c) #\-)))
    (when neg (cur-advance c))
    (let ((digits-start (cursor-pos c)))
      (unless (and (cur-peek c) (digit-char-p (cur-peek c)))
        (setf (cursor-pos c) start) (return-from try-read-octal (values nil nil)))
      (loop while (and (cur-peek c) (digit-char-p (cur-peek c))) do (cur-advance c))
      (if (eql (cur-peek c) #\Q)
          (let ((digits (subseq (cursor-text c) digits-start (cursor-pos c))))
            (cur-advance c)
            (handler-case
                (let ((n (parse-integer digits :radix 8)))
                  (values (if neg (- n) n) t))
              (error () (setf (cursor-pos c) start) (values nil nil))))
          (progn (setf (cursor-pos c) start) (values nil nil))))))

(defun try-read-decimal (c)
  (let ((start (cursor-pos c)) (neg (eql (cur-peek c) #\-)))
    (when neg (cur-advance c))
    (let ((digits (read-digits c)))
      (if (plusp (length digits))
          (values (let ((n (parse-integer digits))) (if neg (- n) n)) t)
          (progn (setf (cursor-pos c) start) (values nil nil))))))

;;; ---- character literal 'c' vs quote -----------------------------------------

(defun try-read-char-literal (c)
  (unless (eql (cur-peek c) #\') (return-from try-read-char-literal (values nil nil)))
  (let ((start (cursor-pos c)))
    (cur-advance c)
    (when (or (cur-eof-p c) (eql (cur-peek c) #\'))
      (setf (cursor-pos c) start) (return-from try-read-char-literal (values nil nil)))
    (let (code)
      (if (eql (cur-peek c) #\\)
          (progn (cur-advance c)
                 (when (cur-eof-p c) (setf (cursor-pos c) start)
                       (return-from try-read-char-literal (values nil nil)))
                 (let ((c1 (cur-peek c)))
                   (cur-advance c)
                   (setf code (case c1 (#\n 10) (#\t 9) (#\r 13) (#\\ 92) (#\' 39) (#\0 0)
                                (t (char-code c1))))))
          (progn (setf code (char-code (cur-peek c))) (cur-advance c)))
      (if (and (not (cur-eof-p c)) (eql (cur-peek c) #\') (<= code 255))
          (progn (cur-advance c) (values (code-char code) t))
          (progn (setf (cursor-pos c) start) (values nil nil))))))

;;; ---- strings ------------------------------------------------------------------

(defun read-string-literal (c)
  (cur-advance c) ; opening quote
  (let ((out (make-array 0 :element-type 'character :adjustable t :fill-pointer 0)))
    (loop
      (when (cur-eof-p c) (reader-error* "unterminated string literal"))
      (let ((ch (cur-peek c)))
        (cond
          ((char= ch #\") (cur-advance c) (return))
          ((char= ch #\\)
           (cur-advance c)
           (when (cur-eof-p c) (reader-error* "unterminated string escape"))
           (let ((e (cur-peek c))) (cur-advance c)
             (case e
               (#\n (vector-push-extend #\Newline out))
               (#\t (vector-push-extend #\Tab out))
               (#\r (vector-push-extend #\Return out))
               (#\\ (vector-push-extend #\\ out))
               (#\" (vector-push-extend #\" out))
               (#\0 (vector-push-extend (code-char 0) out))
               (t (vector-push-extend #\\ out) (vector-push-extend e out)))))
          (t (vector-push-extend ch out) (cur-advance c)))))
    (coerce out 'simple-string)))

;;; ---- symbols ------------------------------------------------------------------

(defun read-symbol-name (c)
  (let ((start (cursor-pos c)))
    (cur-advance c)
    (loop while (sym-continue-p (cur-peek c)) do (cur-advance c))
    (string-upcase (subseq (cursor-text c) start (cursor-pos c)))))

(defun try-read-earmuff (c)
  (let ((start (cursor-pos c)))
    (unless (eql (cur-peek c) #\*) (return-from try-read-earmuff (values nil nil)))
    (cur-advance c)
    (unless (and (cur-peek c) (alpha-char-p (cur-peek c)))
      (setf (cursor-pos c) start) (return-from try-read-earmuff (values nil nil)))
    (loop while (or (alphanumericp (cur-peek c)) (eql (cur-peek c) #\-)) do (cur-advance c))
    (if (eql (cur-peek c) #\*)
        (progn (cur-advance c)
               (values (intern-lamedh (string-upcase (subseq (cursor-text c) start (cursor-pos c)))) t))
        (progn (setf (cursor-pos c) start) (values nil nil)))))

(defun try-read-one-plus-minus (c)
  (let ((start (cursor-pos c)))
    (when (and (eql (cur-peek c) #\1) (member (cur-peek c 1) '(#\+ #\-)))
      (let ((sym (if (eql (cur-peek c 1) #\+) "1+" "1-")))
        (cur-advance c) (cur-advance c)
        (unless (sym-continue-p (cur-peek c))
          (return-from try-read-one-plus-minus (values (intern-lamedh sym) t)))))
    (setf (cursor-pos c) start)
    (values nil nil)))

(defun read-atom (c)
  (multiple-value-bind (v ok) (try-read-one-plus-minus c) (when ok (return-from read-atom v)))
  (multiple-value-bind (v ok) (try-read-number c) (when ok (return-from read-atom v)))
  (multiple-value-bind (v ok) (try-read-earmuff c) (when ok (return-from read-atom v)))
  (when (eql (cur-peek c) #\:)
    (let ((start (cursor-pos c)))
      (cur-advance c)
      (if (sym-initial-p (cur-peek c))
          (progn
            (cur-advance c)
            (loop while (sym-continue-p (cur-peek c)) do (cur-advance c))
            (return-from read-atom
              (intern-lamedh (string-upcase (subseq (cursor-text c) start (cursor-pos c))))))
          (setf (cursor-pos c) start))))
  (when (sym-initial-p (cur-peek c))
    (let ((name (read-symbol-name c)))
      (return-from read-atom
        (cond ((string= name "T") (intern-lamedh "T"))
              ((string= name "NIL") nil)
              (t (intern-lamedh name))))))
  (when (operator-char-p (cur-peek c))
    (let ((start (cursor-pos c)))
      (loop while (operator-char-p (cur-peek c)) do (cur-advance c))
      (return-from read-atom (intern-lamedh (subseq (cursor-text c) start (cursor-pos c))))))
  (reader-error* "unexpected character '~A' at position ~D" (cur-peek c) (cursor-pos c)))

;;; ---- top-level dispatch --------------------------------------------------------

(defvar *quote-sym*)
(defvar *quasiquote-sym*)
(defvar *unquote-sym*)
(defvar *unquote-splicing-sym*)
(defvar *function-sym*)

(defun init-reader-symbols ()
  (setf *quote-sym* (intern-lamedh "QUOTE")
        *quasiquote-sym* (intern-lamedh "QUASIQUOTE")
        *unquote-sym* (intern-lamedh "UNQUOTE")
        *unquote-splicing-sym* (intern-lamedh "UNQUOTE-SPLICING")
        *function-sym* (intern-lamedh "FUNCTION")))

(defun read-form (c)
  (skip-ws c)
  (when (cur-eof-p c) (reader-error* "unexpected end of input"))
  (let ((ch (cur-peek c)))
    (cond
      ((and (char= ch #\#) (member (cur-peek c 1) '(#\S #\s)))
       (cur-advance c) (cur-advance c)
       (let ((body (read-list c)))
         (unless (consp body) (reader-error* "#S(...) requires a brand and fields"))
         (make-lamedh-struct :type-name (car body) :values (coerce (cdr body) 'simple-vector))))
      ((char= ch #\() (read-list c))
      ((char= ch #\") (read-string-literal c))
      ((char= ch #\')
       (multiple-value-bind (v ok) (try-read-char-literal c)
         (if ok v
             (progn (cur-advance c) (list *quote-sym* (read-form c))))))
      ((char= ch #\`) (cur-advance c) (list *quasiquote-sym* (read-form c)))
      ((and (char= ch #\,) (eql (cur-peek c 1) #\@))
       (cur-advance c) (cur-advance c) (list *unquote-splicing-sym* (read-form c)))
      ((char= ch #\,) (cur-advance c) (list *unquote-sym* (read-form c)))
      ((and (char= ch #\#) (eql (cur-peek c 1) #\'))
       (cur-advance c) (cur-advance c) (list *function-sym* (read-form c)))
      (t (read-atom c)))))

(defun read-list (c)
  (cur-advance c) ; (
  (let (items tail (has-tail nil))
    (loop
      (skip-ws c)
      (when (cur-eof-p c) (reader-error* "unterminated list: missing ')'"))
      (when (eql (cur-peek c) #\)) (cur-advance c) (return))
      (when (and (eql (cur-peek c) #\.) (not (sym-continue-p (cur-peek c 1))))
        (cur-advance c)
        (when (null items) (reader-error* "'.' with no leading list element"))
        (setf tail (read-form c) has-tail t)
        (skip-ws c)
        (unless (eql (cur-peek c) #\)) (reader-error* "malformed dotted list"))
        (cur-advance c)
        (return))
      (push (read-form c) items))
    (let ((result (if has-tail tail nil)))
      (dolist (item items) (setf result (cons item result)))
      result)))

;;; ---- public entry points --------------------------------------------------------

(defun lread (string)
  "Read exactly one Lamedh form from STRING, erroring on trailing garbage."
  (let ((c (make-cursor (strip-shebang string))))
    (let ((form (read-form c)))
      (skip-ws c)
      (unless (cur-eof-p c) (reader-error* "unexpected trailing input"))
      form)))

(defun lread-all (string)
  "Read every top-level Lamedh form from STRING; returns a list."
  (let ((c (make-cursor (strip-shebang string))) (out nil))
    (loop
      (skip-ws c)
      (when (cur-eof-p c) (return (nreverse out)))
      (push (read-form c) out))))

(eval-when (:load-toplevel :execute) (init-reader-symbols))
