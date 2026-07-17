# Embedding Lamedh

Lamedh can be embedded in any Rust application as a scripting layer.
This page documents the public embedding API.

## Adding as a dependency

```toml
[dependencies]
lamedh = { path = "../lamedh" }   # or publish to crates.io and use version
```

## Quick start

```rust
use lamedh::{LispError, LispVal, environment::Environment, eval_str};
use std::convert::TryFrom;

fn main() {
    // Lamedh uses large stack frames — run on a dedicated thread.
    lamedh::with_large_stack(|| {
        let env = Environment::with_stdlib();
        let result = eval_str("(+ 1 2)", &env).unwrap();
        let n: i64 = i64::try_from(result).unwrap();
        println!("{n}");  // 3
    });
}
```

See `examples/embedding.rs` for the full walkthrough.

## Stack size

The tree-walking interpreter uses large stack frames. If you embed it on the
main thread or a thread with a small stack you will hit a stack overflow before
the recursion-depth guard fires. Two options:

- **Recommended**: wrap your entire embedding entry point in
  `lamedh::with_large_stack(|| { ... })`.  This spawns a single 512 MiB thread.
- **Alternative**: lower the recursion limit with
  `lamedh::set_eval_depth_limit(n)` so the guard fires earlier.

`LispVal` and `Environment` are `!Send`, so you must create the environment
*inside* the closure passed to `with_large_stack`.

## Creating an environment

```rust
// Builtins only (no defun, list utilities, etc.)
let env = Environment::new_with_builtins();

// Builtins + the Prelude (core vocabulary; no shell/testing/optimizer/
// call-graph/condensation/pattern-matching/help database/... until you
// (require 'name) them — see "Module registry" below)
let env = Environment::with_prelude();

// Builtins + the Prelude + every optional embedded library (recommended
// for a fully-featured interpreter with no extra setup)
let env = Environment::with_stdlib();
```

`with_stdlib` evaluates the stdlib Lisp files that are baked into the binary
at compile time.  It panics if the embedded stdlib fails to parse or evaluate,
which should never happen in a correctly-built binary. `with_prelude` is the
same idea over a smaller file list — see `src/lib.rs`'s crate-level doc
comment for the exact split — and is otherwise just as fully-featured (every
kernel builtin is registered either way); it's for hosts that don't want the
extra load time/vocabulary of libraries they won't use, or that want
`require`'s registry to start from a known-small state before selectively
pulling in their own modules.

### Startup cost: the per-thread prototype fork

The first `with_stdlib()` (or `with_prelude()`) call on a thread evaluates
the embedded sources once into a private prototype world; every call —
including the first — then returns a **deep-copy fork** of that prototype
(`Environment::fork_world`): fresh symbol table, fresh global value cells,
fresh closures and containers, in milliseconds instead of a full stdlib
evaluation. Each returned environment is a fully isolated world — nothing
one environment does (defs, redefinitions, plists, dynamic variables,
capability grants, registry mutations) is ever visible in another, exactly
as with a from-scratch load. The prototype never escapes and is retained in
thread-local storage for the life of the thread.

Two consequences for embedders:

- A host that builds **many** interpreter instances on one thread (test
  harnesses, per-request sandboxes) pays the full load once per thread and
  a cheap fork per instance.
- A host that builds exactly **one** environment per process can call
  `Environment::with_stdlib_fresh()` / `with_prelude_fresh()` to skip the
  prototype cache entirely (this is what the `lamedh` CLI does); the result
  is indistinguishable.

The default build's object graph is `Rc`/`RefCell`-based, so prototypes are
per-thread, never shared across threads.

## Evaluating Lisp

```rust
use lamedh::{eval_str, eval_all};

// Evaluate one expression, get the typed result.
let val: LispVal = eval_str("(+ 1 2)", &env)?;

// Evaluate multiple top-level expressions; get all results.
let vals: Vec<LispVal> = eval_all("(def x 1) (+ x 2)", &env)?;
```

Both functions return `Err(LispError::Generic(msg))` on parse or runtime errors.

## Fast-calling a function

`eval_str`/`eval_all` pay for a string build, a full reader parse, and
evaluation on every call. For a host loop that calls the same Lisp function
every frame (an ECS tick, a physics step, an AI decision function), that
parse cost is pure overhead — the call site doesn't change shape, only the
argument values do. `call_function` and `FnHandle` skip the reader (and the
printer — both return a typed `LispVal`, not a string) and apply the
function to already-evaluated `LispVal` arguments directly.

Before, one `eval_str` per frame:

```rust
for frame in 0..n_frames {
    let dt = 0.016_666;
    // Builds a string, re-parses "(level-tick ...)" from scratch, every frame.
    let src = format!("(level-tick {dt:.6})");
    eval_str(&src, &env)?;
}
```

After, resolve the callee once outside the loop, then call it directly:

```rust
use lamedh::{fn_handle, LispVal};

let tick = fn_handle("level-tick", &env)?;   // interns + resolves LEVEL-TICK once
for frame in 0..n_frames {
    let dt = 0.016_666;
    tick.call(&[LispVal::Float(dt)], &env)?;  // no string, no parse
}
```

`fn_handle` pins the *name*, not the function value bound to it at creation
time — `call` re-resolves the symbol's live binding on every call, so a
redefinition between calls (hot-reloading a script, a REPL edit) is picked
up. Creating a handle for a name with no current binding fails immediately,
at `fn_handle`, rather than on the first `call`.

For a one-off call where paying the name lookup each time is fine, skip the
handle:

```rust
use lamedh::call_function;

let result = call_function("level-tick", &[LispVal::Float(0.016_666)], &env)?;
```

Both functions resolve `name` case-normalized (uppercased, matching how the
reader interns symbols) and work for any callable `funcall` accepts —
builtins, lambdas (interpreted or compiled), and typed `defun*`/`defun-typed`
natives. `MACRO`/`VAU`/`FEXPR` values are rejected with an error: their
calling convention takes unevaluated argument forms, which this API — given
only `LispVal`s and no source text — cannot supply; go through
`eval_str`/`eval_all` for those. As with `eval_str`, running under
`with_large_stack` is the caller's responsibility if the callee may recurse
deeply.

Benchmarked on `benchmarks/embedder` (trivial one-arg tick function, per
call): `eval_str` ~2000ns, `call_function` ~530ns, `FnHandle::call` ~480ns,
vs ~540ns for a pre-parsed form run through `evaluator::eval` directly (so
fast-call's win over `eval_str` is almost entirely the string-build + parse
it skips — the rest is the interpreted callee's own cost) and ~190ns for a
typed-registry `jit_call` into a NATIVE `defun*`. `call_function`/`FnHandle`
is the general-purpose middle rung — it works for any callable, not just a
typed/NATIVE `defun*` — where `jit_call` demands one. See
`benchmarks/embedder/README.md` for the full ladder and current numbers.

## Raw native entry points (leaf kernels)

`jit_call` and the fast-call API still cross a *membrane* on every call: they
box arguments into `LispVal`/`jit::Value`, type-check them against the
signature, and re-box the result. For a per-sample host hot loop — marching
cubes sampling a Lisp signed-distance function one point at a time — that
per-call overhead dominates the arithmetic by ~40x (see rungs B2 vs B3 in
`benchmarks/embedder`). When you cannot push the whole loop across the
membrane, `native_entry` (issue #424, requires the `jit` feature) hands the
host a raw pointer into the JIT-compiled machine code, called with no boxing,
no membrane, and no dispatch:

```rust
use lamedh::native_entry;

// (defun* sdf (x float64) (y float64) (z float64) float64
//   (- (sqrt (+ (* x x) (* y y) (* z z))) 1.0))
let sdf = native_entry("sdf", &env)?;      // extract once, outside the loop

for (i, p) in grid_points.iter().enumerate() {
    let d = sdf.call_f3(p.x, p.y, p.z);    // ~11 ns/sample — near-native
    field[i] = d;
}
```

`NativeFnHandle` exposes typed fast paths (`call_f3`, `call_f1`, `call_i1`,
`call_i2`) and a generic `call_words(&[u64]) -> u64` escape hatch (encode each
argument per `params()`: an `int64` as its own word, a `float64` via
`f64::to_bits`, a `bool` as `0`/`1`; decode the result per `ret()`).

Benchmarked on `benchmarks/embedder` (rung B2.5, 3-float sphere SDF):
**~11 ns/sample** — a ~30x drop from the per-sample `jit_call` membrane and
within ~1.8x of the whole-loop-native rung, sums bit-identical to Rust.

### Snapshot semantics — safe across redefinition

A handle pins the **specific native edition** current at extraction time (an
`Arc` snapshot of the compiled code). Redefining the Lisp function does *not*
invalidate or redirect the handle — it keeps running the edition it captured,
so there is no dangling-pointer hazard. Re-extract to pick up a redefinition;
`handle.generation()` compared against `env.jit_generation(name)` tells you
when the live function has moved on and the handle is running an older
snapshot:

```rust
if handle.generation() != env.jit_generation("sdf").unwrap() {
    handle = native_entry("sdf", &env)?;   // pick up the redefinition
}
```

### Leaf restriction (v1)

Only **leaf kernels** are extractable in v1: functions whose compiled body
performs no cross-function call. A leaf's native code never touches the
call trampoline or the function table, which is what makes it safe to call
from any thread with only a lightweight per-call context. Small callees
usually *inline away* automatically (the common case for SDF kernels built
from a few helpers), turning a call-graph into a leaf; when one doesn't,
extraction fails with an error naming the surviving callees, and you can
shrink or restructure it. The signature must also be raw scalars
(`int64`/`float64`/`bool` parameters and return) — compound and boxed types
cannot cross a raw entry.

### Thread-safety

`NativeFnHandle` is `Send + Sync`: cloning it and calling the same handle
concurrently from many threads on disjoint inputs is sound (each call builds
its own per-call context on its own stack). This is exactly the shape a
parallel host loop — a rayon `par_iter` over grid points — wants.

### Caveats: no flags, no fuel

A raw-entry call trades the membrane's safety net for speed, so two behaviors
differ from `jit_call`:

- **Conditions are not propagated.** Integer overflow, division by zero, and
  out-of-bounds/`code-char` errors are *not* raised into the Lisp condition
  system. They are recorded per-thread and readable via
  `handle.last_error()` after a call (which returns `Some(msg)` for the most
  recent call on the current thread, `None` if it was clean) — checking it is
  the host's responsibility. This mirrors the fuel caveat style in
  `docs/mcp.md`.
- **Unmetered.** Raw entries bypass fuel/step accounting entirely; there is
  no interruption budget. Only extract kernels you trust to terminate.

## Value conversion

### Rust → Lisp (`From<T>`)

| Rust type    | LispVal produced              |
|-------------|-------------------------------|
| `i64`        | `LispVal::Number`             |
| `f64`        | `LispVal::Float`              |
| `bool`       | `LispVal::Symbol("T")` / Nil  |
| `String`     | `LispVal::String`             |
| `&str`       | `LispVal::String`             |
| `Vec<LispVal>` | proper list (Cons chain)    |

```rust
let v: LispVal = LispVal::from(42i64);
let list = LispVal::list([1i64, 2, 3]);   // (1 2 3)
```

### Lisp → Rust (`TryFrom<LispVal>`)

```rust
let n: i64    = i64::try_from(val)?;
let f: f64    = f64::try_from(val)?;   // also accepts Number, coerces
let b: bool   = bool::try_from(val)?;  // Nil → false, anything else → true
let s: String = String::try_from(val)?;
let v: Vec<LispVal> = Vec::try_from(val)?;  // proper list only
```

### LispVal helpers

```rust
val.as_number()?     // → i64
val.as_float()?      // → f64 (coerces Number)
val.as_str_val()?    // → &str
val.as_list_vec()?   // → Vec<LispVal>
val.is_truthy()      // → bool
```

## Registering host functions

`register_fn` installs a Rust closure as a callable Lisp function.

```rust
env.register_fn("add-one", |args, _env| {
    if args.len() != 1 {
        return Err(LispError::Generic("add-one takes 1 arg".to_string()));
    }
    let n = args[0].as_number()?;
    Ok(LispVal::from(n + 1))
});

eval_str("(add-one 41)", &env)  // => Number(42)
```

The name is automatically uppercased to follow Lisp symbol conventions.
The function receives evaluated arguments (same as any built-in).

### Accessing the environment from a host function

The second argument is `&Rc<Environment>`. You can call `eval_str` or look up
symbols from inside the host function if needed, but be careful about
re-entrant evaluation.

## Error handling

```rust
match eval_str("(/ 1 0)", &env) {
    Ok(val)  => { /* use val */ }
    Err(LispError::Generic(msg)) => eprintln!("Lisp error: {msg}"),
    Err(e)   => eprintln!("internal error: {e}"),
}
```

`LispError::Return` and `LispError::Go` are internal control-flow signals for
`PROG`/`RETURN`/`GO` and should not escape `eval_str` under normal use.  If
they do, treat them as internal errors.

## Capability flags (sandboxing)

Features gate access to potentially dangerous operations.  All features are
disabled by default.

```rust
// Grant shell access to the script.
env.enable_feature("SHELL");

// Revoke it.
env.disable_feature("SHELL");

// Check from Rust.
env.feature_enabled("SHELL");
```

From Lisp, the same flags are checked automatically when the `SHELL` function
is called.  You can also expose custom flags to Lisp scripts via
`register_fn` wrappers that call `env.feature_enabled("YOUR_FLAG")`.

Current built-in feature flags:

| Flag      | Description                                                |
|-----------|------------------------------------------------------------|
| `SHELL`   | Allow `(shell ...)` to run subprocesses                    |
| `READ-FS` | Allow filesystem reads, `load-file`, metadata, directories |
| `CREATE-FS` | Allow file writes and filesystem mutation               |
| `TEMP-FS` | Allow temporary file and directory creation                |
| `IO`      | Allow `(read)` to read an s-expression from standard input |
| `NET-DNS` | Allow explicit hostname resolution (`net:resolve`)          |
| `NET-CONNECT` | Allow outbound TCP/UDP connections (`tcp:connect`, `udp:connect!`, `udp:send-to`) |
| `NET-LISTEN` | Allow binding/listening for inbound traffic (`tcp:listen`, `udp:bind`) |
| `OS-ENV` | Allow reading process identity/environment (`os:args`, `os:cwd`, `os:env-get`, `os:pid`, `os:hostname`, ...) |
| `OS-ENV-WRITE` | Allow mutating it (`os:chdir!`, `os:env-set!`, `os:env-unset!`) |
| `OS-PROCESS` | Allow spawning child processes (`os:spawn`); a returned handle needs no further grant to wait/kill/terminate it |
| `OS-SIGNAL` | Allow signaling a PID not held as an owned child handle (`os:signal!`) |

## Module registry (`require`)

Issue #256: `Environment::with_prelude()` environments (and `with_stdlib()`
environments, for optional libraries a script requires that aren't already
loaded) pull in named libraries on demand via Lisp's `(require 'name)`. The
embedder-side half of that is a small API on `Environment`, documented in
full in `docs/manual/10-modules.md` §10.7 (the Lisp-facing story: resolution
order, load-once semantics, cycles, `provide`, `require-reload`).

```rust
use lamedh::environment::Environment;

let env = Environment::with_prelude();

// Register your own library's source under a name, without evaluating it
// yet. Takes priority over any embedded module of the same name.
env.register_module("json", include_str!("../lisp/json.lisp"));

// Trigger `(require 'name)` from Rust, without writing Lisp source.
lamedh::require_module("json", &env)?;

// The module names currently REQUIRE-loaded in this environment.
let loaded: Vec<String> = lamedh::loaded_modules(&env);
assert!(loaded.contains(&"JSON".to_string()));
```

`register_module` and `require_module`/`loaded_modules` are independent: you
can register a source and let a script `require` it later, or require it
immediately from Rust. Registered sources are checked *before* the
compiled-in optional libraries, so a host can override e.g. `shell` with its
own implementation under the same name.

`require`'s third resolution tier reads modules from disk, gated behind the
`READ-FS` capability like every other filesystem operation. Configuring
*where* it looks is Rust-only — Lisp can only read the configured list back,
never set it, so a sandboxed script cannot expand its own filesystem module
search path even if it has `READ-FS`:

```rust
env.enable_feature("READ-FS");
env.add_module_search_path("/opt/myapp/lisp-modules");
// env.clear_module_search_paths();  // reset to none
```

A required module's source always evaluates at the environment's root — a
`require` called from inside a nested Lisp call still defines things
globally, not into whatever local frame happened to be active — so there is
nothing extra to arrange on the Rust side for `defun`/`def` inside a
required module to behave as expected.

## Ports (host-wrapped I/O)

Issue #255: `LispVal::Port` is a synchronous binary I/O handle over a file,
an in-memory byte buffer, a standard stream, or (via the two constructors
below) an arbitrary host `Read`/`Write` stream — a pipe, a decompressor, a
captured in-process buffer — exposed to Lisp without ever handing over a
raw file descriptor. See `docs/manual/11-ports-and-io.md` for the
Lisp-facing story (the `PORTS` module, `lib/31-ports.lisp`: `read-byte!`,
`write-bytes!`, `with-open-port`, capability gating, ...).

```rust
use lamedh::LispVal;

let env = Environment::with_stdlib();

// Wrap an arbitrary Read stream as an input port.
let data = b"hello".to_vec();
let port = LispVal::wrap_reader(
    "my-source",              // diagnostic name
    "pipe",                   // diagnostic resource kind
    Box::new(std::io::Cursor::new(data)),
);
env.set("MY-SOURCE".to_string(), port);

// Wrap an arbitrary Write sink as an output port.
let sink = LispVal::wrap_writer("my-sink", "capture", Box::new(std::io::sink()));
env.set("MY-SINK".to_string(), sink);
```

```lisp
(import ports)
(array->list (read-all-bytes! my-source))  ; => (104 101 108 108 111)
(write-string! my-sink "hi")               ; writes through to the wrapped Write
```

Host-wrapped ports are not seekable and need no capability from Lisp's
side — the host already decided to hand this stream over by constructing
the `LispVal`, exactly like `register_fn`. Every other port operation
(`read-byte!`, `write-bytes!`, `flush!`, `close!`, `port-p`, ...) treats a
host-wrapped port exactly like a file or memory port. Ports compare by
identity and print as an opaque `#<port:kind "name" open|closed>`
diagnostic — never as a readable literal.

## Networking policy (scoped grants)

Issue #258: `NET-DNS`/`NET-CONNECT`/`NET-LISTEN` (above) are coarse
on/off switches, exactly like `READ-FS`/`CREATE-FS`. That is too coarse
for a common embedding shape — granting `NET-CONNECT` so a script can use
an HTTP-client library must not become unrestricted SSRF authority (any
host, any port, including internal/link-local addresses). `set_net_policy`
is a Rust-only hook consulted *in addition to* (not instead of) the
capability check, before every DNS resolution, outbound connection, or
bind/listen:

```rust
use lamedh::NetOperation;

let env = Environment::with_stdlib();
env.enable_feature("NET-CONNECT");

env.set_net_policy(|op, host, port| {
    // Block the cloud-metadata address regardless of port; allow
    // everything else this capability already permits.
    !(op == NetOperation::Connect && host == "169.254.169.254")
});

// Revert to allow-all-when-capability-granted (the default).
// env.clear_net_policy();
```

The callback receives the operation (`NetOperation::Resolve`/`Connect`/
`Listen` — `UDP:CONNECT!`/`UDP:SEND-TO` check as `Connect`, `UDP:BIND` as
`Listen`, the same shape as TCP) and the caller-supplied host/port —
**not** a post-DNS-resolved IP address, so a policy that only checks the
literal hostname string will not catch a malicious DNS answer resolving
an allowed name to an internal address; re-resolving and re-checking the
IP is the caller's responsibility if that threat model matters. Returning
`false` denies with a structured `:POLICY-DENIED` error. `None` (never
installing a policy) is the default and allows every operation once its
capability is granted. Lisp code cannot install, replace, or inspect the
policy — like `add_module_search_path`, this is deliberately Rust-only.

See `docs/manual/13-networking.md` for the Lisp-facing story (`NET`/
`TCP`/`UDP` modules, `NET:ADDRESS`, structured error categories).

## TLS (feature flag + insecure-connect opt-in)

Issue #365: `TLS` (`lib/43-tls.lisp`) wraps a connected TCP `Port` as a
client or server. It requires two separate things from an embedder, one at
build time and one at runtime:

**1. The `net-tls` cargo feature** (off by default — the owner's ruling,
#364/#365, so the default dependency tree/behavior stays exactly as it
was):

```toml
[dependencies]
lamedh = { path = "...", features = ["net-tls"] }
```

With this feature compiled out, `lib/43-tls.lisp` still loads (every
`tls:*` name is bound), but every operation except `tls:available-p`
signals a structured `:CATEGORY :TLS-UNAVAILABLE` error. Check
`(tls:available-p)` from Lisp, or `cfg!(feature = "net-tls")` from Rust, to
tell which build you have.

**2. `Environment::set_allow_insecure_tls`, for `tls:connect-insecure!`
only.** Every ordinary `tls:connect`/`tls:wrap-client` call verifies the
peer certificate; `tls:connect-insecure!`/`tls:wrap-client-insecure!` are
the *only* Lisp-facing way to skip verification, and even they refuse to
run unless the host has separately opted in — mirroring `set_net_policy`'s
"Rust-only to install" shape one level further: Lisp code alone can never
disable certificate verification, no matter what it calls.

```rust
let env = Environment::with_stdlib();
env.enable_feature("NET-CONNECT");

// Left at the default (false): tls:connect-insecure! always signals a
// structured :POLICY-DENIED error, regardless of what Lisp code does.

// A host that has a real reason to allow it (e.g. a developer-only debug
// tool, or a test harness that would otherwise need :extra-roots instead):
env.set_allow_insecure_tls(true);
```

Only present when the `net-tls` feature is enabled (it would be dead code
otherwise — there is no insecure-connect operation to gate without TLS
compiled in at all). Root-of-trust extension for the *ordinary*, verified
path (a private/internal CA, or a test harness's throwaway CA) does not
need this flag at all: pass PEM data as `:extra-roots` to `tls:connect`/
`tls:wrap-client` instead, which is almost always the better fit — it
keeps verification on, just against a different trust store.

See `docs/manual/13-networking.md` §13.8 for the full Lisp-facing story
(certificate/ALPN/SNI diagnostics, structured error categories, the
`https://` integration in `HTTP`).

## OS policy (scoped process/signal grants)

Issue #260: `OS-PROCESS`/`OS-SIGNAL` (above) are the same kind of coarse
on/off switch as `NET-CONNECT` — a `OS-PROCESS` grant so a script can spawn
a specific build tool must not become unrestricted "run any executable
with any argv" authority. `set_os_policy` mirrors `set_net_policy`: a
Rust-only hook consulted *in addition to* (not instead of) the capability
check, before every spawn or signal delivery:

```rust
use lamedh::OsOperation;

let env = Environment::with_stdlib();
env.enable_feature("OS-PROCESS");

env.set_os_policy(|op| match op {
    // Only ever allow spawning this one build tool, and only from the
    // project directory.
    OsOperation::Spawn { program, cwd, .. } => {
        *program == "/usr/bin/make" && *cwd == Some("/srv/build")
    }
    OsOperation::Signal { .. } => false,
});

// Revert to allow-all-when-capability-granted (the default).
// env.clear_os_policy();
```

The callback receives an `OsOperation`: `Spawn { program, args, cwd }`
(`args` is the argv passed to `os:spawn`, not including `program` itself)
or `Signal { pid, signal }` (the numeric signal, resolved from the typed
name Lisp passed to `os:signal!`). Returning `false` denies with a
structured `:POLICY-DENIED` error. `None` (never installing a policy) is
the default and allows every operation once its capability is granted.
Lisp code cannot install, replace, or inspect the policy.

Note the scope: this hook governs `os:spawn` and `os:signal!` only.
`os:process-kill!`/`os:process-terminate!` operate on an already-returned
child handle and are not re-checked against the policy (Chapter 11's
"an open handle is authority to keep using it" rule) — scope what you are
willing to let `os:spawn` start, not what a script can do to a child it
already legitimately started.

See `docs/manual/14-os.md` for the Lisp-facing story (`OS`/`OS-LINUX`
modules, structured error categories, the Drop backstop).

## Condition flags

Condition flags are boolean markers in the environment, distinct from feature
flags. They are used to pass state between Lisp and host code.

```rust
env.set_flag("DEBUG");
env.flag_set("DEBUG");   // → true
env.clear_flag("DEBUG");
env.clear_all_flags();
```

From Lisp: `(set-flag "DEBUG")`, `(flag-set-p "DEBUG")`, `(clear-flag "DEBUG")`.
