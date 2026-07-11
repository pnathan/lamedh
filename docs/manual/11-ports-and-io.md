# 11. Ports and Binary I/O

Chapter 7 covered *whether* a script may touch the filesystem or stdin at
all — the capability story. This chapter covers *how* it moves bytes once
it may: the `PORTS` module, a synchronous binary I/O abstraction over
files, in-memory byte buffers, and the process's standard streams.

`PORTS` is an optional embedded library (`lib/31-ports.lisp`), like `TEXT`
(Chapter 3) and `MODULES` (Chapter 10). Pull it in with `(require 'ports)`
on a `with_prelude()`-style environment, or `(import ports)` to bind its
exports unqualified; `with_stdlib()` environments (including the `lamedh`
CLI) already have it loaded — `import` is all you need.

```console
$ target/debug/lamedh -s "(progn (import ports) (port-p (open-output-bytes)))"
; => T
```

## 11.1 Ports move bytes, not text

A port is a byte stream. Every binary operation reads or writes
`Array<Char>` — an array whose every element is a `Char` byte 0–255, or
one integer/`Char` at a time (Chapter 3's byte-array convention) — never a
`String` implicitly. If you want text, you cross the boundary explicitly
through `TEXT` (`text:string->utf8`/`text:utf8->string`, Chapter 3) — or
through the thin convenience wrappers `read-line!`/`read-string!`/
`write-string!` described in §11.4, which are themselves just calls into
`TEXT`.

This is deliberate: a byte-array port must round-trip *any* byte sequence,
including one that is not valid UTF-8 at all (a truncated download, a
binary file format, ...). There is no implicit coercion to break that.

## 11.2 Opening a port

| Constructor | Kind | Capability |
|---|---|---|
| `(ports:open-input path)` | file, read | `READ-FS` |
| `(ports:open-output path)` | file, write, truncates | `CREATE-FS` |
| `(ports:open-append path)` | file, write, preserves contents | `CREATE-FS` |
| `(ports:open-input-bytes bytes)` | in-memory, reads a copy of `bytes` | none |
| `(ports:open-output-bytes)` | in-memory, accumulates writes | none |
| `(ports:stdin)` | the process's stdin | `IO` |
| `(ports:stdout)` | the process's stdout | none |
| `(ports:stderr)` | the process's stderr | none |

`stdout`/`stderr` need no capability because `princ`/`prin1` already write
to them unconditionally — a port onto the same stream grants nothing new.
`stdin` needs `IO`, matching `(read)`. File ports need the same
`READ-FS`/`CREATE-FS` capabilities as `read-file`/`write-file`, checked the
same way (Chapter 7 §7.1), so `with-capabilities` fences attenuate port
construction exactly like every other host-facing builtin:

```console
$ target/debug/lamedh -s "(progn (import ports) (open-input \"/tmp/greeting.bin\"))"
Error: READ-FS capability is not enabled (grant it via --capability READ-FS or the host API)
  in: OPEN-INPUT

$ target/debug/lamedh --capability READ-FS -s "(progn (import ports) (with-capabilities '() (open-input \"/tmp/greeting.bin\")))"
Error: capability denied: READ-FS (attenuated by an enclosing fence)
  in: OPEN-INPUT
```

Once a port is open, using it performs no further capability check — an
open handle is authority to keep using it, exactly like an open file
descriptor in any other language. Only *acquiring* a new one is gated.

## 11.3 Binary operations

These work uniformly across every port kind:

```console
$ target/debug/lamedh --capability READ-FS --capability CREATE-FS -s "(progn
    (import ports)
    (with-open-port (op (open-output \"/tmp/greeting.bin\"))
      (write-bytes! op (list->array (list 104 105))))
    (with-open-port (ip (open-input \"/tmp/greeting.bin\"))
      (list (read-byte! ip) (read-byte! ip) (read-byte! ip))))"
; => (104 105 ())
```

- `(read-byte! port)` — one byte (0–255), or `NIL` at EOF.
- `(read-bytes! port n)` — up to `n` bytes as a fresh `Array<Char>`.
  Returns fewer than `n` (including an empty array) at EOF or on a partial
  read — **never `NIL`**. Use `read-byte!` when you need to distinguish
  EOF unambiguously one byte at a time.
- `(write-byte! port byte)` — write one byte.
- `(write-bytes! port bytes)` — write an `Array<Char>`; returns the number
  of bytes actually written (may be less than the array's length on a
  partial write).
- `(flush! port)` — flush buffered writes.
- `(close! port)` — close the port. **Idempotent**: a second close is a
  silent no-op, never an error.
- `(open-p port)`, `(input-p port)`, `(output-p port)`, `(seekable-p
  port)` — predicates.
- `(ports:position port)` / `(seek! port offset)` — absolute byte offset,
  on seekable ports only (files and byte-array *input* ports; byte-array
  *output* ports and the standard streams are not seekable and signal an
  error). `position` is the one name `import` does **not** bind
  unqualified: the Prelude already has a flat `(position item lst)` list
  helper, and importing a shadow for it would silently break existing
  code — call `ports:position` qualified.
- `(port-p v)` — `T` if `v` is a port at all (open or closed).
- `(name port)` / `(kind port)` — diagnostic name and resource kind
  (`FILE`, `MEMORY`, `STDIN`, `STDOUT`, `STDERR`, or a host-registered kind
  for an embedder-wrapped port — §11.6).

EOF is a value, not an error — reading past the end of a port never
signals:

```console
$ target/debug/lamedh -s "(progn
    (import ports)
    (with-open-port (ip (open-input-bytes (list->array (list 1 2 3))))
      (list (read-byte! ip) (read-byte! ip) (read-byte! ip) (read-byte! ip))))"
; => (1 2 3 ())
```

Double-close is deliberately harmless, so cleanup code never has to track
whether it already ran:

```console
$ target/debug/lamedh -s "(progn
    (import ports)
    (def p (open-output-bytes))
    (close! p)
    (close! p)
    'fine)"
; => FINE
```

Seeking works the same way on a file and on a byte-array input port:

```console
$ target/debug/lamedh --capability READ-FS --capability CREATE-FS -s "(progn
    (import ports)
    (with-open-port (op (open-output \"/tmp/seekme.bin\"))
      (write-bytes! op (list->array (list 10 20 30 40 50))))
    (with-open-port (ip (open-input \"/tmp/seekme.bin\"))
      (seek! ip 3)
      (list (ports:position ip) (read-byte! ip))))"
; => (3 40)
```

## 11.4 In-memory ports

`open-output-bytes` gives you an accumulating byte sink with no filesystem
involvement at all — useful for building a binary payload, or for tests
that want to assert on exact bytes without touching disk:

```console
$ target/debug/lamedh -s "(progn
    (import ports)
    (def op (open-output-bytes))
    (write-string! op \"hi\")
    (array->list (output-contents op)))"
; => ('h' 'i')
```

`open-input-bytes` is the mirror image: it reads from a private copy of an
existing `Array<Char>`, so writes to the array afterward can't retroactively
change what the port sees. Both need no capability — they never touch a
host resource.

## 11.5 Text convenience wrappers

`read-line!`, `read-string!`, and `write-string!` are thin, explicit
layers over `TEXT`'s UTF-8 boundary (Chapter 3) — not a new implicit
text/byte coercion path. `write-string!` is exactly `(write-bytes! port
(text:string->utf8 s))`; `read-line!`/`read-string!` decode with
`text:utf8->string-lossy`. Splitting on a raw newline byte (`0x0A`) before
decoding is always UTF-8-safe: a continuation byte is always `0x80`–`0xBF`,
so `0x0A` never appears inside a multi-byte character.

```console
$ target/debug/lamedh --capability READ-FS --capability CREATE-FS -s "(progn
    (import ports)
    (with-open-port (op (open-output \"/tmp/lines.txt\"))
      (write-string! op \"héllo\nworld\"))
    (with-open-port (ip (open-input \"/tmp/lines.txt\"))
      (list (read-line! ip) (read-line! ip) (read-line! ip))))"
; => ("héllo" "world" ())
```

`read-line!` returns `NIL` only at *true* EOF (zero bytes read); a final
line with no trailing newline is still returned once, as the example above
shows for `"world"`.

`read-all-bytes!` reads a port to EOF and returns everything as one fresh
`Array<Char>` — handy in tests and for slurping a whole small file's bytes
at once.

## 11.6 `with-open-port`: deterministic cleanup

The documented ownership contract is an **explicit close**. `with-open-port`
is the ordinary way to get that without hand-writing `unwind-protect`
every time:

```lisp
(ports:with-open-port (p (ports:open-input "data.bin"))
  (ports:read-bytes! p 4096))
```

It binds `p` to the port for the body's dynamic extent and closes it
afterward no matter how the body exits — a normal return, an ordinary
`(error ...)`, `throw`, `return-from`, or `go` all run the close, because
`with-open-port` expands into a plain kernel `unwind-protect` (Chapter 6).
Since `close!` is idempotent, the body may also close the port itself
early without causing a double-close error.

Rust's ordinary `Drop` is a last-resort safety net underneath all of this:
if a port value is simply dropped — garbage, never bound, or the process
exits — its underlying file closes automatically because nothing leaks the
file handle out of the port's representation. That backstop exists for
resource safety, not as the documented cleanup path; write `close!` or
`with-open-port` explicitly.

I/O failures carry structured detail, not just an English sentence — the
`data` of the signalled `LispVal::Error` is an alist of `:operation`,
`:kind`, `:name`, and `:os-error`:

```console
$ target/debug/lamedh --capability READ-FS -s "(progn
    (import ports)
    (handler-case (open-input \"/tmp/does-not-exist.bin\")
      (error (e) (error-data e))))"
; => ((:OPERATION . "open-input") (:KIND . "file") (:NAME . "/tmp/does-not-exist.bin") (:OS-ERROR . "No such file or directory (os error 2)"))
```

## 11.7 Embedding: host-wrapped ports

A Rust host can hand Lisp code an arbitrary byte source or sink — a pipe,
a decompressor, a captured in-process buffer — as an ordinary port,
without ever exposing a raw file descriptor to Lisp:

```rust,ignore
use lamedh::LispVal;

let port = LispVal::wrap_reader("my-pipe", "pipe", Box::new(some_reader));
env.set("MY-PIPE".to_string(), port);
```

```rust,ignore
let port = LispVal::wrap_writer("captured-output", "capture", Box::new(some_writer));
env.set("CAPTURED-OUTPUT".to_string(), port);
```

See `docs/embedding.md` for the full signatures. Host-wrapped ports are
not seekable; every other binary operation (`read-byte!`, `write-bytes!`,
`close!`, ...) works on them exactly as it does on a file or memory port.

## 11.8 Summary

- Ports move `Array<Char>` bytes; text crosses the boundary only through
  `TEXT` or the `read-line!`/`read-string!`/`write-string!` wrappers.
- File ports need `READ-FS`/`CREATE-FS`; `stdin` needs `IO`; `stdout`/
  `stderr`/memory ports need nothing. Fences attenuate acquisition exactly
  like every other host builtin; an already-open handle keeps working.
- `close!` is idempotent. `with-open-port` closes on every exit path via
  `unwind-protect`; `Drop` is the backstop, not the contract.
- I/O errors are structured (`:operation`/`:kind`/`:name`/`:os-error`),
  not bare strings.
- Embedders wrap arbitrary `Read`/`Write` streams with `LispVal::wrap_reader`/
  `wrap_writer` — no raw file descriptors cross into Lisp.
