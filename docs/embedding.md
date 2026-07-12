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

## Evaluating Lisp

```rust
use lamedh::{eval_str, eval_all};

// Evaluate one expression, get the typed result.
let val: LispVal = eval_str("(+ 1 2)", &env)?;

// Evaluate multiple top-level expressions; get all results.
let vals: Vec<LispVal> = eval_all("(def x 1) (+ x 2)", &env)?;
```

Both functions return `Err(LispError::Generic(msg))` on parse or runtime errors.

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
