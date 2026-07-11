;;; JSON module — JSON parse/stringify (issue #257, epic #253).
;;;
;;; WHY A MODULE: like every other file in this ticket, a genuinely new
;;; namespaced facility per the epic #253 ruling — call it qualified
;;; (JSON:PARSE) or `(import json)`.
;;;
;;; WHY PURE LISP: a recursive-descent parser over UTF-8 byte codes plus
;;; STRING->NUMBER for the one genuinely representation-sensitive piece —
;;; exact int64-vs-float classification and IEEE-754-faithful float lexing
;;; (Rust's f64::from_str) — is entirely expressible in the Lisp layer using
;;; existing kernel primitives; no new Rust kernel builtins were needed.
;;;
;;; STACK SAFETY (issue #257's explicit concern, generalized): NESTING depth
;;; is bounded explicitly by :MAX-DEPTH (every recursive parse call threads
;;; a small CFG value — MAX-DEPTH . ON-INTEGER-OVERFLOW — plus the current
;;; DEPTH), so deeply nested input is a clean JSON error, not a native stack
;;; overflow. Separately, and just as important for any input of realistic
;;; size: every loop that scales with FLAT input length — total character
;;; count, array element count, object member count — is written tail-
;;; recursively with an explicit accumulator, compiling to a real loop under
;;; this evaluator's TCO instead of growing one eval frame per character/
;;; element (a plain `(cons x (recurse ...))` pattern hits this evaluator's
;;; eval-frame recursion limit at a few thousand elements, independent of
;;; JSON nesting). For the same reason the input STRING is walked as UTF-8
;;; byte codes via TEXT:STRING->UTF8 + ARRAY->LIST + CHAR->CODE (native
;;; Rust, O(n)) rather than the Prelude's STRING->LIST (Lisp recursion, one
;;; eval frame per character). Structural JSON punctuation, digits, and
;;; escape markers are all single ASCII bytes that can never coincide with a
;;; UTF-8 continuation/lead byte (always >= 0x80), so scanning and
;;; re-emitting content at the byte level is exact and lossless; STRINGIFY
;;; uses the same byte-level approach for the same reason, in the same
;;; direction.
;;;
;;; MAPPING (documented, round-trippable, exactly the ticket's mapping):
;;;
;;;   JSON object  <-> hash table, String keys, last-key-wins on duplicates
;;;   JSON array   <-> Array (Lamedh's Array type — dictated by the ticket,
;;;                    not a free choice; Lisp lists play no part in the
;;;                    JSON mapping, so a bare Lisp NIL is never itself a
;;;                    JSON array or object)
;;;   JSON string  <-> String
;;;   true         <-> T
;;;   false        <-> NIL
;;;   null         <-> the keyword :NULL — NEVER NIL, so false/null/an empty
;;;                    array are three distinct, mutually distinguishable
;;;                    Lamedh values (:NULL is a self-evaluating keyword;
;;;                    see NULL-P)
;;;   integer literal, [-2^63, 2^63-1] <-> Number (i64), exact
;;;   integer literal, out of that range <-> :ON-INTEGER-OVERFLOW policy:
;;;       :ERROR (default) signals a structured range error naming the
;;;       literal; :FLOAT widens it to a Float instead (may lose precision)
;;;   any other finite JSON number (has "." or an exponent) <-> Float
;;;
;;; STRINGIFY's Float output always contains a "." (appending ".0" when
;;; Rust's f64 formatter would otherwise print a bare integer, e.g. 2.0) so
;;; a Float always round-trips back through PARSE as a Float, never
;;; silently becoming an integer Number. NaN/infinite floats cannot be
;;; represented in JSON and are a STRINGIFY error, not silently coerced.
;;;
;;; STRICTNESS: PARSE rejects trailing garbage after the top-level value,
;;; unescaped control characters (code points < 0x20) inside strings,
;;; leading zeros in number literals ("01"), lone/unpaired \u surrogate
;;; escapes (Lamedh Strings are valid Unicode text and cannot hold a lone
;;; surrogate code point), and any nesting deeper than :MAX-DEPTH (default
;;; 512). Every error names its line and column.
;;;
;;; REQUIRE-ABLE (issue #256): `(require 'json)` on a `with_prelude()`
;;; environment loads exactly this file. It requires 'modules and 'text
;;; first, mirroring lib/31-ports.lisp.

(require 'modules)
(require 'text)

(defmodule json
  (:export parse stringify null-p))

(with-module json

(def $json-default-max-depth 512)

;;; ---- parser state: (codes line col), CODES a list of UTF-8 byte codes --

(defun $json-mk (codes line col) (list codes line col))
(defun $json-peek (st) (let ((codes (car st))) (if (null codes) nil (car codes))))

(defun $json-advance1 (st)
  (let ((codes (car st)) (line (cadr st)) (col (caddr st)))
    (if (= (car codes) 10)
        ($json-mk (cdr codes) (+ line 1) 1)
        ($json-mk (cdr codes) line (+ col 1)))))

(defun $json-error (st msg)
  (error (concat "JSON: " msg " (line " (princ-to-string (cadr st))
                ", column " (princ-to-string (caddr st)) ")")))

(defun $json-skip-ws (st)
  (let ((c ($json-peek st)))
    (if (and c (member c '(32 9 10 13)))
        ($json-skip-ws ($json-advance1 st))
        st)))

;;; ---- small tail-recursive list-building helpers --------------------------

(defun $json-push (items acc)
  "Cons ITEMS (front to back) onto ACC in order. ITEMS is always small and
fixed-size at every call site (an escape sequence's few bytes, one code
point's UTF-8 bytes), so this recursion is trivially bounded regardless of
overall input size."
  (if (null items) acc ($json-push (cdr items) (cons (car items) acc))))

(defun $json-join-acc (pieces sep acc)
  (cond
    ((null pieces) (apply #'concat (reverse acc)))
    ((null (cdr pieces)) (apply #'concat (reverse (cons (car pieces) acc))))
    (t ($json-join-acc (cdr pieces) sep (cons sep (cons (car pieces) acc))))))

(defun $json-join (pieces sep)
  "Tail-recursive equivalent of (STRING-JOIN PIECES SEP) — the Prelude's
STRING-JOIN conses (CONCATs, really) around its recursive call and so is
not stack-safe for a large number of PIECES; this accumulator version is."
  ($json-join-acc pieces sep ()))

;;; ---- literals -------------------------------------------------------------

(defun $json-expect-literal-rec (st codes value)
  (if (null codes)
      (cons value st)
      (let ((c ($json-peek st)))
        (if (and c (= c (car codes)))
            ($json-expect-literal-rec ($json-advance1 st) (cdr codes) value)
            ($json-error st "invalid literal")))))

(defun $json-expect-literal (st codes value)
  ($json-expect-literal-rec st codes value))

;;; ---- numbers ----------------------------------------------------------

(defun $json-scan-digits-acc (st acc)
  (let ((c ($json-peek st)))
    (if (and c (>= c 48) (<= c 57))
        ($json-scan-digits-acc ($json-advance1 st) (cons c acc))
        (cons (reverse acc) st))))

(defun $json-scan-digits (st) ($json-scan-digits-acc st ()))

(defun $json-parse-frac (st)
  (let* ((st3 ($json-advance1 st))
         (frac ($json-scan-digits st3)))
    (if (null (car frac))
        ($json-error st3 "expected a digit after '.'")
        frac)))

(defun $json-parse-exp (st)
  (let* ((st5 ($json-advance1 st))
         (sign-c ($json-peek st5))
         (has-sign (and sign-c (or (= sign-c 43) (= sign-c 45))))
         (st6 (if has-sign ($json-advance1 st5) st5))
         (edigits ($json-scan-digits st6)))
    (if (null (car edigits))
        ($json-error st6 "expected a digit in exponent")
        (cons (cons 101 (cons (if has-sign sign-c 43) (car edigits))) (cdr edigits)))))

(defun $json-codes->string (codes)
  (list->string (mapcar #'code-char codes)))

(defun $json-parse-number (st cfg)
  (let* ((start-st st)
         (c0 ($json-peek st))
         (neg (and c0 (= c0 45)))
         (st1 (if neg ($json-advance1 st) st)))
    (if (not (and ($json-peek st1) (>= ($json-peek st1) 48) (<= ($json-peek st1) 57)))
        ($json-error st1 "expected a digit")
        (let* ((int-result ($json-scan-digits st1))
               (int-digits (car int-result))
               (st2 (cdr int-result)))
          (if (and (> (list-length* int-digits) 1) (= (car int-digits) 48))
              ($json-error st1 "leading zeros are not allowed in JSON numbers")
              ())
          (let* ((dot-c ($json-peek st2))
                 (has-dot (and dot-c (= dot-c 46)))
                 (dot-result (if has-dot ($json-parse-frac st2) (cons () st2)))
                 (frac-digits (car dot-result))
                 (st4 (cdr dot-result))
                 (ec ($json-peek st4))
                 (has-exp (and ec (or (= ec 101) (= ec 69))))
                 (exp-result (if has-exp ($json-parse-exp st4) (cons () st4)))
                 (exp-codes (car exp-result))
                 (st7 (cdr exp-result))
                 (token (concat (if neg "-" "")
                                ($json-codes->string int-digits)
                                (if has-dot (concat "." ($json-codes->string frac-digits)) "")
                                (if has-exp ($json-codes->string exp-codes) "")))
                 (plain-int (and (not has-dot) (not has-exp))))
            (if plain-int
                (let ((n (string->number token)))
                  (cond
                    ((fixp n) (cons n st7))
                    ((eq (cadr cfg) ':float) (cons n st7))
                    (t ($json-error start-st (concat "integer " token " is out of i64 range")))))
                (cons (string->number token) st7)))))))

;;; ---- strings ------------------------------------------------------------

(defun $json-hex-digit-value-or-nil (c)
  (cond
    ((and (>= c 48) (<= c 57)) (- c 48))
    ((and (>= c 97) (<= c 102)) (+ 10 (- c 97)))
    ((and (>= c 65) (<= c 70)) (+ 10 (- c 65)))
    (t nil)))

(defun $json-hex4-acc (st value remaining)
  (if (= remaining 0)
      (cons value st)
      (let ((c ($json-peek st)))
        (if (null c)
            ($json-error st "unterminated \\u escape")
            (let ((d ($json-hex-digit-value-or-nil c)))
              (if (null d)
                  ($json-error st "invalid hex digit in \\u escape")
                  ($json-hex4-acc ($json-advance1 st) (+ (* value 16) d) (- remaining 1))))))))

(defun $json-hex4 (st) ($json-hex4-acc st 0 4))

(defun $json-cp-utf8-codes (cp)
  (mapcar #'char->code (array->list (text:string->utf8 (code-char cp)))))

(defun $json-parse-low-surrogate (st high acc)
  (if (and (= ($json-peek st) 92)
           (= ($json-peek ($json-advance1 st)) 117))
      (let* ((st3 ($json-advance1 ($json-advance1 st)))
             (hex-result ($json-hex4 st3))
             (low (car hex-result))
             (st4 (cdr hex-result)))
        (if (and (>= low 56320) (<= low 57343))
            (let ((cp (+ 65536 (* (- high 55296) 1024) (- low 56320))))
              ($json-parse-string-acc st4 ($json-push ($json-cp-utf8-codes cp) acc)))
            ($json-error st3 "high surrogate not followed by a low surrogate in \\u escape")))
      ($json-error st "unpaired high surrogate in \\u escape")))

(defun $json-parse-unicode-escape (st acc)
  (let* ((hex-result ($json-hex4 st))
         (code (car hex-result))
         (st1 (cdr hex-result)))
    (cond
      ((and (>= code 55296) (<= code 56319)) ($json-parse-low-surrogate st1 code acc))
      ((and (>= code 56320) (<= code 57343)) ($json-error st "unpaired low surrogate in \\u escape"))
      (t ($json-parse-string-acc st1 ($json-push ($json-cp-utf8-codes code) acc))))))

(defun $json-parse-escape (st acc)
  (let* ((st1 ($json-advance1 st))
         (c ($json-peek st1)))
    (cond
      ((null c) ($json-error st1 "unterminated escape sequence"))
      ((= c 34) ($json-parse-string-acc ($json-advance1 st1) (cons 34 acc)))
      ((= c 92) ($json-parse-string-acc ($json-advance1 st1) (cons 92 acc)))
      ((= c 47) ($json-parse-string-acc ($json-advance1 st1) (cons 47 acc)))
      ((= c 98) ($json-parse-string-acc ($json-advance1 st1) (cons 8 acc)))
      ((= c 102) ($json-parse-string-acc ($json-advance1 st1) (cons 12 acc)))
      ((= c 110) ($json-parse-string-acc ($json-advance1 st1) (cons 10 acc)))
      ((= c 114) ($json-parse-string-acc ($json-advance1 st1) (cons 13 acc)))
      ((= c 116) ($json-parse-string-acc ($json-advance1 st1) (cons 9 acc)))
      ((= c 117) ($json-parse-unicode-escape ($json-advance1 st1) acc))
      (t ($json-error st1 (concat "invalid escape character " (prin1-to-string (code-char c))))))))

(defun $json-parse-string-acc (st acc)
  (let ((c ($json-peek st)))
    (cond
      ((null c) ($json-error st "unterminated string"))
      ((= c 34) (cons (text:utf8->string (list->array (mapcar #'make-char (reverse acc)))) ($json-advance1 st)))
      ((= c 92) ($json-parse-escape st acc))
      (t (if (< c 32)
             ($json-error st "control character in string must be escaped")
             ($json-parse-string-acc ($json-advance1 st) (cons c acc)))))))

(defun $json-parse-string (st)
  ($json-parse-string-acc ($json-advance1 st) ()))

;;; ---- structures -----------------------------------------------------------

(defun $json-check-depth (st depth cfg)
  (if (> depth (car cfg))
      ($json-error st (concat "nesting depth exceeds limit of " (princ-to-string (car cfg))))
      ()))

(defun $json-parse-array-items-acc (st depth cfg acc)
  (let* ((val-result ($json-parse-value st depth cfg))
         (val (car val-result))
         (st1 ($json-skip-ws (cdr val-result)))
         (c ($json-peek st1))
         (acc2 (cons val acc)))
    (cond
      ((null c) ($json-error st1 "unterminated array"))
      ((= c 44) ($json-parse-array-items-acc ($json-skip-ws ($json-advance1 st1)) depth cfg acc2))
      ((= c 93) (cons (reverse acc2) ($json-advance1 st1)))
      (t ($json-error st1 "expected ',' or ']' in array")))))

(defun $json-parse-array (st depth cfg)
  ($json-check-depth st (+ depth 1) cfg)
  (let* ((st1 ($json-advance1 st))
         (st2 ($json-skip-ws st1)))
    (if (= ($json-peek st2) 93)
        (cons (list->array ()) ($json-advance1 st2))
        (let ((items-result ($json-parse-array-items-acc st2 (+ depth 1) cfg ())))
          (cons (list->array (car items-result)) (cdr items-result))))))

(defun $json-parse-object-members (st table depth cfg)
  (let ((st1 ($json-skip-ws st)))
    (if (not (= ($json-peek st1) 34))
        ($json-error st1 "expected a string key in object")
        (let* ((key-result ($json-parse-string st1))
               (key (car key-result))
               (st2 ($json-skip-ws (cdr key-result))))
          (if (not (= ($json-peek st2) 58))
              ($json-error st2 "expected ':' after object key")
              (let* ((st3 ($json-skip-ws ($json-advance1 st2)))
                     (val-result ($json-parse-value st3 depth cfg))
                     (val (car val-result))
                     (st4 ($json-skip-ws (cdr val-result))))
                (sethash table key val)
                (let ((c ($json-peek st4)))
                  (cond
                    ((null c) ($json-error st4 "unterminated object"))
                    ((= c 44)
                     ($json-parse-object-members ($json-skip-ws ($json-advance1 st4)) table depth cfg))
                    ((= c 125) ($json-advance1 st4))
                    (t ($json-error st4 "expected ',' or '}' in object"))))))))))

(defun $json-parse-object (st depth cfg)
  ($json-check-depth st (+ depth 1) cfg)
  (let* ((st1 ($json-advance1 st))
         (st2 ($json-skip-ws st1))
         (table (make-hash-table)))
    (if (= ($json-peek st2) 125)
        (cons table ($json-advance1 st2))
        (cons table ($json-parse-object-members st2 table (+ depth 1) cfg)))))

(def $json-true-codes '(116 114 117 101))
(def $json-false-codes '(102 97 108 115 101))
(def $json-null-codes '(110 117 108 108))

(defun $json-parse-value (st depth cfg)
  (let* ((st ($json-skip-ws st))
         (c ($json-peek st)))
    (cond
      ((null c) ($json-error st "unexpected end of input"))
      ((= c 123) ($json-parse-object st depth cfg))
      ((= c 91) ($json-parse-array st depth cfg))
      ((= c 34) ($json-parse-string st))
      ((= c 116) ($json-expect-literal st $json-true-codes t))
      ((= c 102) ($json-expect-literal st $json-false-codes nil))
      ((= c 110) ($json-expect-literal st $json-null-codes ':null))
      ((or (and (>= c 48) (<= c 57)) (= c 45)) ($json-parse-number st cfg))
      (t ($json-error st (concat "unexpected character " (prin1-to-string (code-char c))))))))

(defun null-p (v)
  "T if V is the JSON null marker :NULL (see the file header — never NIL)."
  (eq v ':null))

(defun parse (s &key (max-depth $json-default-max-depth) (on-integer-overflow ':error))
  "Parse JSON text S into a Lamedh value per the file header's mapping.
:MAX-DEPTH (default 512) bounds array/object nesting; deeper input is a
clean error, not a stack overflow. :ON-INTEGER-OVERFLOW is :ERROR (default:
an integer literal outside i64 range signals an error naming the literal)
or :FLOAT (widen it to a Float instead, silently, which may lose
precision). Errors carry a line/column position; rejects trailing garbage
after the value."
  (let* ((cfg (list max-depth on-integer-overflow))
         (codes (mapcar #'char->code (array->list (text:string->utf8 s))))
         (st ($json-skip-ws ($json-mk codes 1 1)))
         (result ($json-parse-value st 0 cfg))
         (val (car result))
         (st2 ($json-skip-ws (cdr result))))
    (if (null ($json-peek st2))
        val
        ($json-error st2 "trailing garbage after JSON value"))))

;;; ---- stringify --------------------------------------------------------

(defun $json-hex-digit-code (n) (if (< n 10) (+ 48 n) (+ 87 n)))

(defun $json-pad-hex4-codes (code)
  (list ($json-hex-digit-code (mod (/ code 4096) 16))
        ($json-hex-digit-code (mod (/ code 256) 16))
        ($json-hex-digit-code (mod (/ code 16) 16))
        ($json-hex-digit-code (mod code 16))))

(defun $json-escape-char-codes (code)
  "The output UTF-8 byte code(s) for one input byte CODE when JSON-escaping
a string: either an ASCII escape sequence's bytes, or CODE unchanged (safe
for UTF-8 continuation/lead bytes, always >= 0x80, which never match any
of the ASCII cases below)."
  (cond
    ((= code 34) (list 92 34))
    ((= code 92) (list 92 92))
    ((= code 8) (list 92 98))
    ((= code 12) (list 92 102))
    ((= code 10) (list 92 110))
    ((= code 13) (list 92 114))
    ((= code 9) (list 92 116))
    ((< code 32) (append (list 92 117) ($json-pad-hex4-codes code)))
    (t (list code))))

(defun $json-escape-codes-acc (codes acc)
  (if (null codes)
      (reverse acc)
      ($json-escape-codes-acc (cdr codes) ($json-push ($json-escape-char-codes (car codes)) acc))))

(defun $json-stringify-string (s)
  (let ((content ($json-escape-codes-acc (mapcar #'char->code (array->list (text:string->utf8 s))) ())))
    (text:utf8->string (list->array (mapcar #'make-char (cons 34 (append content (list 34))))))))

(defun $json-stringify-float (v)
  (let ((s (number->string v)))
    (cond
      ((member s '("inf" "-inf" "NaN"))
       (error (concat "JSON:STRINGIFY: cannot represent non-finite float " s " as JSON")))
      ((or (string-index-of s ".") (string-index-of s "e") (string-index-of s "E")) s)
      (t (concat s ".0")))))

(defun $json-indent-str (level indent)
  (make-string (* level indent) 32))

(defun $json-stringify-array (arr level indent)
  (let ((items (array->list arr)))
    (cond
      ((null items) "[]")
      ((null level)
       (concat "[" ($json-join (mapcar (lambda (x) ($json-stringify-value x nil indent)) items) ",") "]"))
      (t (let ((inner (+ level 1)))
           (concat "[\n"
                   ($json-join
                    (mapcar (lambda (x) (concat ($json-indent-str inner indent)
                                                ($json-stringify-value x inner indent)))
                            items)
                    ",\n")
                   "\n" ($json-indent-str level indent) "]"))))))

(defun $json-stringify-object (table level indent)
  (let ((ks (keys table)))
    (cond
      ((null ks) "{}")
      ((null level)
       (concat "{"
               ($json-join
                (mapcar (lambda (k) (concat ($json-stringify-string (princ-to-string k)) ":"
                                            ($json-stringify-value (gethash table k) nil indent)))
                        ks)
                ",")
               "}"))
      (t (let ((inner (+ level 1)))
           (concat "{\n"
                   ($json-join
                    (mapcar (lambda (k) (concat ($json-indent-str inner indent)
                                                ($json-stringify-string (princ-to-string k))
                                                ": "
                                                ($json-stringify-value (gethash table k) inner indent)))
                            ks)
                    ",\n")
                   "\n" ($json-indent-str level indent) "}"))))))

(defun $json-stringify-value (v level indent)
  (cond
    ((eq v ':null) "null")
    ((eq v t) "true")
    ((null v) "false")
    ((stringp v) ($json-stringify-string v))
    ((floatp v) ($json-stringify-float v))
    ((numberp v) (number->string v))
    ((hash-table-p v) ($json-stringify-object v level indent))
    ((arrayp v) ($json-stringify-array v level indent))
    (t (error (concat "JSON:STRINGIFY: cannot serialize value " (prin1-to-string v))))))

(defun stringify (v &key (pretty nil) (indent 2))
  "Serialize Lamedh value V to a JSON text String — the exact inverse of
PARSE's mapping (see the file header). :PRETTY (default NIL) produces
multi-line, :INDENT-space-per-level (default 2) indented output; compact
output (no insignificant whitespace) otherwise. Hash-table keys that are
not already Strings are coerced via PRINC-TO-STRING. Signals an error for
a NaN/infinite Float (JSON cannot represent one) or a value outside the
mapping (e.g. a bare Char or Cons)."
  ($json-stringify-value v (if pretty 0 nil) indent))

)

(provide 'json '(json:parse json:stringify json:null-p))
