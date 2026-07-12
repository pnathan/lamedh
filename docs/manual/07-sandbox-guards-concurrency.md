# 7. Sandboxing, Guards, and Concurrency

Lamedh runs untrusted or semi-trusted Lisp code as a first-class use case:
agent-generated programs, user scripts, plugin logic. Chapter 7 covers the
three mechanisms that make that safe:

- **Capabilities** — host-granted permission bits that gate filesystem,
  shell, and stdin access. Off by default.
- **Guard fences** — kernel special forms giving dynamic-extent
  attenuation of capabilities (`with-capabilities`) and execution budget
  (`with-fuel`), composable and monotone (they can only narrow, never
  widen, and narrowing follows the call, not the lexical body).
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

There are eight capabilities:

| Capability  | Gates |
|-------------|-------|
| `READ-FS`   | Read-only filesystem operations: `load-file`, `read-file`, `read-file-byte`, `read-file-section`, `file-exists-p`, `directory-p`, `file-p`, `file-readable-p`, `file-writable-p`, `file-executable-p`, `file-size`, `directory-files`, `file-newer-p` |
| `CREATE-FS` | Filesystem mutation: `write-file`, `chmod`, `create-directory`, `delete-file` |
| `TEMP-FS`   | Temporary file/directory creation: `make-temp-file`, `make-temp-directory` |
| `SHELL`     | The `shell` builtin and the `lib/07-shell.lisp` helper layer |
| `IO`        | Stdin-consuming reads: `read` |
| `NET-DNS`   | Explicit hostname resolution: `net:resolve` (Chapter 13) |
| `NET-CONNECT` | Outbound TCP/UDP connections: `tcp:connect`, `udp:connect!`, `udp:send-to` (Chapter 13) |
| `NET-LISTEN` | Binding/listening for inbound traffic: `tcp:listen`, `udp:bind` (Chapter 13) |

`rename-file` needs *both* `READ-FS` and `CREATE-FS` — renaming observes
whether the source path exists (via its error behavior), so it needs read
authority too, not just write authority.

The `PORTS` module's file-port constructors (Chapter 11) are gated by the
same two capabilities — `ports:open-input` needs `READ-FS`,
`ports:open-output`/`ports:open-append` need `CREATE-FS`, `ports:stdin`
needs `IO` — checked the same way, so everything below about fences and
attenuation applies to opening a port exactly as it applies to
`read-file`/`write-file`. The three `NET-*` capabilities work the same
way too — see Chapter 13 for the full networking story, including a
Rust-only host policy hook that scopes a granted networking capability to
specific hosts/ports.

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
fences — `with-fuel` and `with-capabilities`, backed by `lib/22-guard.lisp`
— let *Lisp code* narrow its own authority and execution budget for a
dynamic extent, useful when you're about to run code you didn't write (an
agent's generated plan, a plugin, a rule body) and want to bound what it
can do without spinning up a whole new process.

As of 0.3 (#320) `with-fuel` and `with-capabilities` are **kernel special
forms**, not Lisp-layer machinery: each arms a thread-local counter or mask
in Rust, with RAII save/restore around the fence's own evaluation. Every
capability check and every evaluator step consults that state directly, so
attenuation follows the **call**, not the fence's lexical body. Concretely:
a helper function *defined outside* a fence and merely *called* from inside
it is still fenced; code reached through `eval` is still fenced. There is
no lexical loophole — only the dynamic extent of the call matters.

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

A gated operation outside the fence's set is denied, with a kernel-level
error naming the capability that was attenuated away:

```console
$ target/debug/lamedh --capability READ-FS --capability CREATE-FS \
    -s "(with-capabilities '(READ-FS) (write-file \"/tmp/x\" \"y\"))"
Error: capability denied: CREATE-FS (attenuated by an enclosing fence)
```

That's a different error than the "capability is not enabled" message from
§7.1 — this one fires specifically when the *host* granted the capability
but a fence narrowed it away for the current dynamic extent; the host-level
message fires when the capability was never granted at all.

The attenuation-only law holds even when a fence asks for more than the
host ever granted — you cannot request your way to authority nobody gave
you, and a narrower fence nested inside a wider one cannot be undone by a
later, wider `with-capabilities` inside it:

```console
$ target/debug/lamedh -s "(with-capabilities '(SHELL) (capabilities-effective))"
; => ()
```

### Dynamic extent, not lexical scope

Because the mask is thread-local kernel state rather than a rewrite of the
fence's body, a helper function defined *outside* a fence and called from
*inside* it is still fenced — the mask doesn't care where a function was
defined, only where it's executing:

```console
$ target/debug/lamedh --capability READ-FS \
    -s "(progn (defun outer-read (p) (read-file p))
               (with-capabilities '() (handler-case (outer-read \"/etc/hostname\") (error (er) 'denied))))"
; => DENIED
```

`outer-read` never mentions the fence — it's an ordinary function that
happens to get called while one is armed, and that's enough. `(read-file
...)` is denied the same as if it had been written directly inside the
`with-capabilities` body.

`capabilities-effective`, `require-capability`, and `feature-enabled-p` all
consult the same live mask, so any of them gives a consistent answer from
inside or outside a fence:

```console
$ target/debug/lamedh --capability READ-FS -s '(feature-enabled-p "READ-FS")'
; => T

$ target/debug/lamedh --capability READ-FS -s "(with-capabilities '() (feature-enabled-p \"READ-FS\"))"
; => ()
```

Custom capabilities — names a module registers via `(:provides CAP)`,
covered in Chapter 10 — join this same vocabulary and attenuate through
`with-capabilities`/`sandboxed`/`spawn` exactly like a built-in; they are
gated only by explicit `require-capability` checks in Lisp code and never
grant kernel abilities like `READ-FS`.

### Escaped closures run with the caller's authority

The dynamic-extent law cuts both ways. A closure *created* under a fence
but *called* after the fence has already exited runs with whatever
authority is ambient at the call site — not the authority in force when it
was made. Defining a lambda doesn't evaluate its body, so a lambda built
inside `(with-capabilities '() ...)` and returned carries no memory of that
fence once `with-capabilities` has returned:

```console
$ target/debug/lamedh --capability READ-FS \
    -s "(let ((f (with-capabilities '() (lambda () (read-file \"/etc/hostname\")))))
          (funcall f))"
; => "elrond\n"
```

The closure's body — a `read-file` call — would have been denied had it
run *inside* the fence; called afterward, outside any fence, it sees the
host's full `READ-FS` grant. Ambient authority belongs to the execution,
not the definition site — the same law `spawn` applies at its thread
boundary (§7.4): a spawned child's authority is whatever the *call to
`spawn`* is attenuated to, not whatever the closure that built its body
form once had.

### `with-fuel`

`(with-fuel n form...)` evaluates `form...` under a budget of `n` **kernel
steps** — the same step-count unit `with-fuel` was already introduced in
Chapter 6, now charged directly by the kernel trampoline rather than by a
Lisp-level walker: once per `eval`/`exec` entry and once per tail-call step,
in both the tree-walking and compiled-code paths. That is much finer
grained than counting only function entries and loop back-edges, so a
budget that "feels small" burns through faster than intuition from other
step-counted systems suggests — realistic budgets read in the hundreds to
thousands, not tens, for anything beyond "fail almost immediately."
Exhaustion signals a catchable "fuel exhausted" error, with a backtrace
frame naming where it ran out, instead of hanging the interpreter on a
runaway loop:

```console
$ target/debug/lamedh -s "(with-fuel 200 (defun spin (n) (if (< n 1) 'done (spin (- n 1)))) (spin 1000000))"
Error: fuel exhausted (kernel step budget)
  in: SPIN
```

Work that fits the budget completes normally and returns the same result
as unfenced code:

```console
$ target/debug/lamedh -s "(with-fuel 100000 (mapcar (lambda (v) (* v v)) (list 1 2 3 4)))"
; => (1 4 9 16)
```

`(fuel-remaining)` reports the live budget from inside a fence, and `nil`
outside any fence. Note it isn't exactly `n` right after entry: arming the
fence and evaluating the call to `fuel-remaining` itself are kernel steps,
charged against the very budget being armed:

```console
$ target/debug/lamedh -s "(fuel-remaining)"
; => ()

$ target/debug/lamedh -s "(with-fuel 1000 (fuel-remaining))"
; => 996
```

Nested budgets clamp to, and **spend from**, the enclosing remainder —
asking for more than what's left just gets you what's left, every step
still charges every enclosing fence, and the inner fence's own setup is
itself a charge against the outer budget:

```console
$ target/debug/lamedh -s "(with-fuel 1000 (with-fuel 5000 (fuel-remaining)))"
; => 994
```

(1000 remaining drops to 996 on entry as above; the inner `with-fuel`
clamps its requested 5000 down to that 996, then spends a few more steps
on its own setup and the `fuel-remaining` call, landing at 994.)

**No widening from Lisp.** `kernel-fuel-set!` lets code arm or disarm the
budget directly, but while a fence is already active it is narrow-only —
lowering the remaining count is allowed, raising it or disarming it is
refused. There is no Lisp-callable capability-mask setter at all (§7.3's
mask only ever moves through `with-capabilities` itself):

```console
$ target/debug/lamedh -s "(with-fuel 100 (kernel-fuel-set! 10))"
; => 97

$ target/debug/lamedh -s "(with-fuel 100 (kernel-fuel-set! 500))"
Error: kernel-fuel-set!: cannot widen or disarm inside a fuel fence

$ target/debug/lamedh -s "(with-fuel 100 (kernel-fuel-set! nil))"
Error: kernel-fuel-set!: cannot widen or disarm inside a fuel fence
```

(`kernel-fuel-set!` returns the *previous* remaining count, which is why
narrowing to 10 above reports 97 — the count already spent entering the
fence and evaluating the call.)

**No-compile.** While a fuel budget is armed, native code generation is
off, so nothing running under the fence can outrun the step counter by
dropping into unmetered compiled code:

- `jit-optimize` becomes a no-op returning the symbol
  `COMPILE-DISABLED-BY-GUARD` instead of compiling.
- `defun-typed` signals an error rather than compiling a typed native.
- `defun*` silently downgrades to a plain (interpreted) `defun` — it still
  defines the function, just without attempting native compilation.
- An ordinary `defun` that would normally auto-compile to a native
  "one-door" edition (§4.5, §4.10) takes its **interpreted fallback**
  instead of its compiled fast path for the fence's whole dynamic extent,
  so its own internal loops run back through the metered trampoline rather
  than at native speed.

```console
$ target/debug/lamedh -s "(with-fuel 1000 (progn (defun sq (x) (* x x)) (jit-optimize 'sq)))"
; => COMPILE-DISABLED-BY-GUARD

$ target/debug/lamedh -s "(with-fuel 1000 (defun-typed sq2 ((x int64)) int64 (* x x)))"
Error: defun-typed is disabled under an active fuel fence (no-compile, issue #284)

$ target/debug/lamedh -s "(with-fuel 1000 (defun* sq3 (x) (* x x)) (sq3 5))"
; => 25
```

The one-door fallback is what makes fuel accounting airtight for ordinary
code: define and warm up a plain `defun` outside any fence (letting it
auto-compile), then call it from inside a small fence — it still runs
metered and still exhausts on a runaway loop, exactly as if it had never
compiled at all:

```console
$ target/debug/lamedh -s "(progn (defun nspin (n acc) (if (< n 1) acc (nspin (- n 1) (+ acc 1))))
      (nspin 10 0)
      (with-fuel 50 (handler-case (nspin 1000000 0) (error (er) 'exhausted))))"
; => EXHAUSTED

$ target/debug/lamedh -s "(progn (defun nspin (n acc) (if (< n 1) acc (nspin (- n 1) (+ acc 1))))
      (nspin 10 0)
      (nspin 1000000 0))"
; => 1000000
```

Unfenced, `nspin`'s million-deep loop finishes instantly on its compiled
fast path; fenced, the same loop is metered and a 50-step budget cannot
possibly finish it.

The one documented exception: a `defrecord` accessor or constructor
compiled through the older `defstruct-typed` native path (pre-existing,
not a one-door `defun`) has no interpreted fallback to fall back to, and
keeps running unmetered even under an armed fence:

```console
$ target/debug/lamedh -s "(progn (defrecord coin (value int64)) (with-fuel 5 (coin-value (make-coin 7))))"
; => 7
```

That succeeds under a 5-step budget specifically because `coin-value`
never re-enters the metered trampoline — a known, documented hole (see the
header comment in `lib/22-guard.lisp`), not something to rely on.

The threat model here is accidental runaway code (an agent's generated
loop, an off-by-one), not a determined adversary studying the fence
mechanism.

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
; => (994 (READ-FS))

$ target/debug/lamedh -s "(handler-case (sandboxed (:fuel 200 :capabilities ()) (while t nil)) (error (er) 'caught))"
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
      (await (spawn (:fuel 400)
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

(`ch` is an ordinary symbol as of 0.3 — H-suffix hex literals require a
leading digit — so name your channels whatever you like.)

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
- `with-capabilities` and `with-fuel` are kernel special forms that let
  *guarded* Lisp code narrow its own authority and step budget for a
  dynamic extent, monotonically — nesting order never matters, narrowing
  can't be undone from inside (`kernel-fuel-set!` is narrow-only, and no
  capability-mask setter exists), it follows the call rather than the
  lexical body, and it stops applying the moment a closure made under a
  fence is called from outside it.
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
