;;; HEX module — hexadecimal encode/decode over Array<Char> bytes (issue
;;; #257, epic #253).
;;;
;;; WHY A MODULE / WHY PURE LISP: see lib/32-base64.lisp's header — same
;;; reasoning applies verbatim: a genuinely new facility (module, not flat
;;; namespace growth) implemented as ordinary Lisp arithmetic, no new Rust
;;; kernel builtins.
;;;
;;; STACK SAFETY: see lib/32-base64.lisp's header — the ENCODE/DECODE loops
;;; are tail-recursive accumulators, and DECODE walks its input STRING as
;;; UTF-8 byte codes via TEXT:STRING->UTF8 + ARRAY->LIST (native, O(n))
;;; rather than the Prelude's STRING->LIST, which is not stack-safe for
;;; inputs of more than a few thousand characters.
;;;
;;; BYTES: a byte is a Char OR an integer 0-255 (epic #253 convention);
;;; ENCODE accepts either via CHAR->CODE, DECODE always produces Char
;;; elements via MAKE-CHAR.
;;;
;;; CASE: ENCODE's :CASE keyword is :LOWER (default) or :UPPER — "predictable
;;; case" per the ticket. DECODE is case-INSENSITIVE (accepts any mixture of
;;; upper/lower hex digits), matching every other hex codec's usual leniency
;;; on input case while ENCODE's output case stays predictable.
;;;
;;; STRICT: DECODE rejects an odd-length input and any non-hex-digit
;;; character, both named with position.
;;;
;;; REQUIRE-ABLE (issue #256): `(require 'hex)` on a `with_prelude()`
;;; environment loads exactly this file. It requires 'modules and 'text
;;; first, mirroring lib/31-ports.lisp.

(require 'modules)
(require 'text)

(defmodule hex
  (:export encode decode))

(with-module hex

(defun $hex-digit-code (n case)
  "The ASCII output byte code for hex nibble N (0-15) in :LOWER or :UPPER
CASE."
  (if (< n 10)
      (+ 48 n)
      (if (eq case ':lower) (+ 87 n) (+ 55 n))))

(defun $hex-encode-acc (bytes case acc)
  (if (null bytes)
      (reverse acc)
      ($hex-encode-acc (cdr bytes) case
                        (cons ($hex-digit-code (mod (car bytes) 16) case)
                              (cons ($hex-digit-code (/ (car bytes) 16) case) acc)))))

(defun encode (bytes &key (case ':lower))
  "Encode BYTES (an Array<Char>, elements Char or integer 0-255) as a
lowercase (default) or uppercase (:CASE :UPPER) hexadecimal ASCII String,
two digits per byte."
  (if (or (eq case ':lower) (eq case ':upper))
      (list->string (mapcar #'code-char ($hex-encode-acc (mapcar #'char->code (array->list bytes)) case ())))
      (error (concat "HEX:ENCODE: unknown :CASE " (prin1-to-string case) " (expected :LOWER or :UPPER)"))))

(defun $hex-nibble-value (code pos)
  (cond
    ((and (>= code 48) (<= code 57)) (- code 48))
    ((and (>= code 97) (<= code 102)) (+ 10 (- code 97)))
    ((and (>= code 65) (<= code 70)) (+ 10 (- code 65)))
    (t (error (concat "HEX:DECODE: invalid hex digit " (prin1-to-string (code-char code))
                      " at position " (princ-to-string pos))))))

(defun $hex-decode-acc (codes pos acc)
  (cond
    ((null codes) (reverse acc))
    ((null (cdr codes)) (error (concat "HEX:DECODE: truncated hex digit at position " (princ-to-string pos))))
    (t ($hex-decode-acc (cddr codes) (+ pos 2)
                         (cons (+ (* 16 ($hex-nibble-value (car codes) pos))
                                  ($hex-nibble-value (cadr codes) (+ pos 1)))
                               acc)))))

(defun decode (s)
  "Decode S (a hexadecimal ASCII String, case-insensitive) into a fresh
Array<Char> of the exact original bytes. Strict: an odd-length input or any
non-hex-digit character is an error naming the offending position."
  (let* ((codes (mapcar #'char->code (array->list (text:string->utf8 s))))
         (n (list-length* codes)))
    (if (/= (mod n 2) 0)
        (error (concat "HEX:DECODE: odd input length " (princ-to-string n)
                       " (hex input must have an even number of digits)"))
        (list->array (mapcar #'make-char ($hex-decode-acc codes 0 ()))))))

)

(provide 'hex '(hex:encode hex:decode))
