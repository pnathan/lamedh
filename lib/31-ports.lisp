;;; PORTS module — synchronous binary ports and deterministic ownership for
;;; host resources (issue #255, epic #253).
;;;
;;; WHY A MODULE: like lib/30-text.lisp, this is a genuinely NEW facility
;;; layered on the kernel, not a completion of an existing flat name, so per
;;; the epic #253 namespace ruling it lives under a module instead of
;;; growing the flat namespace. Call it qualified (PORTS:OPEN-INPUT) or
;;; `(import ports)` to bind the unqualified names.
;;;
;;; MODEL (fixed by the epic; see issue #255): ports move BYTES, never
;;; implicit text. Reading/writing is Array<Char> (a byte at the Lisp
;;; surface is a Char OR an integer 0-255 — see
;;; src/evaluator/builtins_core.rs's GET-CHAR-ARRAY-BYTES) or single
;;; integers/Chars for one byte at a time. Text crosses the boundary only
;;; through the explicit TEXT module (lib/30-text.lisp, issue #254) — the
;;; READ-LINE!/READ-STRING!/WRITE-STRING! convenience wrappers below are
;;; thin, honest layers over TEXT:STRING->UTF8/TEXT:UTF8->STRING-LOSSY, not
;;; a new implicit-coercion path.
;;;
;;; OWNERSHIP: the documented contract is an explicit close
;;; (`(ports:close! p)`); `(ports:with-open-port (p expr) body...)` closes
;;; on every exit from BODY — normal return, an ordinary error, THROW,
;;; RETURN-FROM, or GO — via the kernel UNWIND-PROTECT special form.
;;; Double-close is a documented no-op. Rust's ordinary `Drop` (the
;;; underlying file/etc. closes when the last reference to the port value
;;; goes away) is a last-resort safety net, not the primary cleanup path —
;;; see `PortObj`'s doc comment in src/lib.rs.
;;;
;;; CAPABILITIES: opening a file port for reading needs READ-FS, for
;;; writing/appending needs CREATE-FS, and (PORTS:STDIN) needs IO — the same
;;; vocabulary and the same dynamic-extent WITH-CAPABILITIES fence
;;; attenuation as every other host-facing builtin (issue #320/#325).
;;; STDOUT/STDERR ports need no capability, matching PRINC/PRIN1 already
;;; writing to them unconditionally. Once a port is open, using it performs
;;; no further capability check (the epic's "a successfully returned handle
;;; is authority to continue" rule); closing an existing handle is likewise
;;; never blocked by capability state.
;;;
;;; The kernel primitives this wraps (PORT-OPEN-INPUT-FILE*,
;;; PORT-READ-BYTE*, ...) live in Rust because the actual file descriptors,
;;; OS I/O, and the port's representation are representation-access work
;;; the Lisp layer cannot do on its own; see
;;; src/evaluator/builtins_ports.rs and PortObj in src/lib.rs.
;;;
;;; REQUIRE-ABLE (issue #256): `(require 'ports)` on a `with_prelude()`
;;; environment loads exactly this file. It requires 'modules (for
;;; DEFMODULE/WITH-MODULE) and 'text (for the text-wrapper convenience
;;; functions) first, mirroring 30-text.lisp's own `(require 'modules)`.

(require 'modules)
(require 'text)

;; POSITION is deliberately NOT exported: the Prelude already has a flat
;; (POSITION item lst) list helper (lib/13-functional.lisp), and IMPORT
;; binds every export globally by value — exporting ours would silently
;; clobber it. Call (PORTS:POSITION p) qualified; SEEK! (no collision) is
;; exported normally.
(defmodule ports
  (:export open-input open-output open-append
           open-input-bytes open-output-bytes output-contents
           stdin stdout stderr
           read-byte! read-bytes! write-byte! write-bytes!
           flush! close! open-p input-p output-p seekable-p
           seek! port-p name kind
           read-all-bytes! read-line! read-string! write-string!
           with-open-port))

(with-module ports

  ;; ── Construction ──────────────────────────────────────────────────

  (defun open-input (path)
    "Open PATH as a binary input port. Requires READ-FS."
    (port-open-input-file* path))

  (defun open-output (path)
    "Open PATH as a binary output port, truncating any existing contents
(creating the file if it does not exist). Requires CREATE-FS."
    (port-open-output-file* path))

  (defun open-append (path)
    "Open PATH as a binary output port positioned at end-of-file, creating
it if necessary; existing contents are preserved. Requires CREATE-FS."
    (port-open-append-file* path))

  (defun open-input-bytes (bytes)
    "Open a binary input port reading from a private copy of BYTES (an
Array<Char>). No capability required: touches no host resource."
    (port-open-input-bytes* bytes))

  (defun open-output-bytes ()
    "Open a binary output port that accumulates written bytes in memory;
read them back with OUTPUT-CONTENTS. No capability required. Not
seekable."
    (port-open-output-bytes*))

  (defun output-contents (port)
    "The bytes written so far to an OPEN-OUTPUT-BYTES port, as a fresh
Array<Char>. Errors if PORT is not a memory output port."
    (port-output-contents* port))

  (defun stdin ()
    "The process's standard input as a binary input port. Requires IO."
    (port-stdin*))

  (defun stdout ()
    "The process's standard output as a binary output port. No capability
required (PRINC/PRIN1 already write to it unconditionally)."
    (port-stdout*))

  (defun stderr ()
    "The process's standard error as a binary output port. No capability
required."
    (port-stderr*))

  ;; ── Binary operations ────────────────────────────────────────────

  (defun read-byte! (port)
    "Read one byte from PORT, returned as an integer 0-255, or NIL at EOF."
    (port-read-byte* port))

  (defun read-bytes! (port n)
    "Read up to N bytes from PORT into a fresh Array<Char>. The result may
be shorter than N (including empty) at EOF or on a partial read; it is
never NIL — check its length, or use READ-BYTE! to detect EOF
unambiguously one byte at a time."
    (port-read-bytes* port n))

  (defun write-byte! (port byte)
    "Write one BYTE (a Char or integer 0-255) to PORT."
    (port-write-byte* port byte))

  (defun write-bytes! (port bytes)
    "Write BYTES (an Array<Char>) to PORT. Returns the number of bytes
actually written, which may be less than the length of BYTES on a partial
write."
    (port-write-bytes* port bytes))

  (defun flush! (port)
    "Flush any buffered writes on PORT."
    (port-flush* port))

  (defun close! (port)
    "Close PORT. Idempotent: closing an already-closed port is a silent
no-op, never an error."
    (port-close* port))

  ;; ── Introspection ────────────────────────────────────────────────

  (defun open-p (port)
    "T if PORT has not been closed."
    (port-open-p* port))

  (defun input-p (port)
    "T if PORT supports reading."
    (port-input-p* port))

  (defun output-p (port)
    "T if PORT supports writing."
    (port-output-p* port))

  (defun seekable-p (port)
    "T if PORT supports POSITION/SEEK!."
    (port-seekable-p* port))

  (defun position (port)
    "The current byte offset in a seekable PORT. Signals an error on a
non-seekable port."
    (port-position* port))

  (defun seek! (port offset)
    "Move a seekable PORT to absolute byte OFFSET from the start; returns
the new position. Signals an error on a non-seekable port."
    (port-seek* port offset))

  (defun port-p (v)
    "T if V is a port (open or closed) of any kind."
    (port-p* v))

  (defun name (port)
    "PORT's diagnostic name (e.g. a file path, or \"<stdin>\")."
    (port-name* port))

  (defun kind (port)
    "PORT's diagnostic resource kind, as a symbol: FILE, MEMORY, STDIN,
STDOUT, or STDERR (or a host-registered kind for an embedder-wrapped
port)."
    (port-kind* port))

  ;; ── Text convenience wrappers (issue #255's "text wrappers") ────────
  ;;
  ;; Thin, explicit layers over the TEXT module's UTF-8 boundary (#254) —
  ;; not a new implicit text/byte coercion. A raw LF byte (0x0A) never
  ;; appears inside a multi-byte UTF-8 sequence (continuation bytes are
  ;; 0x80-0xBF), so splitting on it at the byte level before decoding is
  ;; always UTF-8-safe.

  (defun $read-all-bytes-acc! (port acc)
    (let ((b (read-byte! port)))
      (if (null b)
          (list->array (reverse acc))
          ($read-all-bytes-acc! port (cons b acc)))))

  (defun read-all-bytes! (port)
    "Read PORT to EOF, returning every remaining byte as a fresh
Array<Char>."
    ($read-all-bytes-acc! port ()))

  (defun $read-line-acc! (port acc)
    (let ((b (read-byte! port)))
      (cond
        ((null b) (if (null acc) () (text:utf8->string-lossy (list->array (reverse acc)))))
        ((eq b 10) (text:utf8->string-lossy (list->array (reverse acc))))
        (t ($read-line-acc! port (cons b acc))))))

  (defun read-line! (port)
    "Read one line of text from PORT: bytes up to but excluding a trailing
newline (0x0A), decoded as UTF-8 (lossy). Returns NIL only at true EOF —
zero bytes read before the port ran out; a final line with no trailing
newline is still returned once."
    ($read-line-acc! port ()))

  (defun read-string! (port n)
    "Read up to N bytes from PORT and decode them as UTF-8 (lossy),
returning a STRING. May decode fewer than N bytes' worth of text at EOF or
on a partial read; may split a multi-byte character at the boundary if N
lands mid-character."
    (text:utf8->string-lossy (read-bytes! port n)))

  (defun write-string! (port s)
    "Write STRING s to PORT as its exact UTF-8 bytes. Returns the number of
bytes written."
    (write-bytes! port (text:string->utf8 s)))

  ;; ── Lifetime ─────────────────────────────────────────────────────

  (defmacro with-open-port (binding &rest body)
    "(WITH-OPEN-PORT (var port-expr) body...) -- bind VAR to the value of
PORT-EXPR (a port) for BODY's dynamic extent, unconditionally closing it
afterward: normal return, an ordinary error, THROW, RETURN-FROM, or GO
unwinding all run the close (via UNWIND-PROTECT). Double-close is a no-op,
so BODY may close VAR itself without error."
    (let ((var (car binding))
          (expr (car (cdr binding))))
      (list 'let (list (list var expr))
            (append (list 'unwind-protect (cons 'progn body))
                    (list (list 'ports:close! var)))))))

(provide 'ports
  '(ports:open-input ports:open-output ports:open-append
    ports:open-input-bytes ports:open-output-bytes ports:output-contents
    ports:stdin ports:stdout ports:stderr
    ports:read-byte! ports:read-bytes! ports:write-byte! ports:write-bytes!
    ports:flush! ports:close! ports:open-p ports:input-p ports:output-p
    ports:seekable-p ports:position ports:seek! ports:port-p ports:name
    ports:kind ports:read-all-bytes! ports:read-line! ports:read-string!
    ports:write-string! ports:with-open-port))
