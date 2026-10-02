;;; String operations — Lisp layer (issue #147, epic #141; completed and
;;; stabilized by issue #254, epic #253).
;;;
;;; Built on the Rust string primitives: STRING-LENGTH*, SUBSTRING, CHAR-CODE,
;;; CODE-CHAR, STRING->NUMBER, NUMBER->STRING, STRING-CASEFOLD*, plus
;;; CONCAT/INDEX, and the one-pass walks STRING->LIST*, STRING-SPLIT* and
;;; STRING-JOIN* (issue #510: SUBSTRING indexes by character, so a Lisp loop
;;; over it rescans the UTF-8 string each step and is quadratic).
;;; STRING-CASEFOLD* is Unicode-aware and locale-independent (Rust's default
;;; case fold); it backs the STRING-CI= family below.
;;; STRING-UPCASE*/STRING-DOWNCASE* (Unicode full case mapping) and the
;;; CHAR-ALPHABETIC-P*/CHAR-NUMERIC-P*/CHAR-UPPERCASE-P*/CHAR-LOWERCASE-P*
;;; code-point classes back the case and class helpers (issue #519); on ASCII
;;; they agree exactly with the old A-Z/a-z/0-9 range checks. DIGIT-P and
;;; WHITESPACE-P stay ASCII: parsers and trimmers rely on their exact sets. The
;;; explicit UTF-8 <-> Array<Char> boundary (STRING->UTF8, UTF8->STRING,
;;; UTF8->STRING-LOSSY) lives in the TEXT module, lib/30-text.lisp — see its
;;; header for why that surface is namespaced instead of flat.
;;;
;;; NAMING: predicates use the `-p` suffix (STARTS-WITH-P, WHITESPACE-P, ...);
;;; case-sensitive ordering follows Common Lisp's STRING</STRING>/STRING=
;;; names (which are already case-sensitive in CL, so no divergence trap) —
;;; except inequality: the reader here does not treat `/` as a symbol
;;; constituent, so CL's STRING/= cannot be written as one token; it is
;;; STRING-NE instead. The case-insensitive family uses an explicit `-CI`
;;; infix (STRING-CI=, STRING-CI<, STRING-CI-NE, ...) rather than CL's
;;; STRING-EQUAL/STRING-LESSP/STRING-NOT-EQUAL names, because STRING-LESSP
;;; already existed here with case-SENSITIVE (code-point) semantics before
;;; this file grew a case-insensitive family — reusing CL's case-insensitive
;;; names for the new functions would have papered over that pre-existing
;;; divergence instead of documenting it. None of the comparison functions
;;; take optional start/end range arguments; CL's are keyword pairs and a
;;; partial, positional-only reading would be its own accidental
;;; divergence, so ranges are left out of this API and range users should
;;; SUBSTRING first.
;;;
;;; CHARACTERS: the reader's `'c'` literal is an integer code point (see #136).
;;; Functions in this layer accept EITHER a one-character string OR an integer
;;; code point — use CHAR->CODE to coerce. So (alpha-p 'a') and (alpha-p "a")
;;; both work. Case operations (CHAR-UPCASE, CHAR-DOWNCASE) always return a
;;; one-character string regardless of which form was given, so they use the
;;; simple (one-to-one) case mapping: a character whose full mapping is
;;; several characters ("ß" -> "SS") is returned unchanged, as in CL. The
;;; string operations (STRING-UPCASE, STRING-DOWNCASE) use the full mapping.

;;; ---- string <-> list of chars --------------------------------------------

(defun string->list (s)
  "Return the characters of S as a list of one-character STRINGS -- not char
values: (string->list \"ab\") is (\"a\" \"b\"), never ('a' 'b'). Use
CHAR->CODE on an element when a code point is wanted."
  (string->list* s))

(defun list->string (chars)
  "Concatenate a list of strings into one string."
  (apply #'concat chars))

;;; ---- construction and access ----------------------------------------------

(defun string-concat (&rest strs)
  "Concatenate zero or more strings. Alias for CONCAT, named for the STRING-
family; (string-concat) is \"\"."
  (apply #'concat strs))

(defun string-length (s)
  "Number of characters (not bytes) in string S. An alias of the kernel
primitive STRING-LENGTH*; the generic LENGTH also accepts strings."
  (string-length* s))

(defun string-empty-p (s)
  "True if S has length zero."
  (= (string-length* s) 0))

(defun make-string (n &optional char)
  "Return a fresh string of length N, every character CHAR (a one-character
string or code point; default space). Errors if N is negative."
  (if (< n 0)
      (error (concat "MAKE-STRING: length must be non-negative, got "
                      (princ-to-string n)))
      (string-repeat (code-char (char->code (if char char 32))) n)))

(defun char-at (s i)
  "One-character access: the character at index I in S, as a one-character
string. Signals a clear bounds error naming I and S's length when I is out
of range, rather than clamping."
  (let ((n (string-length* s)))
    (if (and (>= i 0) (< i n))
        (substring s i (+ i 1))
        (error (concat "CHAR-AT: index " (princ-to-string i)
                        " out of bounds for string of length "
                        (princ-to-string n))))))

;;; ---- char coercion -------------------------------------------------------

(defun char->code (c)
  "Return the integer code point of C: accepts a char, a one-character string, or an integer."
  (if (fixp c) c (char-code c)))

;;; ---- char classification (accept string or code-point integer) -----------

(defun digit-p (c)
  "True if C (one-character string or code point) is an ASCII digit 0-9."
  (let ((code (char->code c))) (and (>= code 48) (<= code 57))))

(defun alpha-p (c)
  "True if C (one-character string or code point) is a Unicode alphabetic
character (A-Z and a-z on ASCII; also e.g. \"é\", \"λ\", \"漢\")."
  (char-alphabetic-p* (char->code c)))

(defun alphanumeric-p (c)
  "True if C (one-character string or code point) is a Unicode alphabetic or
numeric character (A-Z, a-z and 0-9 on ASCII)."
  (let ((code (char->code c)))
    (or (char-alphabetic-p* code) (char-numeric-p* code))))

(defun char-upper-p (c)
  "True if C (one-character string or code point) is a Unicode uppercase
letter (A-Z on ASCII; also e.g. \"É\", \"Σ\")."
  (char-uppercase-p* (char->code c)))

(defun char-lower-p (c)
  "True if C (one-character string or code point) is a Unicode lowercase
letter (a-z on ASCII; also e.g. \"é\", \"ß\", \"σ\")."
  (char-lowercase-p* (char->code c)))

(defun whitespace-p (c)
  "True if C (one-character string or code point) is space, tab, newline, or carriage return."
  (let ((code (char->code c)))
    (or (= code 32) (= code 9) (= code 10) (= code 13))))

;;; ---- char case mapping ---------------------------------------------------

(defun $char-case-simple (ch mapped)
  "One-to-one case mapping: MAPPED if it is a single character, else CH
unchanged (a full mapping such as \"ß\" -> \"SS\" has no one-character result)."
  (if (= (string-length* mapped) 1) mapped ch))

(defun char-upcase (c)
  "Uppercase C (one-character string or code point), Unicode-aware. Returns a
one-character string; a character with no one-character uppercase (\"ß\") is
returned unchanged."
  (let ((ch (code-char (char->code c))))
    ($char-case-simple ch (string-upcase* ch))))

(defun char-downcase (c)
  "Lowercase C (one-character string or code point), Unicode-aware. Returns a
one-character string."
  (let ((ch (code-char (char->code c))))
    ($char-case-simple ch (string-downcase* ch))))

;;; ---- case mapping --------------------------------------------------------

(defun string-upcase (s)
  "Return S uppercased by the Unicode full case mapping; the result may be
longer than S (\"straße\" -> \"STRASSE\")."
  (string-upcase* s))

(defun string-downcase (s)
  "Return S lowercased by the Unicode full case mapping (a word-final capital
sigma becomes \"ς\")."
  (string-downcase* s))

(defun $string-capitalize-aux (chars in-word acc)
  (if (null chars)
      (reverse-aux acc nil)
      (let* ((c (car chars))
             (alnum (alphanumeric-p c))
             (out (cond ((not alnum) c)
                        (in-word (char-downcase c))
                        (t (char-upcase c)))))
        ($string-capitalize-aux (cdr chars) alnum (cons out acc)))))

(defun $string-capitalize-walk (chars in-word)
  "CL word-capitalization walk: uppercase the first alphanumeric of each
word (a maximal alphanumeric run), lowercase the rest, pass delimiters."
  ($string-capitalize-aux chars in-word nil))

(defun string-capitalize (s)
  "Return S with the first character of every word uppercased and the rest
of each word lowercased (Unicode-aware, one-to-one per CHAR-UPCASE), per CL:
a word is a maximal run of alphanumeric characters. (string-capitalize \"\")
is \"\"."
  (list->string ($string-capitalize-walk (string->list s) ())))

(defun string-reverse (s)
  "Reverse S. A named entry point onto the generic REVERSE (which already
works on strings) for discoverability alongside the rest of the STRING-
family."
  (reverse s))

;;; ---- number parsing ------------------------------------------------------

(defun $parse-integer-whole (s)
  "The strict reading: S as a whole denotes an integer, else NIL."
  (let ((n (string->number s)))
    (if (and (numberp n) (not (floatp n)))
        n
        nil)))

(defun $parse-integer-scan (s i n pred)
  "Index of the first character of S at or after I (S has length N) that
does not satisfy PRED, or N."
  (if (and (< i n) (funcall pred (substring s i (+ i 1))))
      ($parse-integer-scan s (+ i 1) n pred)
      i))

(defun $parse-integer-prefix (s)
  "The :JUNK-ALLOWED reading: the integer spelled by S's leading
[whitespace][sign]digits, ignoring whatever follows; NIL if no digit."
  (let* ((n (string-length* s))
         (start ($parse-integer-scan s 0 n #'whitespace-p))
         (sign (and (< start n) (substring s start (+ start 1))))
         (digits (if (or (equal sign "+") (equal sign "-")) (+ start 1) start))
         (end ($parse-integer-scan s digits n #'digit-p)))
    ;; A "+" is dropped rather than handed to STRING->NUMBER, which not
    ;; every host accepts with a sign.
    (if (> end digits)
        ($parse-integer-whole
         (substring s (if (equal sign "+") digits start) end))
        nil)))

(defun parse-integer (s &key junk-allowed)
  "Parse string S as an integer, returning the integer, or NIL if S does not
denote an integer. Surrounding whitespace is ignored (via STRING->NUMBER); a
value with a fractional part (e.g. \"3.14\") is rejected and yields NIL.
With :JUNK-ALLOWED true, parse the leading [sign]digits and ignore the rest:
(parse-integer \"12x\" :junk-allowed t) is 12. Unlike CL, a miss is NIL
rather than an error, and there is no second (index) value."
  (if junk-allowed
      ($parse-integer-prefix s)
      ($parse-integer-whole s)))

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
  (string-lessp-aux a b 0 (string-length* a) (string-length* b)))

(defun string-ne (a b)
  "True if strings A and B do NOT have the same contents. (Named STRING-NE,
not CL's STRING/=: this reader does not treat `/` as a symbol constituent,
so `string/=` cannot be written as one token.)"
  (not (string= a b)))

(defun string< (a b)
  "True if A is lexicographically (by code point) before B. Same ordering
as STRING-LESSP, under CL's case-sensitive name."
  (string-lessp a b))

(defun string> (a b)
  "True if A is lexicographically (by code point) after B."
  (string< b a))

(defun string<= (a b)
  "True if A does not come lexicographically after B (non-strict ordering)."
  (not (string> a b)))

(defun string>= (a b)
  "True if A does not come lexicographically before B (non-strict ordering)."
  (not (string< a b)))

;;; ---- case-insensitive comparison ------------------------------------------
;;;
;;; Case-insensitive by way of STRING-CASEFOLD* (Unicode default case fold,
;;; locale-independent — see the file header for why these use a `-CI`
;;; infix rather than CL's STRING-EQUAL/STRING-LESSP names).

(defun string-ci= (a b)
  "True if A and B have the same contents under Unicode case folding."
  (string= (string-casefold* a) (string-casefold* b)))

(defun string-ci-ne (a b)
  "True if A and B do NOT have the same contents under Unicode case
folding. (Named STRING-CI-NE for the same `/` reader reason as STRING-NE.)"
  (not (string-ci= a b)))

(defun string-ci< (a b)
  "True if A is lexicographically before B under Unicode case folding."
  (string< (string-casefold* a) (string-casefold* b)))

(defun string-ci> (a b)
  "True if A is lexicographically after B under Unicode case folding."
  (string-ci< b a))

(defun string-ci<= (a b)
  "Non-strict case-insensitive ordering: not (string-ci> a b)."
  (not (string-ci> a b)))

(defun string-ci>= (a b)
  "Non-strict case-insensitive ordering: not (string-ci< a b)."
  (not (string-ci< a b)))

;;; ---- search --------------------------------------------------------------

(defun string-index-of-aux (s sub i n m)
  (cond ((> (+ i m) n) nil)
        ((equal (substring s i (+ i m)) sub) i)
        (t (string-index-of-aux s sub (+ i 1) n m))))

(defun string-index-of (s sub)
  "Return the index of the first occurrence of SUB in S, or NIL."
  (string-index-of-aux s sub 0 (string-length* s) (string-length* sub)))

(defun string-last-index-of-aux (s sub i n m)
  (cond ((< i 0) nil)
        ((equal (substring s i (+ i m)) sub) i)
        (t (string-last-index-of-aux s sub (- i 1) n m))))

(defun string-last-index-of (s sub)
  "Return the index of the LAST (rightmost) occurrence of (non-empty) SUB
in S, or NIL if SUB does not occur (or is empty)."
  (let ((n (string-length* s)) (m (string-length* sub)))
    (if (or (= m 0) (> m n))
        nil
        (string-last-index-of-aux s sub (- n m) n m))))

(defun string-count-aux (s sub i n m)
  (cond ((> (+ i m) n) 0)
        ((equal (substring s i (+ i m)) sub)
         (+ 1 (string-count-aux s sub (+ i m) n m)))
        (t (string-count-aux s sub (+ i 1) n m))))

(defun string-count (s sub)
  "Count non-overlapping occurrences of (non-empty) SUB in S; 0 if SUB is
empty or does not occur."
  (if (= (string-length* sub) 0)
      0
      (string-count-aux s sub 0 (string-length* s) (string-length* sub))))

(defun contains-p (s sub)
  "True if SUB occurs anywhere in S."
  (not (null (string-index-of s sub))))

(defun starts-with-p (s prefix)
  "True if S begins with PREFIX."
  (let ((lp (string-length* prefix)))
    (and (<= lp (string-length* s))
         (equal (substring s 0 lp) prefix))))

(defun ends-with-p (s suffix)
  "True if S ends with SUFFIX."
  (let ((ls (string-length* suffix))
        (n (string-length* s)))
    (and (<= ls n)
         (equal (substring s (- n ls) n) suffix))))

;;; ---- transformation ------------------------------------------------------

(defun string-replace (s old new)
  "Replace every (non-empty) occurrence of OLD in S with NEW. Same as
STRING-REPLACE-ALL; kept under its original name (issue #147)."
  (let ((idx (string-index-of s old)))
    (if (or (null idx) (= (string-length* old) 0))
        s
        (concat (substring s 0 idx)
                new
                (string-replace
                 (substring s (+ idx (string-length* old)) (string-length* s))
                 old new)))))

(defun string-replace-all (s old new)
  "Replace every (non-empty) occurrence of OLD in S with NEW. Alias for
STRING-REPLACE, named to pair explicitly with STRING-REPLACE-FIRST."
  (string-replace s old new))

(defun string-replace-first (s old new)
  "Replace only the first (non-empty) occurrence of OLD in S with NEW."
  (let ((idx (string-index-of s old)))
    (if (or (null idx) (= (string-length* old) 0))
        s
        (concat (substring s 0 idx)
                new
                (substring s (+ idx (string-length* old)) (string-length* s))))))

(defun string-split (s delim)
  "Split S on (non-empty) string DELIM into a list of substrings. Empty
fields are preserved: a leading/trailing/doubled DELIM yields \"\" list
elements, e.g. (string-split \",a,,b,\" \",\") is (\"\" \"a\" \"\" \"b\" \"\").
A DELIM that never occurs (or is empty) yields (list S) unchanged."
  (string-split* s delim))

(defun string-join (lst sep)
  "Join a list of strings LST with separator SEP. (string-join nil sep) is
\"\"; a single-element list is returned unchanged (no separator)."
  (cond ((null lst) "")
        ((null (cdr lst)) (car lst))
        (t (string-join* lst sep))))

(defun $string-ltrim (s i n)
  (if (and (< i n) (whitespace-p (substring s i (+ i 1))))
      ($string-ltrim s (+ i 1) n)
      i))

(defun $string-rtrim (s end)
  (if (and (> end 0) (whitespace-p (substring s (- end 1) end)))
      ($string-rtrim s (- end 1))
      end))

(defun string-trim-left (s)
  "Remove leading whitespace from S."
  (substring s ($string-ltrim s 0 (string-length* s)) (string-length* s)))

(defun string-trim-right (s)
  "Remove trailing whitespace from S."
  (substring s 0 ($string-rtrim s (string-length* s))))

(defun string-trim (s)
  "Remove leading and trailing whitespace from S."
  (let* ((n (string-length* s))
         (start ($string-ltrim s 0 n))
         (end ($string-rtrim s n)))
    (if (< start end) (substring s start end) "")))

;;; ---- padding ---------------------------------------------------------------

(defun $string-repeat-aux (s n acc)
  (if (< n 1)
      acc
      ($string-repeat-aux s (- n 1) (concat acc s))))

(defun string-repeat (s n)
  "S concatenated with itself N times (\"\" when N <= 0)."
  ($string-repeat-aux s n ""))

(defun string-pad-left (s width &optional pad)
  "Pad S on the LEFT to WIDTH using PAD (default \" \"): right-aligns.
S is returned unchanged when it is already WIDTH or longer."
  (let ((fill (- width (string-length* s))))
    (if (< fill 1) s (concat (string-repeat (if pad pad " ") fill) s))))

(defun string-pad-right (s width &optional pad)
  "Pad S on the RIGHT to WIDTH using PAD (default \" \"): left-aligns.
S is returned unchanged when it is already WIDTH or longer."
  (let ((fill (- width (string-length* s))))
    (if (< fill 1) s (concat s (string-repeat (if pad pad " ") fill)))))
