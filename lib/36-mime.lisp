;;; MIME module — case-insensitive multi-value headers and Content-Type
;;; parse/build (issue #257, epic #253).
;;;
;;; WHY A MODULE / WHY PURE LISP: see lib/32-base64.lisp's header — same
;;; reasoning. Header-name comparison reuses STRING-CI= (Unicode default
;;; case fold, already used for HTTP-style case-insensitive comparisons
;;; elsewhere in the stdlib); Content-Type parameter parsing is a small
;;; explicit token/quoted-string scanner (RFC 2045/7231), no regular
;;; expressions.
;;;
;;; HEADER REPRESENTATION: a list of (name . value) conses, in original
;;; order, ORIGINAL CASE PRESERVED. This is deliberately NOT a hash table:
;;; a hash keyed by (case-folded) name could hold only one value per key,
;;; which would silently collapse repeated headers like Set-Cookie — the
;;; ticket's explicit requirement. HEADERS-GET-ALL is the multi-value
;;; accessor; HEADERS-GET returns only the first match for convenience.
;;; HEADER-NAME= is the case-insensitive comparison primitive the other
;;; operations are built on.
;;;
;;; CONTENT-TYPE: PARSE-CONTENT-TYPE returns an alist (TYPE . "text")
;;; (SUBTYPE . "html") (PARAMETERS . ((name . value)...)) — parameter
;;; values already unescaped from any quoted-string form. BUILD-CONTENT-TYPE
;;; is its inverse, quoting a parameter value (escaping '\' and '"') only
;;; when it cannot be written as a bare token.
;;;
;;; REQUIRE-ABLE (issue #256): `(require 'mime)` on a `with_prelude()`
;;; environment loads exactly this file.

(require 'modules)

(defmodule mime
  (:export header-name= headers-get headers-get-all headers-add
           headers-set headers-remove headers-names
           parse-content-type build-content-type content-type-parameter))

(with-module mime

;;; ---- headers ------------------------------------------------------------

(defun header-name= (a b)
  "Case-insensitive header-name equality (Unicode default case fold, same
as STRING-CI=; HTTP header names are ASCII tokens, for which this agrees
with ASCII case-insensitive comparison)."
  (string-ci= a b))

(defun headers-get (headers name)
  "The value of the FIRST header in HEADERS (a list of (name . value)
conses) whose name matches NAME case-insensitively, or NIL if none."
  (cond
    ((null headers) nil)
    ((header-name= (car (car headers)) name) (cdr (car headers)))
    (t (headers-get (cdr headers) name))))

(defun headers-get-all (headers name)
  "Every value in HEADERS whose name matches NAME case-insensitively, in
original order — the multi-value accessor (e.g. every Set-Cookie value;
never collapsed into one)."
  (cond
    ((null headers) ())
    ((header-name= (car (car headers)) name) (cons (cdr (car headers)) (headers-get-all (cdr headers) name)))
    (t (headers-get-all (cdr headers) name))))

(defun headers-add (headers name value)
  "Return a fresh headers list with (NAME . VALUE) appended after HEADERS.
Never removes or collapses an existing entry of the same name — use this
for multi-value headers like Set-Cookie."
  (append headers (list (cons name value))))

(defun headers-set (headers name value)
  "Return a fresh headers list with every existing entry matching NAME
(case-insensitive) removed and (NAME . VALUE) appended once. Use only for
headers that must be singular (e.g. Content-Type) — calling this on a
header meant to carry multiple values collapses them, which is the
opposite of HEADERS-ADD."
  (append (filter (lambda (h) (not (header-name= (car h) name))) headers)
          (list (cons name value))))

(defun headers-remove (headers name)
  "Return a fresh headers list with every entry matching NAME (case-
insensitive) removed."
  (filter (lambda (h) (not (header-name= (car h) name))) headers))

(defun $mime-member-ci (name lst)
  (cond
    ((null lst) nil)
    ((header-name= name (car lst)) t)
    (t ($mime-member-ci name (cdr lst)))))

(defun $mime-names-rec (headers seen)
  (cond
    ((null headers) (reverse seen))
    (($mime-member-ci (car (car headers)) seen) ($mime-names-rec (cdr headers) seen))
    (t ($mime-names-rec (cdr headers) (cons (car (car headers)) seen)))))

(defun headers-names (headers)
  "The distinct header names in HEADERS, each spelled the way it was FIRST
given, in first-seen order."
  ($mime-names-rec headers ()))

;;; ---- Content-Type: parameter scanner (token or quoted-string) -----------

(defun $mime-skip-ws (chars)
  (if (and chars (member (car chars) (list " " "\t"))) ($mime-skip-ws (cdr chars)) chars))

(defun $mime-token-char-p (c)
  (not (member c (list " " "\t" ";" "="))))

(defun $mime-parse-token-acc (chars acc)
  (if (and chars ($mime-token-char-p (car chars)))
      ($mime-parse-token-acc (cdr chars) (cons (car chars) acc))
      (cons (list->string (reverse acc)) chars)))

(defun $mime-parse-token (chars) ($mime-parse-token-acc chars ()))

(defun $mime-parse-quoted-acc (chars acc)
  (cond
    ((null chars) (error "MIME: unterminated quoted parameter value"))
    ((string= (car chars) "\\")
     (if (null (cdr chars))
         (error "MIME: unterminated quoted parameter value")
         ($mime-parse-quoted-acc (cddr chars) (cons (cadr chars) acc))))
    ((string= (car chars) "\"") (cons (list->string (reverse acc)) (cdr chars)))
    (t ($mime-parse-quoted-acc (cdr chars) (cons (car chars) acc)))))

(defun $mime-parse-quoted (chars) ($mime-parse-quoted-acc chars ()))

(defun $mime-parse-value (chars)
  (let ((chars ($mime-skip-ws chars)))
    (if (and chars (string= (car chars) "\""))
        ($mime-parse-quoted (cdr chars))
        ($mime-parse-token chars))))

(defun $mime-parse-params (chars)
  (let ((chars ($mime-skip-ws chars)))
    (if (null chars)
        ()
        (if (string= (car chars) ";")
            ($mime-parse-params (cdr chars))
            (let* ((name-split ($mime-parse-token chars))
                   (name (car name-split))
                   (rest ($mime-skip-ws (cdr name-split))))
              (if (and rest (string= (car rest) "="))
                  (let* ((value-split ($mime-parse-value (cdr rest)))
                         (value (car value-split))
                         (rest2 ($mime-skip-ws (cdr value-split))))
                    (cons (cons name value) ($mime-parse-params rest2)))
                  (error (concat "MIME:PARSE-CONTENT-TYPE: expected '=' after parameter name "
                                (prin1-to-string name)))))))))

(defun parse-content-type (s)
  "Parse a Content-Type header value S into an alist (TYPE . type-string)
(SUBTYPE . subtype-string) (PARAMETERS . ((name . value)...)), parameters
in the given order with quoted-string values already unescaped."
  (let* ((trimmed (string-trim s))
         (semi (string-index-of trimmed ";"))
         (media (string-trim (if semi (substring trimmed 0 semi) trimmed)))
         (paramstr (if semi (substring trimmed semi (string-length* trimmed)) ""))
         (slash (string-index-of media "/")))
    (if (null slash)
        (error (concat "MIME:PARSE-CONTENT-TYPE: missing '/' in media type " (prin1-to-string media)))
        (list (cons 'type (substring media 0 slash))
              (cons 'subtype (substring media (+ slash 1) (string-length* media)))
              (cons 'parameters ($mime-parse-params (string->list paramstr)))))))

(defun $mime-param-lookup (params name)
  (cond
    ((null params) nil)
    ((string-ci= (car (car params)) name) (cdr (car params)))
    (t ($mime-param-lookup (cdr params) name))))

(defun content-type-parameter (ct name)
  "Case-insensitive lookup of parameter NAME's value in CT (as returned by
PARSE-CONTENT-TYPE), or NIL if absent."
  ($mime-param-lookup (cdr (assoc 'parameters ct)) name))

;;; ---- Content-Type: build --------------------------------------------------

(defun $mime-token-char-ok-p (c)
  (let ((code (char->code c)))
    (and (> code 32) (< code 127)
         (not (member c (list "(" ")" "<" ">" "@" "," ";" ":" "\\" "\"" "/" "[" "]" "?" "=" " "))))))

(defun $mime-token-chars-p (chars)
  (or (null chars)
      (and ($mime-token-char-ok-p (car chars)) ($mime-token-chars-p (cdr chars)))))

(defun $mime-token-p (s)
  "T if S can be written as a bare token (RFC 2045 tspecials excluded, no
whitespace) rather than needing a quoted-string."
  (and (not (string-empty-p s)) ($mime-token-chars-p (string->list s))))

(defun $mime-escape-quoted (chars)
  (if (null chars)
      ""
      (concat (if (member (car chars) (list "\"" "\\")) (concat "\\" (car chars)) (car chars))
              ($mime-escape-quoted (cdr chars)))))

(defun $mime-quote-value (s)
  (concat "\"" ($mime-escape-quoted (string->list s)) "\""))

(defun $mime-param-str (pair)
  (concat (car pair) "=" (if ($mime-token-p (cdr pair)) (cdr pair) ($mime-quote-value (cdr pair)))))

(defun $mime-params-str (params)
  (apply #'concat (mapcar (lambda (p) (concat "; " ($mime-param-str p))) params)))

(defun build-content-type (type subtype &optional parameters)
  "Build a Content-Type header value from TYPE, SUBTYPE, and an optional
PARAMETERS list of (name . value) conses (in the given order). A
parameter value is written as a bare token when possible, else a
quoted-string with '\\\\' and '\\\"' escaped."
  (concat type "/" subtype ($mime-params-str parameters)))

)

(provide 'mime
  '(mime:header-name= mime:headers-get mime:headers-get-all mime:headers-add
    mime:headers-set mime:headers-remove mime:headers-names
    mime:parse-content-type mime:build-content-type mime:content-type-parameter))
