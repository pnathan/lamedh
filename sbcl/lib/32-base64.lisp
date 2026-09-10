;;; BASE64 module — Base64 encode/decode over Array<Char> bytes (issue #257,
;;; epic #253).
;;;
;;; WHY A MODULE: like TEXT (#254) and PORTS (#255), this is a genuinely new
;;; facility, not a completion of an existing flat name, so per the epic
;;; #253 namespace ruling it lives under a module. Call it qualified
;;; (BASE64:ENCODE) or `(import base64)` to bind the unqualified names.
;;;
;;; WHY PURE LISP: encode/decode is ordinary arithmetic over byte values
;;; (division/multiplication/MOD — no bit-shift primitive is needed, and none
;;; is used) plus a direct arithmetic alphabet mapping (no table search).
;;; Nothing here needs representation access or performance work Rust alone
;;; can do, so — per the epic's "prefer the Lisp layer" ruling and this
;;; ticket's explicit license to keep clean byte-table codecs in Lisp — it
;;; is 100% Lisp with no new Rust kernel builtins.
;;;
;;; STACK SAFETY: every loop that scales with input LENGTH (not the fixed
;;; 4-character alphabet-index arithmetic) is written tail-recursively with
;;; an explicit accumulator, so it compiles to a real loop under this
;;; evaluator's TCO instead of growing one eval frame per byte — the same
;;; concern the epic's JSON depth-limit guidance raises, generalized to any
;;; large flat input, not just nested structure. The input STRING (for
;;; DECODE) is walked as UTF-8 byte codes via TEXT:STRING->UTF8 +
;;; ARRAY->LIST (both O(n) native Rust, not Lisp recursion) rather than the
;;; Prelude's STRING->LIST, which recurses once per character and would hit
;;; the evaluator's eval-frame recursion limit on inputs of a few thousand
;;; characters.
;;;
;;; BYTES: per the epic #253 convention (see lib/30-text.lisp), a byte is a
;;; Char OR an integer 0-255; ENCODE accepts either (via CHAR->CODE) inside
;;; an Array, and DECODE always produces an Array of Char (via MAKE-CHAR),
;;; matching TEXT:STRING->UTF8's own convention.
;;;
;;; ALPHABETS: :STANDARD (RFC 4648 §4, "+/") and :URL (RFC 4648 §5, "-_"),
;;; selected with the :ALPHABET keyword (default :STANDARD). PADDING is
;;; controlled by the :PAD keyword (default T): T requires/produces trailing
;;; "=" characters out to a multiple of 4; NIL requires/produces none. Every
;;; combination of ALPHABET x PAD is independently selectable — the ticket's
;;; "standard and URL-safe alphabets with explicit padding policy".
;;;
;;; DECODE is strict: characters outside the selected alphabet, padding in
;;; the wrong position, the wrong number of "=" characters, or an input
;;; length inconsistent with the padding policy are all errors naming the
;;; offending character/position, never silently ignored or truncated.
;;;
;;; REQUIRE-ABLE (issue #256): `(require 'base64)` on a `with_prelude()`
;;; environment loads exactly this file. It requires 'modules and 'text
;;; first, mirroring lib/31-ports.lisp.

(require 'modules)
(require 'text)

(defmodule base64
  (:export encode decode))

(with-module base64

;;; ---- alphabet arithmetic (no table search) -------------------------------

(defun $base64-index->code (i alphabet)
  "The output ASCII byte code for 6-bit value I (0-63) under ALPHABET
(:STANDARD or :URL)."
  (cond
    ((< i 26) (+ 65 i))
    ((< i 52) (+ 97 (- i 26)))
    ((< i 62) (+ 48 (- i 52)))
    ((eq alphabet ':url) (if (= i 62) 45 95))
    (t (if (= i 62) 43 47))))

(defun $base64-code->index (code alphabet)
  "The 6-bit value (0-63) for ASCII byte CODE under ALPHABET, or NIL if
CODE is not a member of that alphabet."
  (cond
    ((and (>= code 65) (<= code 90)) (- code 65))
    ((and (>= code 97) (<= code 122)) (+ 26 (- code 97)))
    ((and (>= code 48) (<= code 57)) (+ 52 (- code 48)))
    ((eq alphabet ':url) (cond ((= code 45) 62) ((= code 95) 63) (t nil)))
    (t (cond ((= code 43) 62) ((= code 47) 63) (t nil)))))

(defun $base64-alphabet-keyword (alphabet)
  (cond
    ((eq alphabet ':standard) ':standard)
    ((eq alphabet ':url) ':url)
    (t (error (concat "BASE64: unknown alphabet " (prin1-to-string alphabet)
                      " (expected :STANDARD or :URL)")))))

;;; ---- encode ---------------------------------------------------------------

(defun $base64-group-codes (b0 b1 b2 count alphabet)
  "The 4 output byte codes for a group of COUNT (1-3) real bytes B0/B1/B2
(B1/B2 zero when absent, per the standard zero-padding-before-slicing
algorithm); 61 (\"=\") for pad positions."
  (let ((i0 (/ b0 4))
        (i1 (+ (* (mod b0 4) 16) (/ b1 16)))
        (i2 (+ (* (mod b1 16) 4) (/ b2 64)))
        (i3 (mod b2 64)))
    (cond
      ((= count 1) (list ($base64-index->code i0 alphabet) ($base64-index->code i1 alphabet) 61 61))
      ((= count 2) (list ($base64-index->code i0 alphabet) ($base64-index->code i1 alphabet)
                          ($base64-index->code i2 alphabet) 61))
      (t (list ($base64-index->code i0 alphabet) ($base64-index->code i1 alphabet)
               ($base64-index->code i2 alphabet) ($base64-index->code i3 alphabet))))))

(defun $base64-push-group (codes pad acc)
  "Cons CODES (a list of at most 4 elements, front to back) onto ACC in
order, dropping trailing pad (61, \"=\") codes when PAD is NIL. CODES is
always small and fixed-size, so this recursion is trivially bounded
regardless of overall input size."
  (cond
    ((null codes) acc)
    ((and (not pad) (= (car codes) 61)) ($base64-push-group (cdr codes) pad acc))
    (t ($base64-push-group (cdr codes) pad (cons (car codes) acc)))))

(defun $base64-encode-acc (bytes alphabet pad acc)
  (cond
    ((null bytes) (reverse acc))
    ((null (cdr bytes))
     (reverse ($base64-push-group ($base64-group-codes (car bytes) 0 0 1 alphabet) pad acc)))
    ((null (cddr bytes))
     (reverse ($base64-push-group ($base64-group-codes (car bytes) (cadr bytes) 0 2 alphabet) pad acc)))
    (t ($base64-encode-acc (cdddr bytes) alphabet pad
                            ($base64-push-group
                             ($base64-group-codes (car bytes) (cadr bytes) (caddr bytes) 3 alphabet)
                             t acc)))))

(defun encode (bytes &key (alphabet ':standard) (pad t))
  "Encode BYTES (an Array<Char>, elements Char or integer 0-255) as a Base64
ASCII String. :ALPHABET is :STANDARD (default, RFC 4648 \"+/\") or :URL
(RFC 4648 \"-_\"). :PAD (default T) controls whether trailing \"=\"
padding is emitted."
  (let ((alph ($base64-alphabet-keyword alphabet)))
    (list->string (mapcar #'code-char ($base64-encode-acc (mapcar #'char->code (array->list bytes)) alph pad ())))))

;;; ---- decode ---------------------------------------------------------------

(defun $base64-codes-all-equal-p (codes)
  (cond ((null codes) t) ((/= (car codes) 61) nil) (t ($base64-codes-all-equal-p (cdr codes)))))

(defun $base64-take-acc (lst n acc)
  (if (or (null lst) (< n 1)) (reverse acc) ($base64-take-acc (cdr lst) (- n 1) (cons (car lst) acc))))

(defun $base64-take (lst n)
  "Tail-recursive equivalent of (TAKE LST N) — the Prelude's TAKE conses
around its recursive call and so is not stack-safe for a large N; this
accumulator version is."
  ($base64-take-acc lst n ()))

(defun $base64-index-of-code (code alphabet pos)
  (let ((i ($base64-code->index code alphabet)))
    (if (null i)
        (error (concat "BASE64:DECODE: invalid character " (prin1-to-string (code-char code))
                       " at data position " (princ-to-string pos)))
        i)))

(defun $base64-indices-acc (codes alphabet pos acc)
  (if (null codes)
      (reverse acc)
      ($base64-indices-acc (cdr codes) alphabet (+ pos 1)
                            (cons ($base64-index-of-code (car codes) alphabet pos) acc))))

(defun $base64-push-bytes (bytes acc)
  "Cons BYTES (a list of at most 3 elements) onto ACC in order; always
small and fixed-size."
  (if (null bytes) acc ($base64-push-bytes (cdr bytes) (cons (car bytes) acc))))

(defun $base64-decode-acc (idxs acc)
  "IDXS is the core data as a list of 6-bit values, length mod 4 in {0,2,3}."
  (cond
    ((null idxs) (reverse acc))
    ((null (cddr idxs))
     (reverse (cons (+ (* (car idxs) 4) (/ (cadr idxs) 16)) acc)))
    ((null (cdddr idxs))
     (let ((i0 (car idxs)) (i1 (cadr idxs)) (i2 (caddr idxs)))
       (reverse ($base64-push-bytes
                 (list (+ (* i0 4) (/ i1 16)) (+ (* (mod i1 16) 16) (/ i2 4)))
                 acc))))
    (t (let ((i0 (car idxs)) (i1 (cadr idxs)) (i2 (caddr idxs)) (i3 (cadddr idxs)))
         ($base64-decode-acc
          (cddddr idxs)
          ($base64-push-bytes
           (list (+ (* i0 4) (/ i1 16)) (+ (* (mod i1 16) 16) (/ i2 4)) (+ (* (mod i2 4) 64) i3))
           acc))))))

(defun $base64-check-padded-length (corelen padcount)
  (let ((expect (cond ((= padcount 0) 0) ((= padcount 1) 3) (t 2))))
    (if (= (mod corelen 4) expect)
        ()
        (error (concat "BASE64:DECODE: input length inconsistent with "
                       (princ-to-string padcount) " padding character(s)")))))

(defun decode (s &key (alphabet ':standard) (pad t))
  "Decode S (a Base64 ASCII String, per :ALPHABET/:PAD — see ENCODE) into a
fresh Array<Char> of the exact original bytes. Strict: rejects characters
outside the alphabet, misplaced '=' padding, the wrong padding count, or an
input length inconsistent with the padding policy — all named with position."
  (let* ((alph ($base64-alphabet-keyword alphabet))
         (codes (mapcar #'char->code (array->list (text:string->utf8 s))))
         (n (list-length* codes))
         (eq-idx (position 61 codes))
         (padcount (if eq-idx (- n eq-idx) 0))
         (core (if eq-idx ($base64-take codes eq-idx) codes))
         (pad-tail (if eq-idx (drop codes eq-idx) ())))
    (if (and eq-idx (not ($base64-codes-all-equal-p pad-tail)))
        (error "BASE64:DECODE: '=' padding character in an invalid position")
        ())
    (cond
      (pad
       (if (> padcount 2)
           (error "BASE64:DECODE: too many '=' padding characters (at most 2 allowed)")
           ())
       ($base64-check-padded-length (list-length* core) padcount))
      (t
       (if (> padcount 0)
           (error "BASE64:DECODE: unexpected '=' padding character (alphabet configured unpadded via :PAD NIL)")
           (if (= (mod n 4) 1)
               (error (concat "BASE64:DECODE: invalid input length " (princ-to-string n)
                              " (unpadded Base64 cannot leave a single leftover character)"))
               ()))))
    (list->array (mapcar #'make-char
                          ($base64-decode-acc
                           ($base64-indices-acc core alph 0 ())
                           ())))))

)

(provide 'base64 '(base64:encode base64:decode))
