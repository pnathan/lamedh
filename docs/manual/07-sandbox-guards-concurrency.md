# 7. Sandboxing, Guards, and Concurrency

Lamedh runs untrusted or semi-trusted Lisp code as a first-class use case:
agent-generated programs, user scripts, plugin logic. Chapter 7 covers the
three mechanisms that make that safe:

- **Capabilities** — host-granted permission bits that gate filesystem,
  shell, and stdin access. Off by default.
- **Guard fences** — Lisp-level, dynamic-extent attenuation of capabilities
  (`with-capabilities`) and execution budget (`with-fuel`), composable and
  monotone (they can only narrow, never widen).
- **Concurrency** — share-nothing interpreter threads (`spawn`) and
  channels, so parallel work can't corrupt shared state because there is
  no shared state.

Every example below was run against the `lamedh` binary; the `; =>` comment
shows the actual output. Examples that need a host capability show the full
command line, including the `--capability` flags.

## 7.1 The Sandbox Model

By default, a fresh Lamedh interpreter can do arithmetic, build data
structures, define functions — anything that stays inside the Lisp heap.
It cannot touch the filesystem, run a shell command, or read from stdin.
Those are host capabilities, and the host (the CLI, or your embedding Rust
code) must grant them explicitly.

There are five capabilities:

| Capability  | Gates |
|-------------|-------|
| `READ-FS`   | Read-only filesystem operations: `load-file`, `read-file`, `read-file-byte`, `read-file-section`, `file-exists-p`, `directory-p`, `file-p`, `file-readable-p`, `file-writable-p`, `file-executable-p`, `file-size`, `directory-files`, `file-newer-p` |
| `CREATE-FS` | Filesystem mutation: `write-file`, `chmod`, `create-directory`, `delete-file` |
| `TEMP-FS`   | Temporary file/directory creation: `make-temp-file`, `make-temp-directory` |
| `SHELL`     | The `shell` builtin and the `lib/07-shell.lisp` helper layer |
| `IO`        | Stdin-consuming reads: `read` |

`rename-file` needs *both* `READ-FS` and `CREATE-FS` — renaming observes
whether the source path exists (via its error behavior), so it needs read
authority too, not just write authority.

Try a gated operation with nothing granted:

```console
$ target/debug/lamedh -s '(read-file "/etc/hostname")'
Error: READ-FS capability is not enabled (grant it via --capability READ-FS or the host API)
```

Grant it and the same call succeeds:

```console
$ target/debug/lamedh --capability READ-FS -s '(read-file "/etc/hostname")'
; => "elrond\n"
```

`--capability` is repeatable, so a script that both reads and writes needs
both flags:

```console
$ target/debug/lamedh --capability CREATE-FS --capability READ-FS \
    -s '(progn (write-file "/tmp/lamedh-demo.txt" "hi") (read-file "/tmp/lamedh-demo.txt"))'
; => "hi"
```

`TEMP-FS` and `SHELL` behave the same way — denied by default, granted by
flag:

```console
$ target/debug/lamedh -s '(make-temp-file "prefix")'
Error: TEMP-FS capability is not enabled (grant it via --capability TEMP-FS or the host API)

$ target/debug/lamedh --capability TEMP-FS -s '(make-temp-file "prefix")'
; => "/tmp/prefix-1551459-0"

$ target/debug/lamedh -s '(shell "echo hi")'
Error: SHELL capability is not enabled (grant it via --capability SHELL or the host API)

$ target/debug/lamedh --capability SHELL -s '(shell "echo hi")'
; => (0 "hi\n" "")
```

`(shell cmd)` returns a three-element list: `(exit-code stdout stderr)`.

`IO` gates `read`, which consumes stdin:

```console
$ target/debug/lamedh -s '(read)'
Error: IO capability is not enabled (grant it via --capability IO or the host API)
```

Two properties matter for the threat model: Lisp code cannot self-escalate
(there is no `enable-feature` builtin exposed to Lisp — only the host,
via `env.enable_feature(...)` or `--capability`, grants), and denial is a
catchable Lisp condition, not a panic, so a script can probe for authority
with `handler-case` and degrade gracefully.

Embedding Rust code grants capabilities the same way, on an
`Environment`:

```rust
let env = Environment::with_stdlib();
env.enable_feature("READ-FS");
env.enable_feature("SHELL");
```

## 7.2 The Shell Layer

`lib/07-shell.lisp` wraps the raw `shell` builtin (which just returns the
`(exit-code stdout stderr)` triple) with convenience accessors and an
error-checked runner, all still gated by `SHELL`.

`shell-exit-code`, `shell-stdout`, and `shell-stderr` pull a field out of
a raw result:

```console
$ target/debug/lamedh --capability SHELL \
    -s '(let ((r (shell "echo x"))) (list (shell-exit-code r) (shell-stdout r) (shell-ok-p r)))'
; => (0 "x\n" T)
```

`sh` is the one you reach for day to day: it runs a command and returns
just its stdout, signaling a Lisp error if the exit code is non-zero.

```console
$ target/debug/lamedh --capability SHELL -s '(sh "echo hello-from-sh")'
; => "hello-from-sh\n"

$ target/debug/lamedh --capability SHELL -s '(sh "exit 3")'
Error: shell command failed: exit 3
```

## 7.3 Guard Fences

Host-granted capabilities are coarse: once the process has `READ-FS`,
every piece of Lisp code running in that interpreter has it too. Guard
fences (`lib/22-guard.lisp`) let *Lisp code* narrow its own authority and
execution budget for a dynamic extent — useful when you're about to run
code you didn't write (an agent's generated plan, a plugin, a rule body)
and want to bound what it can do without spinning up a whole new process.

Two fences, one law: **both are monotone attenuators.** `with-fuel`
clamps its budget to whatever budget is already in force; `with-capabilities`
intersects its requested set with whatever is already effective. Nesting
order never matters, and a fence can never hand back authority an outer
fence already stripped away.

### `with-capabilities`

`(with-capabilities '(CAP...) form...)` evaluates `form...` with the
effective capability set narrowed to the intersection of the listed caps
and whatever was already effective. `(capabilities-effective)` reports the
live set from inside (or outside) a fence:

```console
$ target/debug/lamedh -s '(capabilities-effective)'
; => ()

$ target/debug/lamedh --capability READ-FS --capability SHELL -s '(capabilities-effective)'
; => (READ-FS SHELL)
```

Narrowing in action — the host grants both `READ-FS` and `CREATE-FS`, the
fence asks for only `READ-FS`:

```console
$ target/debug/lamedh --capability READ-FS --capability CREATE-FS \
    -s "(with-capabilities '(READ-FS) (capabilities-effective))"
; => (READ-FS)
```

A gated operation outside the fence's set is denied, with an error naming
the operation, what it requires, and what's actually effective:

```console
$ target/debug/lamedh --capability READ-FS --capability CREATE-FS \
    -s "(with-capabilities '(READ-FS) (write-file \"/tmp/x\" \"y\"))"
Error: capability denied: WRITE-FILE requires (CREATE-FS); effective (READ-FS)
```

The attenuation-only law holds even when a fence asks for more than the
host ever granted — you cannot request your way to authority nobody gave
you, and a narrower fence nested inside a wider one cannot be undone by a
later, wider `with-capabilities` inside it:

```console
$ target/debug/lamedh -s "(with-capabilities '(SHELL) (capabilities-effective))"
; => ()
```

### `with-fuel`

`(with-fuel n form...)` evaluates `form...` under a step budget of `n`,
charged at function entries and loop back-edges (`while`/`for`/`dolist`/
`dotimes` bodies, `prog` `go` labels). Exhaustion signals a catchable
"fuel exhausted" error instead of hanging the interpreter on a runaway
loop:

```console
$ target/debug/lamedh -s "(with-fuel 50 (defun spin (n) (if (< n 1) 'done (spin (- n 1)))) (spin 1000000))"
Error: fuel exhausted (budget 50)
```

Work that fits the budget completes normally and returns the same result
as unfenced code:

```console
$ target/debug/lamedh -s "(with-fuel 100000 (mapcar (lambda (v) (* v v)) (list 1 2 3 4)))"
; => (1 4 9 16)
```

`(fuel-remaining)` reports the live budget from inside a fence, and `nil`
outside any fence:

```console
$ target/debug/lamedh -s "(fuel-remaining)"
; => ()

$ target/debug/lamedh -s "(with-fuel 100 (fuel-remaining))"
; => 100
```

Nested budgets clamp to the enclosing remainder — asking for more than
what's left just gets you what's left, and every step still charges every
enclosing fence:

```console
$ target/debug/lamedh -s "(with-fuel 100 (with-fuel 1000 (fuel-remaining)))"
; => 100
```

Two safety properties worth knowing about `with-fuel`: a **kernel-level
backstop** closes the obvious escape hatch — the Lisp-level walker only
instruments code written *inside* the fence, so a function defined outside
and merely called from inside would otherwise loop unmetered; the kernel
arms a second, coarser step counter (roughly 256× the Lisp budget) for the
same extent that catches exactly this case, catchably. And **no-compile**:
inside a fence, `jit-optimize` becomes a no-op returning
`COMPILE-DISABLED-BY-GUARD`, `defun-typed` signals an error, and `defun*`
is silently downgraded to a plain `defun` — a compiled edition would run
at native speed and bypass the tick instrumentation entirely.

The threat model here is accidental runaway code (an agent's generated
loop, an off-by-one), not a determined adversary studying the fence
mechanism — see the doc comment at the top of `lib/22-guard.lisp` for the
documented Phase-1 leaks the kernel backstop closes.

### Composing fences

`with-fuel` and `with-capabilities` nest in either order with identical
results:

```console
$ target/debug/lamedh --capability READ-FS \
    -s "(with-fuel 1000 (with-capabilities '() (handler-case (read-file \"/etc/hostname\") (error (er) 'denied))))"
; => DENIED
```

`sandboxed` combines both in one call: `(sandboxed (:fuel n :capabilities
(cap...)) form...)`, either key optional.

```console
$ target/debug/lamedh --capability READ-FS \
    -s "(sandboxed (:fuel 1000 :capabilities (READ-FS)) (list (fuel-remaining) (capabilities-effective)))"
; => (1000 (READ-FS))

$ target/debug/lamedh -s "(handler-case (sandboxed (:fuel 40 :capabilities ()) (while t nil)) (error (er) 'caught))"
; => CAUGHT
```

### Static capability manifests

Rather than guessing what capabilities a function might need at runtime,
`capabilities-needed` walks the call graph (`lib/19-call-graph.lisp`)
transitively and reports the union of every gated builtin's requirement
reachable from a given function — before you ever call it:

```console
$ target/debug/lamedh -s "(progn (defun fetch (p) (read-file p)) (capabilities-needed 'fetch))"
; => (READ-FS)

$ target/debug/lamedh -s "(progn (defun pure (x) (* x x)) (capabilities-needed 'pure))"
; => ()
```

`capabilities-needed-form` does the same analysis on a raw, not-yet-bound
form:

```console
$ target/debug/lamedh -s "(capabilities-needed-form '(write-file \"x\" (sh \"date\")))"
; => (SHELL CREATE-FS)
```

This is a static, conservative approximation in both directions — dynamic
calls (`funcall`/`apply` of a computed function value, `eval` of data) are
invisible to the call graph, and reachability doesn't mean the gated call
executes on every path. It's meant to drive a *minimal* fence: infer the
manifest, then grant exactly that:

```console
$ target/debug/lamedh --capability READ-FS --capability CREATE-FS \
    -s "(progn (defun probe (p) (file-exists-p p))
               (with-capabilities (capabilities-needed 'probe)
                 (list (probe \"/etc/hostname\")
                       (handler-case (write-file \"/tmp/x\" \"y\") (error (er) 'denied)))))"
; => (T DENIED)
```

`probe` only needed `READ-FS` to run `file-exists-p`, so the fence built
from its manifest grants that and nothing else — a `write-file` inside the
same fence is denied even though the host process holds `CREATE-FS`.

## 7.4 Concurrency: `spawn` and Share-Nothing Threads

`spawn` runs Lisp code on a fresh, independent interpreter thread — its
own 512 MiB stack, its own environment loaded from scratch with the
standard library, nothing shared with the parent. The child's authority is
the requested capability set intersected with the parent's *effective*
set (the same monotone attenuation law as `with-capabilities`), optionally
bounded by a fuel budget wired to the kernel step backstop. The body
crosses the thread boundary as serialized source text — code is data — and
the result comes back the same way. Nothing is shared, so there is nothing
to race.

```console
$ target/debug/lamedh -s "(await (spawn () (+ 40 2)))"
; => 42
```

`await` blocks for the child's value and re-signals its error in the
parent on failure. `spawn-value` gives you the raw outcome datum instead —
`(:OK value)` or `(:ERROR message)` — if you'd rather inspect it than have
it raised:

```console
$ target/debug/lamedh -s "(spawn-value (spawn () (+ 1 1)))"
; => (:OK 2)

$ target/debug/lamedh -s "(spawn-value (spawn () (car 5)))"
; => (:ERROR "Generic(\"CAR: expected a list, got 5\")")
```

### Capability intersection

A child can never end up with more authority than its parent's *effective*
set — requesting a capability the parent doesn't hold yields an empty
grant for it, not an error:

```console
$ target/debug/lamedh -s "(spawn-value (spawn (:capabilities (SHELL)) (capabilities-effective)))"
; => (:OK ())
```

A `with-capabilities` fence around the `spawn` call is a hard ceiling on
what the child can request, even if the host process holds more. Here the
host has both `READ-FS` and `SHELL`, but the fence narrows to `READ-FS`
first, so the child's request for `SHELL` is denied too:

```console
$ target/debug/lamedh --capability READ-FS --capability SHELL \
    -s "(with-capabilities '(READ-FS) (spawn-value (spawn (:capabilities (READ-FS SHELL)) (capabilities-effective))))"
; => (:OK (READ-FS))
```

### Fuel budgets

`:fuel n` arms the same kernel step backstop `with-fuel` uses, scoped to
the child thread — a runaway child is bounded and its failure is
catchable in the parent via `await`:

```console
$ target/debug/lamedh -s "(handler-case
      (await (spawn (:fuel 40)
               (progn (defun sp (n) (if (< n 1) 'd (sp (- n 1)))) (sp 100000000))))
      (error (er) 'child-fuel-caught))"
; => CHILD-FUEL-CAUGHT
```

### `spawn*`: splicing runtime values

`spawn`'s body is a literal, unevaluated form serialized as source text —
it has no closures over the calling environment, because there is no
shared heap to close over. If you need to parameterize a spawned body with
a value computed at runtime, build the form yourself and hand it to
`spawn*`, the functional counterpart:

```console
$ target/debug/lamedh -s "(mapcar await (mapcar (lambda (n) (spawn* () (list '* n n))) (list 1 2 3 4 5)))"
; => (1 4 9 16 25)
```

Each `(spawn* () (list '* n n))` call builds a fresh, self-contained form
like `(* 3 3)` with `n`'s *value* spliced in, then hands that data to the
child — the closure over `n` never crosses the thread boundary, only the
number it captured does.

### Records cross the boundary as data too

Since nothing but serialized text crosses the thread boundary, a
`defrecord` value doesn't survive as a live object — it round-trips
through the printer's `#S(...)` syntax (`(make-point 1 2)` prints as
`"#S(POINT 1 2)"`) and the reader, so the *child* needs its own
`defrecord` definition for the same shape before it can read a
spliced-in value back into a proper record. Splice a live record's
printed form into a child body that redefines the same record and reads
it back:

```console
$ target/debug/lamedh -s "(progn
      (defrecord point (x int64) (y int64))
      (let ((p (make-point 3 4)))
        (spawn-value
          (spawn* ()
            (list 'progn
                  '(defrecord point (x int64) (y int64))
                  (list 'point-x (list 'quote p)))))))"
; => (:OK 3)
```

The parent's `p` prints as `#S(POINT 3 4)`, gets quoted into the child's
spliced form, and the child — with its own, independently-registered
`point` record type — reads it back and calls its own `point-x` accessor.
Two interpreters, two type registrations, one value shape agreed on by
convention. `spawn-error-p` tests an outcome datum the same way, without
unwrapping it: `(spawn-error-p (spawn-value (spawn () (car 5))))` is `T`.

### Share-nothing, concretely

A binding set in the parent is invisible to a child, and a binding set in
a child never leaks back to the parent:

```console
$ target/debug/lamedh -s "(progn (setq parent-only 99) (spawn-value (spawn () (boundp 'parent-only))))"
; => (:OK ())

$ target/debug/lamedh -s "(progn (await (spawn () (setq child-only 7))) (boundp 'child-only))"
; => ()
```

This is the whole point of the design — a data race requires shared
mutable state, and spawned children simply don't have any.

## 7.5 Channels

Channels are the lower-level concurrency primitive `spawn`/`await` are
themselves built on (`await` is `channel-recv` under the hood). They're
available directly for hand-rolled producer/consumer patterns, gated
behind the `concurrency` Cargo feature — on by default in this workspace's
`default-members` build, so no extra flags are needed.

`(make-channel)` creates a channel value; `channel-send` and `channel-recv`
push and block-pull a value through it. Like the `spawn` boundary, values
cross via the printer on the way in and the reader on the way out.

```console
$ target/debug/lamedh -s "(let ((chan (make-channel))) (channel-send chan 42) (channel-recv chan))"
; => 42
```

(Note: avoid `ch` as a variable name — the reader parses a bare `ch` as
the Lisp-1.5 assembly hex literal `Ch` (hex `C` = 12), not a symbol;
`chan` is safe.)

`channel-recv-timeout` bounds the wait; it returns `nil` if nothing
arrives in time, or the value if one does:

```console
$ target/debug/lamedh -s "(let ((chan (make-channel))) (channel-recv-timeout chan 50))"
; => ()

$ target/debug/lamedh -s "(let ((chan (make-channel))) (channel-send chan 'hi) (channel-recv-timeout chan 500))"
; => HI
```

`clone-interpreter` deep-clones the current interpreter's visible bindings
into a fresh `Environment` value, useful for building your own isolated
evaluation contexts (outside the `spawn` machinery) that still start from
the caller's current definitions rather than a bare stdlib load:

```console
$ target/debug/lamedh -s "(progn (setq x 5) (let ((e2 (clone-interpreter))) (eval '(+ x 1) e2)))"
; => 6
```

Closures copied into the clone still reference their original definition
environment; the clone gets its own copy of the *visible bindings*, not a
fully independent heap the way a `spawn`ed child is. For full share-nothing
isolation, reach for `spawn`/`spawn*` instead.

## 7.6 Summary

- Nothing dangerous is available until the host grants it: `READ-FS`,
  `CREATE-FS`, `TEMP-FS`, `SHELL`, `IO`. Lisp code cannot self-escalate.
- `with-capabilities` and `with-fuel` let *guarded* Lisp code narrow its
  own authority and step budget for a dynamic extent, monotonically —
  nesting order never matters, and narrowing can't be undone from inside.
- `capabilities-needed`/`capabilities-needed-form` give a static,
  conservative manifest to drive a minimal fence before running unfamiliar
  code.
- `spawn`/`spawn*`/`await`/`spawn-value` run code on isolated,
  share-nothing interpreter threads whose authority is the requested set
  intersected with the caller's effective set — real Rust threads, no
  shared mutable Lisp heap, nothing to race. `spawn*` splices runtime
  values into a spawned body, since the body itself carries no closures.
- Channels (`make-channel`, `channel-send`, `channel-recv`,
  `channel-recv-timeout`) are the primitive `spawn` is built from, and are
  available directly for hand-rolled producer/consumer patterns.
