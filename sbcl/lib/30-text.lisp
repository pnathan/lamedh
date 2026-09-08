;;; TEXT module — the explicit String <-> UTF-8 Array<Char> boundary
;;; (issue #254, epic #253).
;;;
;;; WHY A MODULE: lib/14-strings.lisp is the flat Prelude surface completed
;;; by #147 and #254 — it stays exactly as it is. UTF-8 encode/decode is a
;;; genuinely NEW text facility layered on top of it, not a completion of an
;;; existing flat name, so per the epic #253 namespace ruling it lives under
;;; a module (lib/27-modules.lisp: DEFMODULE / WITH-MODULE) instead of
;;; growing the flat namespace further. Call it qualified (TEXT:STRING->UTF8)
;;; or `(import text)` to bind the unqualified names.
;;;
;;; REPRESENTATION (fixed by the epic, do not change): CHAR is exactly a
;;; u8 byte, never a Unicode scalar. Unicode text lives only in STRING; a
;;; single Unicode character is a one-character STRING. ARRAY<CHAR> — an
;;; ordinary Lisp array every one of whose elements is a CHAR — is the
;;; language-level byte-vector surface; these three functions are the
;;; explicit (never implicit/coercive) crossing between it and STRING.
;;;
;;; The three kernel primitives this wraps (STRING->UTF8*, UTF8->STRING*,
;;; UTF8->STRING-LOSSY*) live in Rust because Unicode validation and byte
;;; conversion are representation-access work the Lisp layer cannot do
;;; efficiently or correctly on its own; see src/evaluator/builtins_core.rs.
;;; No capability is required: these are pure data transforms with no host
;;; side effect.
;;;
;;; REQUIRE-ABLE (issue #256): TEXT is one of the optional embedded modules
;;; -- `(require 'text)` on a `with_prelude()` environment loads exactly this
;;; file. It requires 'modules first because DEFMODULE/WITH-MODULE are
;;; themselves an optional library now (lib/27-modules.lisp), not Prelude.
;;; `with_stdlib()` still loads this file unconditionally, unchanged.

(require 'modules)

(defmodule text
  (:export string->utf8 utf8->string utf8->string-lossy))

(with-module text

  (defun string->utf8 (s)
    "Return the exact UTF-8 bytes of string S as a fresh Array<Char> (an
ARRAY whose every element is a CHAR byte 0-255). Inverse of UTF8->STRING
for any S (round-trips exactly; every Lisp STRING is valid Unicode)."
    (string->utf8* s))

  (defun utf8->string (bytes)
    "Decode BYTES (an Array<Char>) as UTF-8 and return the resulting
STRING. Signals a descriptive error naming the offending byte offset if
BYTES is not well-formed UTF-8; use UTF8->STRING-LOSSY when replacement-
character decoding is wanted instead of an error."
    (utf8->string* bytes))

  (defun utf8->string-lossy (bytes)
    "Decode BYTES (an Array<Char>) as UTF-8, substituting the Unicode
replacement character (U+FFFD) for any invalid byte sequence instead of
signalling an error."
    (utf8->string-lossy* bytes)))

(provide 'text '(text:string->utf8 text:utf8->string text:utf8->string-lossy))
