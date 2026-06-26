;;; String operations — Lisp layer (issue #147, epic #141).
;;;
;;; Built on the Rust string primitives: STRING-LENGTH, SUBSTRING, CHAR-CODE,
;;; CODE-CHAR, STRING->NUMBER, NUMBER->STRING, plus CONCAT/INDEX.
;;;
;;; NAMING: predicates use the `-p` suffix (STARTS-WITH-P, WHITESPACE-P, ...).
;;;
;;; CHARACTERS: the reader's `'c'` literal is an integer code point (see #136).
;;; Functions in this layer accept EITHER a one-character string OR an integer
;;; code point — use CHAR->CODE to coerce. So (alpha-p 'a') and (alpha-p "a")
;;; both work. Case operations (CHAR-UPCASE, CHAR-DOWNCASE) always return a
;;; one-character string regardless of which form was given.

;;; ---- string <-> list of chars --------------------------------------------

(defun string->list-aux (s i n)
  (if (< i n)
      (cons (substring s i (+ i 1)) (string->list-aux s (+ i 1) n))
      nil))

(defun string->list (s)
  "Return the characters of S as a list of one-character strings."
  (string->list-aux s 0 (string-length s)))

(defun list->string (chars)
  "Concatenate a list of strings into one string."
  (apply #'concat chars))

;;; ---- char coercion -------------------------------------------------------

(defun char->code (c)
  "Return the integer code point of C, which may be a one-character string or
an integer code point (e.g. from a reader char literal like 'a')."
  (if (stringp c) (char-code c) c))

;;; ---- char classification (accept string or code-point integer) -----------

(defun digit-p (c)
  "True if C (one-character string or code point) is an ASCII digit 0-9."
  (let ((code (char->code c))) (and (>= code 48) (<= code 57))))

(defun alpha-p (c)
  "True if C (one-character string or code point) is an ASCII letter A-Z or a-z."
  (let ((code (char->code c)))
    (or (and (>= code 65) (<= code 90))
        (and (>= code 97) (<= code 122)))))

(defun alphanumeric-p (c)
  "True if C (one-character string or code point) is an ASCII letter or digit."
  (or (alpha-p c) (digit-p c)))

(defun char-upper-p (c)
  "True if C (one-character string or code point) is an ASCII uppercase letter A-Z."
  (let ((code (char->code c))) (and (>= code 65) (<= code 90))))

(defun char-lower-p (c)
  "True if C (one-character string or code point) is an ASCII lowercase letter a-z."
  (let ((code (char->code c))) (and (>= code 97) (<= code 122))))

(defun whitespace-p (c)
  "True if C (one-character string or code point) is space, tab, newline, or carriage return."
  (let ((code (char->code c)))
    (or (= code 32) (= code 9) (= code 10) (= code 13))))

;;; ---- char case mapping ---------------------------------------------------

(defun char-upcase (c)
  "Uppercase C (one-character string or code point). Returns a one-character string."
  (let ((code (char->code c)))
    (if (and (>= code 97) (<= code 122)) (code-char (- code 32)) (code-char code))))

(defun char-downcase (c)
  "Lowercase C (one-character string or code point). Returns a one-character string."
  (let ((code (char->code c)))
    (if (and (>= code 65) (<= code 90)) (code-char (+ code 32)) (code-char code))))

;;; ---- case mapping --------------------------------------------------------

(defun string-upcase (s)
  "Return S with ASCII letters uppercased."
  (list->string (mapcar #'char-upcase (string->list s))))

(defun string-downcase (s)
  "Return S with ASCII letters lowercased."
  (list->string (mapcar #'char-downcase (string->list s))))

;;; ---- comparison ----------------------------------------------------------

(defun string= (a b)
  "True if strings A and B have the same contents."
  (equal a b))

(defun string-lessp-aux (a b i la lb)
  (cond ((>= i la) (< la lb))
        ((>= i lb) nil)
        (t (let ((ca (char-code (substring a i (+ i 1))))
                 (cb (char-code (substring b i (+ i 1)))))
             (cond ((< ca cb) t)
                   ((> ca cb) nil)
                   (t (string-lessp-aux a b (+ i 1) la lb)))))))

(defun string-lessp (a b)
  "True if string A is lexicographically (by code point) before string B."
  (string-lessp-aux a b 0 (string-length a) (string-length b)))

;;; ---- search --------------------------------------------------------------

(defun string-index-of-aux (s sub i n m)
  (cond ((> (+ i m) n) nil)
        ((equal (substring s i (+ i m)) sub) i)
        (t (string-index-of-aux s sub (+ i 1) n m))))

(defun string-index-of (s sub)
  "Return the index of the first occurrence of SUB in S, or NIL."
  (string-index-of-aux s sub 0 (string-length s) (string-length sub)))

(defun contains-p (s sub)
  "True if SUB occurs anywhere in S."
  (not (null (string-index-of s sub))))

(defun starts-with-p (s prefix)
  "True if S begins with PREFIX."
  (let ((lp (string-length prefix)))
    (and (<= lp (string-length s))
         (equal (substring s 0 lp) prefix))))

(defun ends-with-p (s suffix)
  "True if S ends with SUFFIX."
  (let ((ls (string-length suffix))
        (n (string-length s)))
    (and (<= ls n)
         (equal (substring s (- n ls) n) suffix))))

;;; ---- transformation ------------------------------------------------------

(defun string-replace (s old new)
  "Replace every (non-empty) occurrence of OLD in S with NEW."
  (let ((idx (string-index-of s old)))
    (if (or (null idx) (= (string-length old) 0))
        s
        (concat (substring s 0 idx)
                new
                (string-replace
                 (substring s (+ idx (string-length old)) (string-length s))
                 old new)))))

(defun string-split (s delim)
  "Split S on (non-empty) string DELIM into a list of substrings."
  (let ((idx (string-index-of s delim)))
    (if (or (null idx) (= (string-length delim) 0))
        (list s)
        (cons (substring s 0 idx)
              (string-split
               (substring s (+ idx (string-length delim)) (string-length s))
               delim)))))

(defun string-join (lst sep)
  "Join a list of strings LST with separator SEP."
  (cond ((null lst) "")
        ((null (cdr lst)) (car lst))
        (t (concat (car lst) sep (string-join (cdr lst) sep)))))

(defun string-trim-left (s i n)
  (if (and (< i n) (whitespace-p (substring s i (+ i 1))))
      (string-trim-left s (+ i 1) n)
      i))

(defun string-trim-right (s end)
  (if (and (> end 0) (whitespace-p (substring s (- end 1) end)))
      (string-trim-right s (- end 1))
      end))

(defun string-trim (s)
  "Remove leading and trailing whitespace from S."
  (let* ((n (string-length s))
         (start (string-trim-left s 0 n))
         (end (string-trim-right s n)))
    (if (< start end) (substring s start end) "")))
