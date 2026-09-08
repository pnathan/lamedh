;;; URL module — URL parse/build, percent-encoding, and query-string
;;; parse/build (issue #257, epic #253).
;;;
;;; WHY A MODULE / WHY PURE LISP: see lib/32-base64.lisp's header. This is a
;;; new namespaced facility, not flat-namespace growth; parsing is a small
;;; explicit state machine over string search (STRING-INDEX-OF /
;;; STRING-LAST-INDEX-OF / SUBSTRING) — no regular expressions are used or
;;; needed (the ticket's own non-goal). Percent-encoding needs the TEXT
;;; module's explicit String<->UTF-8 boundary (lib/30-text.lisp) so non-ASCII
;;; characters encode/decode as their exact UTF-8 bytes.
;;;
;;; TWO ENCODERS, ONE DECODER. "Percent encode for path segments and query
;;; components; these are different contexts and must not be conflated"
;;; (issue #257) is about which characters are safe to leave UN-encoded:
;;;
;;;   ENCODE-PATH-SEGMENT  — unreserved (RFC 3986 ALPHA/DIGIT/"-"/"."/"_"/"~")
;;;                           plus sub-delims and ":"/"@" stay literal (valid
;;;                           unencoded in a path segment); "/" is ALWAYS
;;;                           encoded (it is the segment separator).
;;;   ENCODE-QUERY-COMPONENT — unreserved only; every other byte (including
;;;                           "&", "=", "+", ";", "/", "?") is percent-encoded
;;;                           since those are meaningful query-string
;;;                           punctuation.
;;;
;;; Percent-DECODING, by contrast, is context-free: "%XX" always means the
;;; same one byte regardless of which encoder produced it, so there is a
;;; single DECODE (with a :LOSSY option, mirroring TEXT:UTF8->STRING vs.
;;; TEXT:UTF8->STRING-LOSSY). DECODE-PATH-SEGMENT and DECODE-QUERY-COMPONENT
;;; are provided too, as plain aliases of DECODE, purely so every ENCODE-*
;;; has a same-named DECODE-* counterpart (naming consistency across the
;;; codec modules) even though the underlying operation is identical.
;;;
;;; URL PARSE/BUILD. PARSE returns an alist with keys SCHEME, USERINFO,
;;; HOST, PORT, PATH, QUERY, FRAGMENT (symbols; unused components are NIL
;;; except PATH, always a string). PATH/QUERY/FRAGMENT/USERINFO are returned
;;; RAW — still percent-encoded exactly as they appeared in the URL, never
;;; auto-decoded — so callers who want decoded query pairs call PARSE-QUERY
;;; on the QUERY field, and callers who want decoded path segments split
;;; PATH on "/" and DECODE each segment themselves. This mirrors most
;;; standard URL libraries (e.g. Go's net/url keeping RawQuery separate from
;;; the parsed Query()) and avoids the double-decoding ambiguity of guessing
;;; how many times to unescape.
;;;
;;; QUERY-STRING PARSE/BUILD preserve repeated keys and ordering: PARSE-QUERY
;;; returns a list of (key . value) conses in the string's original order,
;;; one per "&"-separated piece (never collapsed into a hash table, which
;;; would silently drop repeats) — BUILD-QUERY is its inverse.
;;;
;;; REQUIRE-ABLE (issue #256): `(require 'url)` on a `with_prelude()`
;;; environment loads exactly this file. It requires 'modules and 'text
;;; first, mirroring lib/31-ports.lisp.

(require 'modules)
(require 'text)

(defmodule url
  (:export encode-path-segment encode-query-component
           decode decode-path-segment decode-query-component
           parse-query build-query
           parse build
           scheme userinfo host port path query fragment))

(with-module url

;;; ---- percent-encoding -------------------------------------------------

(def $url-hex-digits "0123456789ABCDEF")

(defun $url-unreserved-p (code)
  (or (and (>= code 65) (<= code 90))
      (and (>= code 97) (<= code 122))
      (and (>= code 48) (<= code 57))
      (= code 45) (= code 46) (= code 95) (= code 126)))

(defun $url-path-segment-safe-p (code)
  (or ($url-unreserved-p code)
      (member code '(33 36 38 39 40 41 42 43 44 59 61 58 64))))

(defun $url-query-component-safe-p (code)
  ($url-unreserved-p code))

(defun $url-percent-byte (code)
  (concat "%" (char-at $url-hex-digits (/ code 16)) (char-at $url-hex-digits (mod code 16))))

(defun $url-encode-bytes-acc (codes safe-p acc)
  (if (null codes)
      (reverse acc)
      ($url-encode-bytes-acc (cdr codes) safe-p
                             (cons (if (funcall safe-p (car codes)) (code-char (car codes)) ($url-percent-byte (car codes)))
                                   acc))))

(defun $percent-encode (s safe-p)
  (apply #'concat ($url-encode-bytes-acc (mapcar #'char->code (array->list (text:string->utf8 s))) safe-p ())))

(defun encode-path-segment (s)
  "Percent-encode S for use as one path segment: unreserved characters plus
sub-delims and \":\"/\"@\" stay literal; every other byte (including \"/\")
is percent-encoded."
  ($percent-encode s #'$url-path-segment-safe-p))

(defun encode-query-component (s)
  "Percent-encode S for use as a query-string key or value: only unreserved
characters stay literal; everything else (including \"&\"/\"=\"/\"+\") is
percent-encoded."
  ($percent-encode s #'$url-query-component-safe-p))

(defun $url-hex-nibble (code pos)
  (cond
    ((and (>= code 48) (<= code 57)) (- code 48))
    ((and (>= code 97) (<= code 102)) (+ 10 (- code 97)))
    ((and (>= code 65) (<= code 70)) (+ 10 (- code 65)))
    (t (error (concat "URL:DECODE: invalid percent-escape hex digit " (prin1-to-string (code-char code))
                      " at position " (princ-to-string pos))))))

(defun $url-decode-chars-acc (codes pos acc)
  (cond
    ((null codes) (reverse acc))
    ((= (car codes) 37)
     (if (or (null (cdr codes)) (null (cddr codes)))
         (error (concat "URL:DECODE: truncated percent-escape at position " (princ-to-string pos)))
         ($url-decode-chars-acc (cdddr codes) (+ pos 3)
                                (cons (+ (* 16 ($url-hex-nibble (cadr codes) (+ pos 1)))
                                         ($url-hex-nibble (caddr codes) (+ pos 2)))
                                      acc))))
    (t ($url-decode-chars-acc (cdr codes) (+ pos 1) (cons (car codes) acc)))))

(defun decode (s &key (lossy nil))
  "Percent-decode S (produced by either ENCODE-PATH-SEGMENT or
ENCODE-QUERY-COMPONENT — decoding is context-free) back into the original
Unicode STRING. \"%XX\" escapes decode to raw bytes; literal characters
contribute their own UTF-8 bytes. Truncated or malformed escapes are
errors naming the position. The reassembled bytes are validated as UTF-8
and signal a descriptive error unless :LOSSY is T, in which case invalid
sequences become U+FFFD (mirrors TEXT:UTF8->STRING vs. -LOSSY)."
  (let* ((codes (mapcar #'char->code (array->list (text:string->utf8 s))))
         (bytes (list->array (mapcar #'make-char ($url-decode-chars-acc codes 0 ())))))
    (if lossy (text:utf8->string-lossy bytes) (text:utf8->string bytes))))

(defun decode-path-segment (s &key (lossy nil))
  "Alias for DECODE: percent-decoding is context-free, so this is identical
to DECODE-QUERY-COMPONENT; provided only so ENCODE-PATH-SEGMENT has a
same-named inverse."
  (decode s :lossy lossy))

(defun decode-query-component (s &key (lossy nil))
  "Alias for DECODE; see DECODE-PATH-SEGMENT."
  (decode s :lossy lossy))

;;; ---- query-string parse/build ------------------------------------------

(defun $url-split-first (s sep)
  "(before . after) at the first occurrence of SEP in S, or (s . nil)."
  (let ((i (string-index-of s sep)))
    (if (null i)
        (cons s nil)
        (cons (substring s 0 i) (substring s (+ i (string-length* sep)) (string-length* s))))))

(defun $url-parse-query-pair (piece)
  (let ((split ($url-split-first piece "=")))
    (cons (decode (car split)) (decode (if (cdr split) (cdr split) "")))))

(defun $url-query-pairs-rec (s)
  (if (string-empty-p s)
      ()
      (let* ((split ($url-split-first s "&"))
             (piece (car split))
             (rest (cdr split)))
        (cons ($url-parse-query-pair piece)
              (if (null rest) () ($url-query-pairs-rec rest))))))

(defun parse-query (s)
  "Parse query string S (without a leading \"?\") into a list of (key
. value) conses, decoded via DECODE, in the string's original order —
repeated keys are preserved as repeated conses, never collapsed."
  ($url-query-pairs-rec s))

(defun $url-build-query-rec (pairs)
  (if (null pairs)
      ()
      (cons (concat (encode-query-component (princ-to-string (car (car pairs))))
                    "="
                    (encode-query-component (princ-to-string (cdr (car pairs)))))
            ($url-build-query-rec (cdr pairs)))))

(defun build-query (pairs)
  "Build a query string (without a leading \"?\") from PAIRS, a list of
(key . value) conses (strings or PRINC-able values), in the given order —
the inverse of PARSE-QUERY. Each key/value is percent-encoded via
ENCODE-QUERY-COMPONENT."
  (string-join ($url-build-query-rec pairs) "&"))

;;; ---- full URL parse/build -----------------------------------------------

(defun $url-scheme-char-p (code firstp)
  (if firstp
      (or (and (>= code 65) (<= code 90)) (and (>= code 97) (<= code 122)))
      (or (and (>= code 65) (<= code 90)) (and (>= code 97) (<= code 122))
          (and (>= code 48) (<= code 57)) (= code 43) (= code 45) (= code 46))))

(defun $url-scheme-codes-p (codes)
  (cond
    ((null codes) t)
    ((not ($url-scheme-char-p (car codes) nil)) nil)
    (t ($url-scheme-codes-p (cdr codes)))))

(defun $url-valid-scheme-p (s)
  "T if S consists entirely of valid URI scheme characters (ALPHA first,
then ALPHA/DIGIT/+/-/.). Walks UTF-8 byte codes (not STRING->LIST) so a
pathological long non-scheme prefix before a stray colon cannot exceed the
evaluator's recursion limit."
  (let ((codes (mapcar #'char->code (array->list (text:string->utf8 s)))))
    (and codes
         ($url-scheme-char-p (car codes) t)
         ($url-scheme-codes-p (cdr codes)))))

(defun $url-split-scheme (s)
  (let ((i (string-index-of s ":")))
    (if (and i ($url-valid-scheme-p (substring s 0 i)))
        (cons (substring s 0 i) (substring s (+ i 1) (string-length* s)))
        (cons nil s))))

(defun $url-parse-port (s)
  (if (string-empty-p s)
      nil
      (let ((n (string->number s)))
        (if (and (numberp n) (fixp n) (>= n 0))
            n
            (error (concat "URL:PARSE: invalid port " (prin1-to-string s)))))))

(defun $url-split-host-port (hp)
  (if (starts-with-p hp "[")
      (let ((close (string-index-of hp "]")))
        (if (null close)
            (error "URL:PARSE: unterminated IPv6 literal host (missing ']')")
            (let* ((h (substring hp 0 (+ close 1)))
                   (rest (substring hp (+ close 1) (string-length* hp))))
              (if (starts-with-p rest ":")
                  (cons h ($url-parse-port (substring rest 1 (string-length* rest))))
                  (cons h nil)))))
      (let ((i (string-last-index-of hp ":")))
        (if (null i)
            (cons (if (string-empty-p hp) nil hp) nil)
            (cons (substring hp 0 i) ($url-parse-port (substring hp (+ i 1) (string-length* hp))))))))

(defun parse (s)
  "Parse URL string S into an alist with keys SCHEME, USERINFO, HOST, PORT,
PATH, QUERY, FRAGMENT. All are NIL when absent except PATH (always a
string, possibly \"\"). QUERY/FRAGMENT are the raw text after \"?\"/\"#\"
(no leading delimiter); PATH/USERINFO are likewise raw/still-encoded — see
the file header. No regular expressions are used: this is a small explicit
state machine splitting on \"#\", \"?\", the first valid \"scheme:\",
\"//\", \"@\", and the last \":\" in the host:port piece (or a bracketed
IPv6 literal)."
  (let* ((frag-split ($url-split-first s "#"))
         (rest1 (car frag-split)) (fragment (cdr frag-split))
         (query-split ($url-split-first rest1 "?"))
         (rest2 (car query-split)) (qs (cdr query-split))
         (scheme-split ($url-split-scheme rest2))
         (sch (car scheme-split)) (rest3 (cdr scheme-split)))
    (if (starts-with-p rest3 "//")
        (let* ((after-slashes (substring rest3 2 (string-length* rest3)))
               (path-idx (string-index-of after-slashes "/"))
               (authority (if path-idx (substring after-slashes 0 path-idx) after-slashes))
               (p (if path-idx (substring after-slashes path-idx (string-length* after-slashes)) ""))
               (ui-split ($url-split-first authority "@"))
               (has-ui (not (null (cdr ui-split))))
               (ui (if has-ui (car ui-split) nil))
               (hostport (if has-ui (cdr ui-split) authority))
               (hp ($url-split-host-port hostport)))
          (list (cons 'scheme sch) (cons 'userinfo ui)
                (cons 'host (car hp)) (cons 'port (cdr hp))
                (cons 'path p) (cons 'query qs) (cons 'fragment fragment)))
        (list (cons 'scheme sch) (cons 'userinfo nil) (cons 'host nil) (cons 'port nil)
              (cons 'path rest3) (cons 'query qs) (cons 'fragment fragment)))))

(defun build (u)
  "Build a URL string from an alist U shaped like PARSE's result. Fields
are taken raw (as PARSE returns them, or as you constructed them) —
percent-encode PATH segments and build QUERY via BUILD-QUERY yourself
before assembling them into U."
  (let ((sch (cdr (assoc 'scheme u)))
        (ui (cdr (assoc 'userinfo u)))
        (h (cdr (assoc 'host u)))
        (p (cdr (assoc 'port u)))
        (pa (cdr (assoc 'path u)))
        (q (cdr (assoc 'query u)))
        (f (cdr (assoc 'fragment u))))
    (concat
     (if sch (concat sch ":") "")
     (if h (concat "//" (if ui (concat ui "@") "") h (if p (concat ":" (princ-to-string p)) "")) "")
     (if pa pa "")
     (if q (concat "?" q) "")
     (if f (concat "#" f) ""))))

(defun scheme (u) (cdr (assoc 'scheme u)))
(defun userinfo (u) (cdr (assoc 'userinfo u)))
(defun host (u) (cdr (assoc 'host u)))
(defun port (u) (cdr (assoc 'port u)))
(defun path (u) (cdr (assoc 'path u)))
(defun query (u) (cdr (assoc 'query u)))
(defun fragment (u) (cdr (assoc 'fragment u)))

)

(provide 'url
  '(url:encode-path-segment url:encode-query-component
    url:decode url:decode-path-segment url:decode-query-component
    url:parse-query url:build-query
    url:parse url:build
    url:scheme url:userinfo url:host url:port url:path url:query url:fragment))
