# v0.3.0 — unreleased

## Fuzz battery off the default path (dev tooling)

`tests/brutal_correctness.rs` — the randomized JIT differential/metamorphic
fuzz battery (nine batches, tens of thousands of random programs, minutes in
debug) — is now gated behind a non-default `fuzz` cargo feature. Plain
`cargo test` compiles it to nothing and skips it entirely; the ship gauntlet
(`scripts/gauntlet.sh`) and CI run `--features fuzz` in **release**, where the
same battery is seconds. `BRUTAL=1` still scales every batch to its deep
sweep. This takes the single biggest tax off day-to-day and agent iteration
with no loss of ship-gate coverage.

## perf(startup): with_stdlib world fork + module-rewrite kernel builtin

Two attacks on stdlib-loading cost, one per consumer shape:

**Many environments per process** (test suites, per-request sandboxes):
`Environment::with_stdlib()`/`with_prelude()` now build a private
per-thread prototype world once and serve every call as a **deep-copy
fork** (`Environment::fork_world`): a memoized graph copy that
duplicates the symbol table (same names/ids, fresh `Rc` cells), every
global value cell, environment frame, closure (including pre-compiled
`Code` bodies), hash table, array, plist, and the typed registry —
preserving sharing and `EQ` identity isomorphically, sharing only
immutable/host-owned values (builtins, `Native`/`Extension` handles,
routing tables, typed declaration-plane definitions, closure-tier
compiled editions). Forked worlds share no mutable cell, so isolation
is exactly that of independent full loads; a world holding live host
handles (ports/sockets/children/channels) refuses to fork and the
caller falls back to a real load. Measured: a fork costs ~65 ms
release / ~130 ms debug vs ~740 ms / ~5.6 s for a full load, and
`cargo test --test test_examples` (61 fresh environments on one
large-stack thread) drops accordingly. `tests/test_helpers.rs`'s
`env_with_stdlib` now rides `with_stdlib()` instead of re-reading
`lib/` from disk. New `tests/test_world_fork.rs` pins the contract:
isolation (defs, redefinitions, plists, registries, capability flags,
dynamic vars, typed definitions), symbol identity, late binding, full
vocabulary, fresh/fork agreement, and fork-is-cheap.

**One environment per process** (the CLI): cold start was dominated by
the module system's rename walk — `$MODULE-REWRITE` did an O(renames)
`ASSOC` per AST node over every `WITH-MODULE` body (loading
`40-http.lisp` alone cost ~390 ms). The walk is now the `SEXPR-RENAME`
kernel builtin (hash-table lookup per symbol, quote/quasiquote
untouched, exact same semantics) plus a single merged
`module.functions` registry write instead of a quadratic per-name
append; `with-module` keeps the same surface. New
`Environment::with_stdlib_fresh()`/`with_prelude_fresh()` run the
loader directly with no prototype retention; the CLI uses
`with_stdlib_fresh()` (it builds exactly one world). Measured:
`40-http.lisp` 393 ms → ~118 ms, full embedded load ~985 ms → ~710 ms
on the same machine.

## Gauntlet speed (dev tooling)

`scripts/gauntlet.sh` runs the authoritative verification in **release**
(the `test_examples` and `brutal_correctness` suites are execution-bound —
release runs them several times faster than debug, repaying the one-time
compile) and writes an explicit verdict file the caller gates ship on.
`brutal_correctness`'s default smoke sweep dropped 4_000 → 1_200 random
programs (`BRUTAL=1` still runs the 50_000-program deep sweep;
`BRUTAL_PROGRAMS` overrides either default). No behavior change to the
interpreter — verification only.

condensation

Everything from the v0.2.0 tag to here. The headline: **one record
definition form** over one type story — records, sums, HM generics,
guards, processes, patterns, modules, and the checker meeting in one
language. Sections below, roughly newest first.

## modules: help subsystem registered; #56 namespacing complete

`doc-renderer` and `help-system` gain metadata-only `defmodule`
declarations, completing issue #56: **every optional stdlib library is now
a registered module.** The help surface stays flat — `help`/`apropos` are
REPL UX and `register-doc`/`register-category` are the documentation API
called across the whole codebase — so this is registration, not
qualification.

With this, the #56 retrofit is done. Two libraries were fully **qualified**
because their public names are genuinely tool-shaped: `shell` (`shell:sh`,
…) and the `match` search/rewrite tools (`match:sgrep`, `match:rewrite`,
…). Every other optional — the language-defining forms (variants,
protocols, guard, condensation's records, types), the ergonomically-flat
DSLs (testing, instrument, lisp15), the pipeline-hook/kernel-pinned
surfaces (rules, optimizer-vau, call-graph), and the help subsystem — is a
**metadata-only registered module** whose names stay exactly where they
were. Introspect any of them with `(module-p 'name)`,
`(module-exports 'name)`, `(module-requires 'name)`. The prerequisite
load-order reorder (module system ahead of the optionals) is the
foundation this all rests on.

## modules: sgrep/rewrite tools move under the `match` namespace (#56)

The structural search-and-rewrite tools in `lib/23-match.lisp` are now
qualified under the `match` module: `match:sgrep`, `match:sgrep-fn`,
`match:sgrep-source`, `match:sgrep-file`, `match:rewrite`. The matcher
**engine stays flat** — `pat-match`, `match`, `destructuring-bind`,
`instantiate`, `match-fail-p` are unchanged: `match`/`destructuring-bind`
are language forms, and `pat-match`/`instantiate` are the deconstruct/
reconstruct engine primitives other libraries build on directly (e.g.
`lib/24-rules.lisp` uses flat `pat-match`/`instantiate` and is unaffected).
Only the clearly tool-shaped `#171` search/rewrite surfaces are namespaced.
`instantiate` was moved up beside the engine so the tool functions form one
`with-module` block; the flat engine names they call are not module-local,
so `with-module` leaves those calls untouched. **Behavior change**: call
the tools qualified (`match:sgrep …`) or `(import match)`.

## modules: register the flat optional libraries as modules (#56)

Eleven optional libraries whose names are deliberately kept flat —
`lisp15`, `testing`, `optimizer-vau`, `call-graph`, `guard`, `rules`,
`variants`, `instrument`, `types`, `protocols`, `help-data` — now each
carry a `(defmodule NAME (:export ...))` declaration for introspection
and registration. This is **metadata only**: there is no `with-module`
body rewrite and nothing is imported, so every name stays exactly where
it was (`deftest`, `defvariant`, `defprotocol`, `trace`, `attrib` … are
all still flat) and no caller changes. The gain is uniform module
introspection: `(module-p 'testing)` → `T`, `(module-exports 'variants)`,
`(module-requires 'guard)`, alongside the fully-qualified modules
(`shell`, and the #253 modules). These libraries stay flat by design —
they are language-defining forms (variants, protocols, guard) or
ergonomically-flat DSLs (the `testing` assertions, the `instrument`
debug verbs, the Lisp 1.5 compat names) — so registration, not
qualification, is the right treatment. `types` and `help-data` are data
modules (type-table axioms / help database) with empty export sets.

## modules: shell helpers move under the `shell` namespace (#56)

`lib/07-shell.lisp` is now the `shell` module (`defmodule shell`, built on
the reordered load sequence from the previous entry). Its five helpers are
qualified: `shell:sh`, `shell:shell-exit-code`, `shell:shell-stdout`,
`shell:shell-stderr`, `shell:shell-ok-p` — call them qualified or bind them
flat with `(import shell)`. The module declares `(:requires SHELL)` for
capability introspection (`(module-requires 'shell)`). The bare kernel
`shell` builtin (the `(exit-code stdout stderr)` primitive) is unchanged and
stays flat — only the Lisp helper layer is namespaced. **Behavior change**:
`(sh ...)` and the flat accessors are no longer bound by default; use
`shell:sh` or `(import shell)`. Guard's `capabilities-needed` still infers
`SHELL` through `shell:sh` (it walks the call graph into the kernel
primitive). First of the #56 tool-library retrofits.

## internal: stdlib load order — module system ahead of the optionals (#56 prep)

Reordered the embedded `STDLIB_SOURCES` load sequence so the module
system (`20-condensation.lisp` then `27-modules.lisp`) loads immediately
after the Prelude, ahead of every optional library. Previously the
pre-0.3 optionals (`07-shell`, `10-testing`, `11-optimizer-vau`,
`19-call-graph`, `22-guard` … `26-instrument`) loaded *before*
`27-modules.lisp`, so none of them could be wrapped in
`DEFMODULE`/`WITH-MODULE` at their own load position — the blocker for
the #56 namespacing retrofit. Only the load ORDER in `src/lib.rs`
changed; filenames keep their historical numbers, the optionals are
reordered by their real dependency edges (optimizer-vau → rules,
call-graph → guard, types → protocols), and `PRELUDE_SOURCES`
(`with_prelude`) is untouched. No user-visible behavior change: every
name still resolves identically; this only makes the module system
available to the libraries that will be retrofitted onto it.

## io: `read-file-section` decodes strictly — no more silent lossy text coercion (#359)

`read-file-section` decoded its bytes with `from_utf8_lossy`, silently
substituting U+FFFD for any non-UTF-8 byte — exactly the implicit lossy
text coercion epic #253's design rules out (text must cross the byte
boundary explicitly). It now decodes **strictly**: invalid UTF-8 signals a
descriptive error naming the offending byte offset, mirroring
`text:utf8->string`. Two explicit opt-ins replace the old silent behavior:
`read-file-section-lossy` (U+FFFD substitution, like
`text:utf8->string-lossy`) and `read-file-section-bytes` (the raw bytes as
an `Array<Char>`, to cross the text boundary yourself via the `text:` /
codec substrate). Both are READ-FS-gated like their sibling. `read-file`
(whole-file) already errored on invalid UTF-8 via `read_to_string`, and
`read-file-byte` already returned a raw integer — neither was ever lossy,
so neither changed. **Behavior change**: code that relied on
`read-file-section` silently tolerating non-UTF-8 input must now call
`read-file-section-lossy` (same result) or `-bytes` (raw). Tests:
`tests/test_read_file_section.rs`.

## stdlib: tail-recursive Prelude list/string builders — no more eval-frame overflow on ordinary-sized input (#361)

`string->list`, `filter` (the LIST fallback under the protocol), `take`,
`take-while`, `butlast`, `zip`, `pairlis`, `copy` (and its actual dispatch
target for lists, `copy-list*`), `remove-duplicates`,
`string-join`, `string-repeat` (and `make-string`, which calls it),
`string-capitalize`'s word-walk, and `string-split` recursed
non-tail-recursively — each built its result via `(cons x (recurse ...))`
(or the CONCAT equivalent for the string builders) with the recursive call
*inside* the cons, not in tail position. That is genuinely non-tail: it
consumed one native eval frame per input element, hitting the ~10000-frame
recursion guard on inputs of only a few thousand elements/characters —
`(string->list <4kb string>)` or `(filter #'evenp <5000-element list>)`
errored outright. Found during #257 (codecs); its workaround was local to
the codec modules, so every flat-Prelude caller stayed exposed.

Each is now a tail-recursive accumulator (+ `reverse`/`reverse-aux` at the
end) or, for the string builders, a direct accumulating concatenation —
the evaluator's TCO keeps native stack usage O(1) regardless of input
size. Purely an internal rewrite: arguments, return values, and edge
cases (empty input, `n <= 0`, `n` past the end, dotted tails) are
unchanged; new `$`-prefixed accumulator helpers are internal. Declared/
inferred type schemes for every touched public function are unchanged
(verified via `see-type` before/after).

## runtime(net): TLS port wrapping behind an off-by-default cargo feature (#365)

A new optional embedded module, `TLS` (`lib/43-tls.lisp`), wrapping a
connected TCP `Port` (#258) as a TLS client or server — the seam #258's own
design predicted. Backed by `rustls` + `webpki-roots` + `rustls-pemfile`
(owner ruling, #364/#365: rustls over native-tls, so nothing here links
system OpenSSL/SChannel/SecureTransport), all three behind a new **`net-tls`
cargo feature that is NOT in the default feature set** — the default build's
behavior and dependency tree are byte-identical to before this change; only
`cargo build --features net-tls` (or `cargo test --features net-tls`) pulls
in a TLS crate at all. The crypto provider is `ring`, not `aws-lc-rs`: no
cmake/nasm build-tool requirement.

A TLS stream is an ordinary `PORTS` `Port` (two new `PortState` variants,
themselves gated by `net-tls`): every `PORTS` read/write/close operation and
every `TCP` out-of-band operation (`shutdown!`, `set-read-timeout!`/
`set-write-timeout!`, `local-addr`/`peer-addr`) keeps working on it
unchanged. `tls:wrap-client`/`tls:wrap-server` take ownership of the
underlying TCP connection out of the plaintext `Port` you pass in (it
becomes `CLOSED` the instant it is wrapped — never simultaneously reachable
as both plaintext and TLS); `tls:connect`/`tls:connect-insecure!` are
`tcp:connect` + wrap sugar for the common case.

Verification is on by default (default root store: Mozilla's set, via
`webpki-roots`, plus any `:extra-roots` you supply as PEM — a String path,
`READ-FS`-gated, or `Array<Char>` bytes — this is also how a test harness
trusts a throwaway self-signed CA) and checks the certificate against
`:hostname`, also used for SNI. `:alpn` offers protocol names;
`tls:alpn-protocol` reads back what was negotiated. `tls:peer-certificates`/
`tls:peer-certificate-summary` expose the peer's raw DER chain and a
structural summary (count, leaf length, leaf bytes) — no X.509 parser is
part of this dependency ruling, so there are no parsed subject/issuer/expiry
fields, by design. `tls:sni-hostname` reads the SNI a client offered,
server-side. Server cert/key are supplied the same way as `:extra-roots`
(PEM path or bytes); a handshake timeout is the underlying TCP port's own
read/write timeout, set before (or after) wrapping.

**No insecure bypass without an explicitly named API, and a host opt-in.**
`tls:connect-insecure!`/`tls:wrap-client-insecure!` are the only way to skip
certificate verification — there is no keyword flag on `tls:connect` that
silently does it — and even they signal a structured `:policy-denied` error
unless the *host* embedding this interpreter separately calls the new
`Environment::set_allow_insecure_tls` (Rust-only, default `false`, modeled
on `set_net_policy`'s "Lisp cannot install/inspect this" shape). Lisp code
alone can never disable verification, no matter what it calls. Key material
is never exposed to any printer path.

**Feature-off behavior, not feature-off absence.** Every `TLS-*` kernel
builtin is registered unconditionally, so `(require 'tls)` and every
`tls:*` name always resolve regardless of how the crate was built — with
`net-tls` compiled out, every one of them except `tls:available-p` (which
reports the compiled-in state) signals a structured `:category
:tls-unavailable` error instead of doing any work, rather than an
unbound-variable error.

`HTTP` (#259, `lib/40-http.lisp`) gains `https://` client support when
`net-tls` is compiled in: `$http-check-scheme!` — the single scheme
checkpoint the plaintext-only design deliberately isolated for exactly this
— now permits `https://` (initial request or redirect `Location` alike)
when `(tls:available-p)`, connecting via `tcp:connect` then `tls:connect`
before speaking HTTP/1.1 over the resulting port; `http:request`/`get`/
`post` gained an `:extra-roots` keyword forwarded to `tls:connect`. With
`net-tls` compiled out, `https://` keeps its previous structured
`:https-unsupported` error (now naming the `net-tls` feature rather than
this now-closed issue). Redirects still never silently cross schemes.
Server-side `https://` (wrapping an accepted connection before `serve`/
`serve-one!` parses it) is out of scope here — `tls:wrap-server` already
composes directly with `tcp:accept` for a caller that wants one without
going through `HTTP`'s own accept loop.

Tests: `tests/test_tls.rs` (`#[cfg(feature = "net-tls")]`-gated,
loopback-only) — a throwaway self-signed CA + "localhost" leaf certificate
generated fresh per test via the `rcgen` **dev-dependency** (never a
build/normal dependency, so it does not affect `net-tls`'s "off by default"
story for embedders); client/server round trip both directions of the wrap,
negotiated ALPN, peer-certificate summary, verification failure against an
untrusted CA with the default root store, `connect-insecure!` denied without
the host opt-in and working with it, and `https://` end to end through
`http:request`. Feature-off behavior is covered in the normal suites instead
(`tests/test_net.rs`, `tests/test_http.rs`), gated
`#[cfg(not(feature = "net-tls"))]` so they run in the default and
`--no-default-features` builds and stay correct once `net-tls` is on.
Manual: `docs/manual/13-networking.md` §13.8; embedding: `docs/embedding.md`.

## runtime(os): typed Linux/POSIX interface, opaque owned handles, no raw syscalls (#260)

Two new optional embedded modules over a new `OS-*`/`OS-LINUX-*` Rust
kernel substrate (`src/evaluator/builtins_os.rs`, zero new crate
dependencies — `std::env`/`std::process`/`std::fs`/`std::time` only,
plus one hard-coded `kill(2)` FFI declaration, see below): `OS`
(`lib/41-os.lisp`, portable) and `OS-LINUX` (`lib/42-os-linux.lisp`,
typed Linux-specific facilities), per the epic's "portable module + a
Linux-only module, unsupported operations signal structured errors
rather than binding opaquely" ruling.

**No raw syscall numbers, no bare file descriptors/PIDs as authority.**
A spawned process is `LispVal::OsChild` — an opaque handle (compares by
identity, deterministic `wait!`/`try-wait!` reap, Drop backstop) never
reconstructible from a printed integer; its stdio pipes (when
requested) are ordinary #255 `PORTS` ports, reusing the existing
`wrap_reader`/`wrap_writer` embedder API rather than adding new port
representations. Signals go only through a fixed typed-name table
(`os:signal!`, `:term`/`:kill`/`:hup`/...) — never a raw signal number.

`OS`: process identity/environment (`args`, `executable-path`, `cwd`,
`chdir!`, `env-get`/`env-list`/`env-set!`/`env-unset!`, `pid`, `ppid`,
`hostname`), time (`now`, `monotonic-nanos`, `elapsed-seconds`,
`sleep`/`sleep-seconds`), randomness (`make-prng`/`prng-next` — a pure,
explicitly-seeded SplitMix64 generator — and `random-bytes`, OS-backed
secure entropy from `/dev/urandom`; the pre-existing global `random`
convenience function is untouched, not replaced), and process
spawn/control (`spawn` — explicit argv with **no shell interpolation**,
explicit/inherited environment, per-stream `:inherit`/`:null`/`:pipe`
stdio, returning a `(:handle :stdin :stdout :stderr)` alist —
`process-wait!`/`process-try-wait!`/`process-kill!`/`process-terminate!`/
`process-alive-p`/`process-id`, and `signal!` for PIDs not held as an
owned handle). `OS-LINUX`: `stat`/`lstat` (typed alist — size, mode,
uid/gid, nlink, ino/dev, mtime/atime/ctime, is-dir/is-file/is-symlink;
never a raw C struct) and `readlink`.

**Capabilities** (all off by default, same `require_*`/fence-attenuation
machinery as `READ-FS`/`NET-CONNECT`): `OS-ENV` (read identity/env),
`OS-ENV-WRITE` (mutate it), `OS-PROCESS` (spawn — a returned child
handle needs no further grant to `wait!`/`try-wait!`/`kill!`/
`terminate!`, the epic's "a returned handle is authority to continue"
rule), `OS-SIGNAL` (signal a PID not held as an owned handle).
`OS-LINUX-STAT*`/`OS-LINUX-READLINK*` reuse the existing `READ-FS` grant
rather than inventing a parallel one. Time/randomness are ungated (pure
or read-only-entropy, mirroring the pre-existing global `random`). A new
host policy hook, `Environment::set_os_policy`/`OsOperation`, mirrors
`set_net_policy` for spawn/signal (scope a grant to a specific
executable/argv/cwd or target PID) — checked in addition to, not
instead of, the capability.

**Structured errors** throughout (`:category` `:not-found`/
`:permission-denied`/`:already-exists`/`:invalid-argument`/`:closed`/
`:policy-denied`/`:unsupported-platform`/`:signal-failed`/`:other`),
following #258's `builtins_net.rs` pattern.

**Deferred** out of this increment (see the ticket's own "only after the
basic resource/polling model is stable" sequencing): a standalone `pipe`
primitive not tied to a spawned child, terminal/PTY settings,
eventfd/timerfd/signalfd/epoll, Unix-domain sockets, and openat-style
directory-relative operations.

**`kill(2)` FFI**: `SIGTERM`/arbitrary-PID `SIGNAL!` need a signal number
Rust std has no safe API for; `src/lib.rs` declares one hard-coded
`unsafe extern "C" { fn kill(pid: i32, sig: i32) -> i32; }` (the same
technique `std::process::Child::kill` uses internally for `SIGKILL`) —
a single fixed, typed, internal binding, not the "arbitrary FFI"/"raw
syscall numbers" the epic rules out; Lisp only ever reaches it through
the fixed signal-name table and the typed kernel primitives.

Relation to `lib/07-shell.lisp`: `shell`/`sh` (unchanged) remain the
`/bin/sh -c` convenience escape hatch; `os:spawn` is this ticket's
principled typed layer — explicit argv, no shell interpolation,
non-blocking wait, and kill/terminate control none of `shell`'s API
exposes.

## stdlib(http): capability-gated streaming HTTP/1.1 client and server (#259)

A new optional embedded library, `HTTP` (`lib/40-http.lisp`) — an
HTTP/1.1 client and server written **entirely in Lisp** over the #258
TCP substrate and the #257 codecs (`URL` for parsing, `MIME` for
ordered/case-insensitive multi-value headers, `JSON` for
`http:collect-json`): **zero new crate dependencies, zero new Rust
kernel surface** (the ticket's "back it with a Rust HTTP crate" text was
overridden by the epic's no-new-dependency ruling; status-line/header/
chunked framing proved to be ordinary Lisp string/byte work). HTTP/2 and
HTTP/3 are out of scope. `(require 'http)` on a prelude environment;
`with_stdlib()` loads it eagerly like every optional module.

**Plaintext `http://` only**: TLS is pending the owner dependency ruling
(#365), so `https://` — given directly or arriving via a redirect
`Location` — is a structured `:CATEGORY :HTTPS-UNSUPPORTED` error naming
that issue, checked at one single scheme checkpoint where TLS later
slots in without API change; never a silent downgrade.

Client: `http:request` (+ `http:get`/`http:post`), request bodies
NIL/String/`Array<Char>`/readable port (ports stream out chunked);
response `:BODY` is an unread framing-aware body stream —
`Content-Length` exact / chunked (hex size lines, extensions, trailers,
zero-chunk terminator) / read-to-close / no body for HEAD/1xx/204/304 —
read incrementally (`http:stream-read!`/`stream-eof-p`/`stream-close!`)
or collected bounded (`http:collect-bytes`/`-string`/`-json`, 10 MiB
default cap; text is an explicit UTF-8 step). `Connection: close`
always (no pooling). Timeouts: connect / per-read / overall deadline.
Redirects: 301/302/303/307/308, hop-capped, 303→GET, 301/302 POST→GET,
307/308 method+body-preserving (unreplayable streamed bodies are a clear
error), cross-origin hops strip `Authorization`/`Cookie`/
`Proxy-Authorization`; no ambient proxy variables.

Server: `http:serve`/`http:serve-one!` over a `tcp:listen` listener;
handler gets a request alist (path/query/headers/streaming body — CL and
chunked framing both) and returns `http:respond` alists (body
NIL/String/bytes/port, `Content-Length` set automatically, port bodies
chunked). Serial keep-alive: one connection served fully before the next
accept (concurrency is #140's business); request-line/header-size/
header-count/body-size limits enforced (oversize body → `413` without
running the handler); an uncaught handler error is a generic `500` that
never leaks the condition to the peer (`:on-error` receives it
host-side); `:stop-p`/`:max-requests` for between-connection shutdown.

**No new capability**: requiring `HTTP` grants nothing — the client
rides `tcp:connect`'s `NET-CONNECT` gate, the server `tcp:listen`'s
`NET-LISTEN`, both fence-attenuated as ever (tested for both sides).

Fixed en route: `lib/27-modules.lisp` now declares its real dependency
`(require 'condensation)` — under `with_prelude()`, `(require 'modules)`
followed by any `WITH-MODULE` body (e.g. `(require 'text)`) died with
"unbound variable: CONDENSE-APPEND-NEW" because `with_stdlib()`'s eager
load order had been masking the missing declaration.

Tests: `tests/test_http.rs` — loopback-only `std::thread` peers,
OS-assigned ports; chunked/CL/204/304 framing, repeated headers,
case-insensitive lookup, truncated bodies, malformed status lines, read
timeouts, >100 KB bodies both framings (tail-recursive, stack-safe),
redirect matrix incl. loop capping and credential stripping, server
parsing/dispatch/keep-alive/413/500-sanitization, capability denial and
fence attenuation. Manual: `docs/manual/13-networking.md` §13.7.

## runtime(net): capability-gated DNS, TCP, and UDP over binary ports (#258)

Lamedh now has synchronous networking: DNS resolution, TCP (connect/bind/
listen/accept/shutdown/timeouts), and UDP (bind/send-to/receive-from),
built entirely on `std::net` — **zero new crate dependencies**. Three new
optional embedded libraries follow the epic #253 namespace ruling (a
genuinely new facility, not flat-namespace growth): `NET`
(`lib/37-net.lisp`, addresses/DNS), `TCP` (`lib/38-tcp.lisp`), and `UDP`
(`lib/39-udp.lisp`). **TLS is explicitly deferred** — see below.

Representation: a connected TCP stream is an ordinary `LispVal::Port`
(#255) — a new `PortState::TcpStream` variant — so every `PORTS` operation
(`read-byte!`/`write-bytes!`/`close!`/`with-open-port`/`port-p`/...) works
on it unchanged, and it is the seam a future TLS layer wraps without an
API change. Listeners and UDP sockets are not byte streams, so they get
their own opaque resource, a new `LispVal::NetHandle` (`NetHandleObj` in
`src/lib.rs`) — identity-compared, printed, self-evaluating, and Drop-
backstopped exactly like `Port`. Kernel substrate:
`src/evaluator/builtins_net.rs`.

Addresses: `NET:ADDRESS` is a `DEFRECORD` (family/ip/port) — first-class,
printable data; the kernel never hands a raw `std::net::SocketAddr` to
Lisp, only a `(family ip port)` triple that `NET` wraps.
`NET:ADDRESS->STRING` brackets IPv6 (`"[::1]:8080"`). `NET:RESOLVE`
resolves a host (and optional service port) to an ordered list of
addresses; `NET:LOCAL-ADDR`/`NET:PEER-ADDR` inspect a connected port or
handle.

TCP: `TCP:CONNECT` (optional timeout-ms), `TCP:LISTEN` (backlog argument
accepted for API completeness but currently advisory-only — `std::net`
has no OS backlog customization without an extra crate), `TCP:ACCEPT`
(returns `(port . peer-address)`), `TCP:SHUTDOWN!` (`:READ`/`:WRITE`/
`:BOTH`), `TCP:SET-READ-TIMEOUT!`/`SET-WRITE-TIMEOUT!`,
`TCP:CLOSE-LISTENER!` (idempotent; every subsequent `ACCEPT` errors
immediately — a blocked `ACCEPT` on another OS thread is not guaranteed
to unblock, since plain `std::net::TcpListener` has no portable
wakeup-on-close).

UDP: `UDP:BIND`, `UDP:CONNECT!` (sets a default peer), `UDP:SEND-TO`/
`UDP:SEND`, `UDP:RECEIVE-FROM` (returns `(bytes peer-address
possibly-truncated-p)` — datagram boundaries are preserved; the
truncation flag is a best-effort "received length equals the requested
buffer size" signal, since plain `std::net` exposes no `MSG_TRUNC`
without raw syscalls, out of this issue's no-new-dependency/no-ioctl
scope).

Capability model: three new ambient authorities, checked through the same
`require_*`/`cap_mask_allows` dynamic-extent-fence pattern as
`READ-FS`/`CREATE-FS`/`IO` (#320/#325) — verified directly, a networking
call inside `(with-capabilities () ...)` fails even when the host/CLI
granted the capability. `NET-DNS` gates explicit resolution
(`NET:RESOLVE`); `NET-CONNECT` gates outbound connections/sends
(`TCP:CONNECT`, `UDP:CONNECT!`, `UDP:SEND-TO`); `NET-LISTEN` gates
binding/listening (`TCP:LISTEN`, `UDP:BIND` — even an ephemeral-port UDP
socket receives from any sender once bound, so it needs the "inbound
traffic" capability like a TCP listener does). Once a resource is
acquired, continued use needs no further check (#255's "a successfully
returned handle is authority to continue" rule) — reading/writing an open
TCP port, or sending on an already-`CONNECT!`ed UDP socket, is
unrestricted.

In addition to the capability, a new Rust-only embedder hook,
`Environment::set_net_policy` (`src/environment.rs`), is consulted before
every resolve/connect/bind with the operation and the caller-supplied
host/port (not yet DNS-resolved): a scoped-grant mechanism so a broad
`NET-CONNECT` grant (e.g. for an HTTP client) does not become
unrestricted SSRF authority. `None` (the default) allows every operation
once its capability is granted; Lisp code cannot install, replace, or
inspect the policy. Errors are structured `LispVal::Error` values whose
`data` alist includes `:OPERATION`/`:CATEGORY`/`:HOST`/`:PORT`/
`:OS-ERROR`, `:CATEGORY` being one of `:TIMEOUT`/`:REFUSED`/`:RESET`/
`:DNS`/`:CLOSED`/`:POLICY-DENIED`/`:ADDR-IN-USE`/
`:ADDR-NOT-AVAILABLE`/`:OTHER` — extended onto `PORTS`' existing
`:OPERATION`/`:KIND`/`:NAME`/`:OS-ERROR` error shape for TCP-stream byte
operations too, so a timed-out `read-byte!` on a TCP port classifies the
same way a failed `TCP:CONNECT` does.

**TLS deferred.** The issue's TLS section (wrap a connected TCP port as a
TLS client/server port, certificate verification, ALPN/SNI) is out of
scope for this change: it forces a TLS-crate dependency decision that
belongs to the project owner, and this issue ships zero new dependencies.
The port-wrapping design (a TCP connection is an ordinary `Port`) is the
seam a TLS layer wraps later without changing the `PORTS`/`TCP` API.

## checker: literal-nil `if` joins stop committing to `(list a)` — the nil-on-miss guard idiom checks (#336)

The on-demand derived-scheme path (#308) was re-introducing the bias the
declared layer's honesty rule 1 exists to prevent: deriving `parse-integer`
(body `(if hit n nil)`) committed the `if` join to the nil side, producing
`(-> (a) (list b))`, and the standard guard idiom downstream —
`(let ((n (parse-integer s))) (if (numberp n) n 10))` — was then REJECTED
with "`if` branches disagree: List(Var) vs Int64". Now a literal-`nil`
branch meeting a non-list branch (ground scalar or still-free variable)
degrades the `if` to `any` (gradual, matching `when`/`unless`) instead of
erroring or pinning a free variable to "list of something"; nil-vs-list
still unifies as a list, and two non-nil branches that genuinely disagree
still error. A companion fix keeps self-recursive nil-on-miss helpers
honest: when a body's final type degrades to `any` but a recursive call
already concretized the assumed return variable mid-elaboration,
`check_callee` forces the assumption back to `any` before generalizing
(this also newly CHECKS `$require-resolve-disk`, previously a TYPE-ERROR).
`cond` joins are deliberately unchanged (see `elab_cond`'s comment; the
`cond` analogue needs a nil-on-miss-aware recursion assumption — a 0.4
direction the ticket itself names). Docs: the join rule is described in
docs/typed-checker-design.md.

## stdlib(io): format's directive set grows, plus READ-LINE, WITH-OUTPUT-TO-STRING, and s-expression file round-trip (#150)

Closes out the I/O & formatting ticket that #255's PORTS module left
partially done. `format` (`lib/18-format.lisp`) grows from `~a ~s ~d ~% ~~`
to `~a ~s ~d ~f ~x ~o ~b ~c ~% ~& ~~ ~{...~} ~^`: `~f`/`~,<n>f` fixed-point
floats, `~x`/`~o`/`~b` integer radix, `~c` bare-character rendering, `~&`
fresh-line (scoped to the current `format` call's own output — nothing in
the language tracks a destination's column across calls), `~{...~}` list
iteration with the `~a~^, `-style early-stop separator idiom named as
optional in the original ticket. **An unrecognized directive, or a
supported one written with an unsupported numeric/colon/at-sign prefix
(`~3a`, `~:d`), is now a hard error naming it, not the old silent
pass-through** — the larger directive set made a typo too easy to miss;
see `docs/cl-divergences.md`. `format`'s destination also now accepts a
`PORTS` port (writes the UTF-8 bytes to it), on top of the existing `nil`
(string) and `t` (stdout).

New on top of `PORTS` (#255), each lazily `(require 'ports)`ing on first
use so an environment that never touches I/O never pays for it: `read-line`
(`&optional port`, defaulting to the process's stdin under the `IO`
capability) and `with-output-to-string` (capture writes to a fresh
in-memory port as a string, always closing the port, even on error).
`read-sexpr-file`/`write-sexpr-file` round-trip a list of s-expressions
through a file on top of the existing whole-file `read-file`/`write-file`
builtins and the existing `read-string` reader builtin — no new Rust
kernel surface for any of this ticket. `format-build`'s control-string
walk and the new `~{...~}` iteration helper both stay tail-recursive
(stack-safe past the #361 10,000-frame trap for both a long control
string and a long iteration list).

## stdlib(data): optional JSON, URL, Base64, hex, and MIME codec modules (#257)

Five new optional embedded libraries for ordinary application and HTTP
programming, following the `TEXT`/`PORTS` namespace ruling from epic
#253: `BASE64` (`lib/32-base64.lisp`), `HEX` (`lib/33-hex.lisp`), `URL`
(`lib/34-url.lisp`), `JSON` (`lib/35-json.lisp`), and `MIME`
(`lib/36-mime.lisp`) — `(require 'name)` or `(import name)`, independently
of each other. All five are pure data transforms: **no capability is
required**, and every operation works inside a sandbox with every
capability denied. Every module is 100% Lisp — no new Rust kernel
builtins were needed; JSON parsing leans on the existing `STRING->NUMBER`
kernel primitive for exact int64-vs-float number classification and
IEEE-754-faithful float lexing, and every codec's per-character/per-byte
scan is written tail-recursively over UTF-8 byte codes (via
`TEXT:STRING->UTF8` + `ARRAY->LIST`, both native and O(n)) rather than the
Prelude's `STRING->LIST`, which recurses once per character and is not
stack-safe past a few thousand characters — a general fix for any large
flat input, not just JSON's own explicit `:MAX-DEPTH` nesting guard.

`JSON:PARSE`/`STRINGIFY`: object↔hash table (`String` keys, last-key-wins
on duplicates), array↔`Array` (not a list — the ticket's mapping, not a
free choice), `true`/`false`/`null`↔`T`/`NIL`/the keyword `:NULL` (three
mutually distinct values — `:NULL` avoids the `NIL`-is-both-false-and-
empty-list pun), integer literals in `i64` range as exact `Number`s,
literals outside that range controlled by `:ON-INTEGER-OVERFLOW` (`:ERROR`
default, or `:FLOAT` to widen instead of erroring), every other number as
`Float`. `STRINGIFY` always writes a `.` in `Float` output (even for a
whole value like `2.0`) so a `Float` never silently round-trips back as an
integer `Number`; `NaN`/infinite floats are a `STRINGIFY` error, not a
silent approximation. Strict throughout: rejects trailing garbage,
unescaped control characters, leading zeros, and unpaired `\u` surrogate
escapes, with every error carrying a line/column position;
`:MAX-DEPTH` (default 512) bounds nesting so deep input is a clean error
instead of a native stack overflow. `:PRETTY`/`:INDENT` control compact
vs. indented serialization.

`BASE64:ENCODE`/`DECODE` and `HEX:ENCODE`/`DECODE`: `Array<Char>`
bytes↔ASCII `String` (a byte is a `Char` or an integer 0-255, per the
epic's byte-array convention), every one of the 256 byte values
round-trips exactly in every position. Base64 supports `:ALPHABET`
(`:STANDARD` RFC 4648 §4 `+/`, or `:URL` RFC 4648 §5 `-_`) and `:PAD`
(default `T`) independently; Hex supports predictable-case `:CASE`
(`:LOWER` default, `:UPPER`) on encode and is case-insensitive on decode.
Both decoders are strict: invalid characters, misplaced/wrong-count
padding, and length/padding-policy mismatches are named errors.

`URL:ENCODE-PATH-SEGMENT`/`ENCODE-QUERY-COMPONENT` use different
safe-character sets (conflating path-segment and query-component
percent-encoding is a real bug class); `DECODE` is context-free (one
decoder for both, `:LOSSY` mirrors `TEXT:UTF8->STRING`/`-LOSSY`).
`URL:PARSE`/`BUILD` split/rebuild `SCHEME`/`USERINFO`/`HOST`/`PORT`
(handles a bracketed IPv6 literal)/`PATH`/`QUERY`/`FRAGMENT` (the latter
three returned raw, never auto-decoded, avoiding double-decode ambiguity)
via a small explicit state machine — no regular expressions.
`URL:PARSE-QUERY`/`BUILD-QUERY` preserve repeated keys and ordering as a
list of conses, never collapsed into a hash table.

`MIME:HEADERS-*`: a header list is `(name . value)` conses in original
order with original case preserved — deliberately not a hash table, so
`HEADERS-GET-ALL` never collapses a repeated header like `Set-Cookie`
(`HEADERS-GET` returns only the first match; `HEADERS-ADD`/`-SET`/
`-REMOVE`/`-NAMES` round out the API). `MIME:PARSE-CONTENT-TYPE`/
`BUILD-CONTENT-TYPE` handle Content-Type parameters including
quoted-string values with backslash escapes.

New manual chapter: [12. Codecs](docs/manual/12-codecs.md).

## runtime(io): binary ports and deterministic ownership for host resources (#255)

Lamedh now has a synchronous binary `Port` abstraction for byte streams —
files, in-memory byte buffers, and the process's standard streams — living
in a new optional embedded library, the `PORTS` module
(`lib/31-ports.lisp`, `(require 'ports)` or `(import ports)`), following
the namespace ruling from epic #253 and mirroring how `TEXT` (#254) wraps
its kernel primitives. Representation: a new opaque `LispVal::Port`
variant (`PortObj` in `src/lib.rs`) backed by a Rust enum over
`fs::File`/`Cursor<Vec<u8>>`/`Vec<u8>`/the standard streams/a
host-registered `Read`/`Write`, compared by identity like
`Array`/`HashTable`/`Environment`; the kernel `PORT-*` substrate
primitives live in `src/evaluator/builtins_ports.rs`.

Construction: `ports:open-input`/`open-output`/`open-append` (files),
`ports:open-input-bytes`/`open-output-bytes` + `output-contents`
(in-memory byte buffers), `ports:stdin`/`stdout`/`stderr` (the process's
standard streams). Binary operations work uniformly across every port
kind: `read-byte!`/`read-bytes!` (EOF is `NIL` from `read-byte!`, a
possibly-empty `Array<Char>` — never `NIL` — from `read-bytes!`),
`write-byte!`/`write-bytes!` (returns the count actually written, so
partial writes are observable), `flush!`, `close!` (idempotent — a
second close is a silent no-op), `open-p`/`input-p`/`output-p`/
`seekable-p`, and `position`/`seek!` on seekable ports (files and
byte-array input ports; byte-array output ports and the standard streams
are not seekable and signal a structured error on `position`/`seek!`).
`ports:position` is qualified-only — deliberately left out of the export
list so `(import ports)` never shadows the Prelude's flat
`(position item lst)` list helper.
Text wrappers (`read-line!`, `read-string!`, `write-string!`,
`read-all-bytes!`) are thin, explicit layers over `TEXT`'s UTF-8
boundary (#254) — bytes never implicitly become text. `with-open-port`
closes its port on every exit from its body — normal return, an ordinary
error, `THROW`, `RETURN-FROM`, or `GO` — via `UNWIND-PROTECT`; Rust's
ordinary `Drop` (the file closes when the last reference to the port
value is dropped) is a last-resort safety net, not the documented
contract.

Capability model unchanged from the existing filesystem builtins: opening
a file port for reading needs `READ-FS`, for writing/appending needs
`CREATE-FS`, `(ports:stdin)` needs `IO`; `stdout`/`stderr` and in-memory
ports need no capability (matching `PRINC`/`PRIN1` already writing to
stdout unconditionally). Acquisition is gated by `require_read_fs`/
`require_create_fs`/`require_io` exactly like `read-file`/`write-file`,
so `WITH-CAPABILITIES`'s dynamic-extent fence attenuation (#320/#325)
reaches port construction the same way it reaches every other host
builtin — verified directly: a port-open call inside
`(with-capabilities () ...)` fails even when the host/CLI granted the
capability. I/O errors are structured `LispVal::Error` values whose
`data` is an alist of `:operation`/`:kind`/`:name`/`:os-error`, not just
an English string.

Embedding: `LispVal::wrap_reader`/`wrap_writer` let a host hand Lisp code
an arbitrary `Read`/`Write` stream as a port (a pipe, a decompressor, a
captured buffer, ...) without ever exposing a raw file descriptor to
Lisp — see `docs/embedding.md`.

## runtime(modules): REQUIRE/PROVIDE and the Prelude/optional-library split (#256)

Lamedh now has named, dependency-aware, load-once library units —
`(require 'name)` / `(provide 'name)` — layered on the existing global
symbol table with zero new symbol-identity machinery (no Common Lisp
packages, no `pkg:symbol` reader syntax, no import renaming, no enforced
privacy; see `lib/06-require.lisp`'s header for the full design). This is
a *loading* discipline, distinct from and composable with `defmodule`'s
existing *naming* discipline (`lib/27-modules.lisp`, docs/manual/10-modules.md
§10.7).

`require` resolves a module name through a per-environment registry in
order: host-registered sources (`env.register_module(name, source)`,
no capability required), sources embedded in the binary (no capability
required), then — only under `READ-FS` — files under host-configured disk
search paths (`env.add_module_search_path(path)`; Lisp can read but never
set this list, so a host constrains disk resolution without exposing that
authority to sandboxed code). A second `require` of an already-loaded
module is a documented no-op; a module whose source errors, or which
finishes without calling `(provide 'name)`, is *not* marked loaded (partial
top-level definitions are not rolled back — this was never a transaction).
A `require` for a module already mid-load (directly or transitively) is a
hard cycle error naming the full chain. `(require-reload 'name)` is the
explicit development escape hatch; ordinary `require` never silently
re-evaluates. `provide` takes an optional exports list (metadata only);
`require` warns on an unbound declared export or a cross-module export
collision (errors instead, given `*require-strict-exports*`).
`(loaded-modules)`, `(module-state 'name)`, and `(module-info 'name)`
introspect the registry.

New `Environment::with_prelude()` loads only the stable general-purpose
vocabulary (core forms, lists, math, control flow, functional/string/
sets-hash/conditions/array helpers, `format`, `setf`/CL-compat, and
`require`/`provide` itself — see `src/lib.rs`'s crate doc for the exact
file list) — lighter and faster-starting than `Environment::with_stdlib()`,
which remains fully source- and behavior-compatible: it is now defined as
the Prelude plus every previously-unconditional optional library (shell,
Lisp 1.5 compatibility, testing, the optimizer, call-graph analysis,
condensation, guard fences, pattern matching, the rulebook, variants,
instrumentation, `defmodule` itself, the type table, protocols, the
`TEXT` module, and the help system — including 20-condensation.lisp,
closing the epic #253 acceptance criterion that it be loadable as an
embedded optional module rather than only via `-i lib/`), loaded in the
same order as before and immediately marked REQUIRE-loaded so a later
`(require 'name)` against a `with_stdlib()` environment is a correct
no-op rather than a redundant re-evaluation. Embedder API additions:
`Environment::with_prelude`, `Environment::register_module`,
`Environment::add_module_search_path`/`clear_module_search_paths`,
`lamedh::require_module`, `lamedh::loaded_modules`.

## stdlib(text): complete String API and the explicit UTF-8 Array<Char> boundary (#254)

`lib/14-strings.lisp` (the flat Prelude string surface from #147) is now
a complete String API: construction/access (`make-string`,
`string-concat`, `string-empty-p`, bounds-checked `char-at`), the full
case-sensitive comparison family (`string<`, `string>`, `string<=`,
`string>=`, `string-ne`, alongside the existing `string=`/`string-lessp`)
and a new Unicode-aware, locale-independent case-insensitive family
(`string-ci=`, `string-ci<`, `string-ci-ne`, ...), reverse search
(`string-last-index-of`), occurrence counting (`string-count`),
`string-replace-first`/`string-replace-all`, one-argument
`string-trim-left`/`string-trim-right`, `string-capitalize`, and
`string-reverse`. `string-lessp`'s pre-existing case-sensitive (not CL's
case-insensitive) meaning is unchanged and undisturbed by the new
`-ci` family. New Rust kernel primitive `string-casefold*` backs the
case-insensitive comparisons via Rust's locale-independent Unicode case
fold.

New `TEXT` module (`lib/30-text.lisp`, `(defmodule text ...)` /
`with-module`, 0.3's module story) carries the explicit, non-coercive
String <-> UTF-8 `Array<Char>` boundary: `(text:string->utf8 s)`,
`(text:utf8->string bytes)` (strict — signals a descriptive error naming
the offending byte offset on invalid UTF-8), and
`(text:utf8->string-lossy bytes)` (U+FFFD substitution). `Char` stays
exactly a `u8` byte and `Array<Char>` stays the ordinary dynamic array
already used by the #137 typed/JIT `(array char)` island — no new value
representation. New optional-library surface is namespaced per the
epic #253 ruling rather than growing the flat Prelude; existing #147
names are untouched.

## variant-case composes with the checker (#350)

The checker has a native eliminator rule for `variant-case`: the
scrutinee unifies with the clause constructors' owning variant, clause
vars bind positionally to the constructor's field types, and every
clause body joins to one result type. Before this, clauses like
`(circ (r) (* r r))` were misread as constructor applications, so every
variant-consuming `defun` carried a false `TYPE-ERROR` and was stuck on
the dynamic tier — the two flagship 0.3 features (sums and the checker)
didn't compose. Now `(defun area (x) (variant-case x (circ (r) (* r r))
(sq (s) (* s s))))` checks as `(-> (SHAPE) INT64)`, parametric variants
generalize (`(defun opt-len (o) (variant-case o (some (v) 1) (none ()
0)))` : `∀a. (option a) → int64`), wrong-variant scrutinees are rejected
at call sites, and binder-count/clause-body errors are real errors.
Exhaustiveness stays a runtime concern (the vau names missing brands).

## Docs: divergences from Common Lisp

New one-pager `docs/cl-divergences.md` — the cheat-sheet for readers
with CL reflexes, every claim probe-verified: Lisp-1 namespace, wrapping
64-bit integers (no bignums/rationals), `'a'` char literals, pure
`sort`/`rplaca`, collection-first hash accessors, the absent/replacement
table (loop→dotimes/while, CLOS→protocols, values→lists,
eql→eq/equal, …), same-word-different-behavior notes (`defvar`,
`format`'s exact directive set, `case` defaults), and the list of CL
reflexes that just work.

## Records — one definition form (breaking)

```lisp
(defrecord npc
  (tag symbol) (hp int64) (loot (list string))
  (:invariant (>= hp 0))
  (:derive equality lens))
```

- **`defrecord` is the only way to define a record**, and it does
  everything `defconcept`/`defstruct`/`defrecord` did together: branded,
  checker-denotable, NOMINAL, row-subsumable types over one runtime
  representation (StructObj), tier-dispatched — all-native fields compile,
  anything else runs dynamic with the same surface
  (`(record-compiled-p 'name)` reports the tier). Generates `make-Name`,
  `Name-p`, `Name-field`, `validate-Name`; optional `(:invariant ...)` and
  `(:derive equality|printer|lens ...)` sections; bare field names mean
  `(field any)`; the condensation trace (`condense-trace`, `edit!`,
  `deflaw`, `example`, fingerprints) works on every record.
- **Removed: `defconcept`** — use `defrecord`; a legacy
  `(:fields ((f ty)...))` section is still accepted to ease migration.
- **Removed: untyped `defstruct`** — with its keyword constructor protocol
  and `set-NAME-FIELD!` mutators. Records are values: update with
  `record-with`. `setf` accessor places still expand to
  `(set-<accessor>! obj v)` against user-defined mutators.
- **Removed: the positional record representation** (`(BRAND f1 ...)`
  lists). `defstruct-typed` remains only as internal machinery behind the
  compiled tier.
- `record-ref` / `record-with`: by-name field read and functional update
  with checker-native row rules — `(defun worth (x) (record-ref x 'value))`
  derives `(forall (a b) (-> ((record ((value a)) b)) a))` with no axioms.
- Records print as `#S(BRAND v ...)` and the reader accepts `#S(...)`
  literals: print/read round-trip (spawn and channel serialization), usable
  as source syntax.

## Sequence protocols: map and for-each

- `(map coll fn)` — kind-preserving map as a protocol (list → list,
  array → array, string → string); `(for-each coll fn)` visits for
  effect over list/array/string/hash, with the hash instance receiving
  `(fn key value)`. Collection FIRST (protocols dispatch on their first
  argument — and it matches the container convention). Extend either
  with `definstance`.
- Ruled: `filter`/`reduce` stay list-specific and fn-first (the
  `mapcar`/`every` heritage class); protocols dispatch on the first
  argument, so protocolizing them would flip their argument order.
  Convert at the edge (`array->list`, `string->list`) for other kinds.
- **Breaking**: the Lisp 1.5 appendix's `map` (apply f to successive
  TAILS, return nil) is renamed **`map-tails`** — the bare name now means
  what every modern reader expects. Documented appendix deviation.

## Stdlib staples

Gap-probe additions (each follows its class's argument order):

- `(sort-by lst keyfn [pred])` — sort by extracted key, collection first
  like `sort`; `pred` (default `#'<`) compares keys.
- `(enumerate lst [start])` — index/element pairs, `zip` shape.
- `(frequencies lst)` — `(element . count)` alist, first-seen order;
  typed `(forall (a) (-> ((list a)) (list (pair a int64))))`.
- `(string-pad-left s width [pad])` / `string-pad-right` /
  `(string-repeat s n)` — padding never truncates.

## One dispatch system (breaking)

- **Removed: `definterface`, `implements?`, `method`, `method-symbol`**
  and the whole lib/21-interfaces layer (row-aware signature unifier,
  fingerprinted claims). Two dispatch mechanisms was one too many:
  `defprotocol`/`definstance` is the survivor.
- The conformance value lives on, re-seated on protocols:
  `(implements! type protocol...)` asserts that TYPE carries a clean
  instance of every named protocol (graded INSTANCE / MISMATCH — the
  implementation's checker verdict is a type error / MISSING), and
  `implements-p` is the predicate form. A "contract" is a set of
  protocol names.
- `examples/npcs.lisp` and `examples/oo-patterns.lisp` rewritten to
  protocols; identical output, less machinery.

## HOF protocols are function-first (breaking)

- Ruled: higher-order functions follow the CL convention — FUNCTION
  FIRST. `map` and `for-each` flip to `(map fn coll)` /
  `(for-each fn coll)`; `option-then` and `result-then` flip to
  `(option-then f o)` / `(result-then f r)`, matching
  `option-map`/`result-map`. Access operations (`ref`, `put!`, `copy`,
  `sort-by`) stay collection-first, matching CL's `aref`/`elt`/`sort`.
- Protocols now declare their dispatch position:
  `(defprotocol map "..." (:dispatch 1))` dispatches on the second
  argument (kernel: `declare-protocol-dispatch!`; the checker selects
  instances by the dispatch argument's shape).
- `filter` is now a generic kind-preserving protocol — it was already
  fn-first, so nothing breaks: lists as before, plus arrays and strings
  (`(filter #'alpha-p "a1b2c")` is `"abc"`).

## Invariants are enforced at construction (breaking)

- `make-Name` now REFUSES a value the `:invariant` would fail
  (`MAKE-ACCT: invariant violated`) instead of constructing it; the
  wrapper captures the validator at definition time, so later rebinding
  `validate-Name` cannot weaken construction. `validate-Name` remains
  the judgment for the two roads that bypass the constructor door:
  `record-with` updates and `#S` reader literals — validate explicitly
  after either.

## flatten respects dotted pairs (breaking)

- `flatten` now flattens nested PROPER lists only; a dotted pair is a
  leaf: `(flatten '((1 . 2) (3 4)))` is `((1 . 2) 3 4)`. The old
  behavior recursed into every cons and silently destroyed
  alist/coordinate-shaped data (it turned the classics' game-of-life
  world into bare integers). New `proper-list-p` predicate, typed.

## Dotted parameter tails

- `(lambda (a . more) ...)`, `(defun f (a . more) ...)`, and
  `(defmacro m (test . body) ...)` now accept the classic dotted-tail
  shorthand as `&rest` (previously an inscrutable `list_to_vec` error).
  Mixing both spellings errors; fexprs keep their fixed `(args env)`
  shape with a message that says so.

## random is a PRNG now

- `random` previously returned `nanos-since-epoch mod n` on EVERY call —
  a monotonic wall-clock ramp, not a random sequence (found when the
  monte-carlo-pi example converged to 2.57). It is now SplitMix64 over
  persistent thread-local state, lazily time-seeded; new
  `(random-seed! n)` makes runs reproducible.

## The substrate star (breaking)

The monomorphic per-type implementations behind the protocols now carry
a trailing `*`, in the old Lisp tradition of marking a name as visibly
outside the normative vocabulary: `string-length*`, `array-length*`,
`hash-table-count*`, `list-length*`, `array-map*`, `copy-list*`,
`array-copy*`, `copy-hash*`, `array-fetch*`, `array-store*`. The
unstarred names are REMOVED. Write against the protocol names
(`length`, `map`, `copy`, `ref`, `put!`); call a starred form when
you've committed to the type and want the direct monomorphic call
(until 0.4 splices instances at call sites, that's the hot path).
Unstarred survivors are not substrate: converters (`array->list`,
`char->code`, ...), true type-specifics (`string-split`, `array-fill`,
...), and the separately-ruled lenient reads (`gethash`, `nth`, `elt`).

## Access protocols: ref, put!, copy

One vocabulary over the per-type access zoo, all collection first:

- `(ref coll k)` — strict read at an index/key/field: lists, arrays,
  strings, hash tables, and records (by brand, via the fallback).
  **Absence is an ERROR**, which is what lets every instance carry an
  honest result type (`(check-type (ref (list 1 2) 0))` is `int64`);
  the lenient nil-on-miss reads keep their old names (`gethash`, `nth`,
  `elt`). A known type with no instance is a static error.
- `(put! coll k v)` — write, returns `v`: arrays and hash tables (the
  mutable containers; records are values — `record-with`).
- `(copy x)` — fresh list/array/hash (**`copy-hash` was missing
  entirely and is new**), identity on immutable strings, Lisp 1.5
  structure-copy fallback for atoms.
- The type-prefixed names (`fetch`, `store`, `array-copy`, ...) remain
  as the monomorphic substrate the instances dispatch to and compile
  through. Ruled: `char-code` (strict kernel) vs `char->code` (coercing
  wrapper) are not duplicates; both stay.

## Dotted pairs in the checker

- `(cons 'k 2)` now types as `(pair symbol int64)` instead of erroring:
  a cons whose tail is a known non-list ground type takes the
  dotted-pair view, and `car`/`cdr` project known pairs. Unknown tails
  keep the list-cons view (the recursion default), so existing derived
  schemes are unchanged. The alist-cell idiom types end to end.
- New example: `examples/wordcount.lisp` — the classic word-frequency
  report, dogfooding the 0.3 staples (`frequencies`, `sort-by`,
  `enumerate`, padding, `for-each`, Option, `variant-case`). Found both
  the missing pair rule and the staples gaps.

## Typed protocols

```lisp
(defprotocol volume "loudness of a thing")
(definstance volume ((n int64)) int64 (* n 2))
(definstance volume ((s string)) int64 (string-length s))
```

- One name, many typed instances, three resolutions: the CHECKER selects
  the instance whose shape matches the first argument's inferred type and
  gives its precise scheme (a known type with no instance is a static
  error — "no `volume` instance for (list int64)"); the RUNTIME
  dispatches on the value's kind (list/string/array/hash/scalars, record
  brands, variants); the COMPILER treats each instance body as an
  ordinary defun, so eligible instances go native through the one-door
  pipeline. When every instance agrees on one ground result type, even a
  gradual call site derives it: `(defun n-items (x) (length x))` checks
  as `(forall (a) (-> (a) int64))`.
- `defprotocol` captures any prior binding as the fallback instance, so
  protocolizing a builtin is seamless. **`length` is the shipped pilot**:
  lists/strings/arrays/hash tables out of the box, and your own types
  join with one `definstance`.
- Instance implementations live under reader-unnameable hidden names —
  they cannot be shadowed or called around the dispatcher from source.

## The census, batches 2–3: one name, and the type table

- **`delete-key`/`delete-key-bang` removed; `remhash` is the one hash
  removal name** (collection first, kernel-direct).
- **`length` covers every sized collection**: lists, strings, arrays, and
  now hash tables.
- **The type table** (`lib/28-types.lisp`): verified declared schemes for
  ~45 builtins and stdlib functions — predicates
  `(forall (a) (-> (a) bool))`, integer-only predicates strict both ways,
  list functions (`member`/`filter`/`mapc`/`every`/`exists`/`notany`),
  strings and conversions (`string-upcase`, `princ-to-string : a →
  string`, `intern`, `implode`...), math with known results (`sqrt → 
  float64`, `floor → int64`). Two honesty rules exclude entries: NIL-ON-
  MISS functions never claim a result type (`nth`, `assoc`,
  `string->number` stay gradual — a declared "hit" type would let checked
  code consume a legal ()), and variadic/multi-arity functions can't carry
  fixed-arity schemes (they get kernel checker rules instead). Net effect:
  string/list/math pipelines derive full schemes with zero annotations,
  and misuse errors at the call site.

## The census, batch 1: variadicity (breaking)

The full special-form/builtin/stdlib census lives in docs/audit-0-3.md;
this batch fixes the variadicity findings.

- `append` is a variadic kernel builtin — `(append)`, `(append xs)`,
  `(append a b c ...)`, dotted final tail preserved — replacing the 2-ary
  stdlib recursion (it is also faster and checker-known now).
- `gcd`/`lcm` are variadic folds with the CL identities (`(gcd)` = 0,
  `(lcm)` = 1).
- **`logor` is renamed `logior`** — the name every Lisp reader (and
  model) expects; `logand`/`logxor` were already variadic and now the
  trio is uniform.
- `mapcar` takes N lists Common-Lisp style, zipping and stopping at the
  shortest.
- Checker-native rules for the variadic family: `append` (all `(list a)`
  → `(list a)`; dotted-tail append stays dynamic-only), `concat`
  (strings → string), `min`/`max` (numeric chain), `logand`/`logior`/
  `logxor`/`gcd`/`lcm` (int64). Side effect of the `concat` rule: the NPC
  example's greet methods left the dynamic frontier — their row schemes
  now derive fully.

## HM generics — parametric records and variants

```lisp
(defrecord (duo a b) (first a) (second b))
(defvariant (option a) (some (value a)) (none))
(defrecord (node a) (val a) (next (option (node a))))
```

- Records and variants take TYPE PARAMETERS, as proper HM type
  application: `make-duo : (forall (a b) (-> (a b) (duo a b)))`,
  instantiated freshly at every use — `(duo-first (make-duo 1 "s"))` is
  `int64`, `(unwrap-or (some "s") 0)` is a static error, `(none)`
  instantiates freely like nil. Nominal by name with pairwise argument
  unification; constructor applications absorb into their variant's
  application; SIBLING constructors of one variant unify (an `if`
  building `(some x)` / `(none)` types cleanly); parametric record
  applications subsume into rows with instantiated field types.
- **Recursion composes**: `(node a)` referencing `(option (node a))`
  gives `node-next : (forall (a) (-> ((node a)) (option (node a))))`.
- **Option and Result are parametric now** with precise helper schemes
  (`unwrap-or : (forall (a) (-> ((option a) a) a))`, monadic
  `option-then`/`result-then`, `result : (result a e)`).
- A BARE generic name in a type is sugar for the all-`any` application
  (`option` ≡ `(option any)`) — the gradual reading, and what
  pre-parametric code already meant.
- Erased at runtime: every value is the same branded StructObj; generics
  never compile (checker-only), monomorphic records keep their tiers.
- Built-in type-constructor names (`pair`, `list`, `array`, `record`,
  scalars) are REJECTED as record/variant names — they'd silently shadow
  the built-in meaning in type surfaces.

## Sum types

```lisp
(defvariant shape
  (circle (r int64))
  (rect   (w int64) (h int64)))
```

- **`defvariant`**: a closed set of branded record constructors (bare
  constructor names — `(circle 3)`) plus a checker-level union type. A
  `CIRCLE` unifies where a `SHAPE` is demanded; a constructor of another
  variant is rejected by name ("HEADS is not a constructor of variant
  SHAPE").
- **`variant-case` is exhaustive**: missing a constructor without an
  `else` clause errors, naming the missing brands.
- **`match` destructures records/constructors with `#S` patterns**:
  `(match v (#S(CIRCLE ?r) ...) ...)`, nesting and all.
- **Option and Result** are ordinary variants in the stdlib
  (`some`/`none`, `ok`/`err`) with `unwrap`/`unwrap-or`/`option-map`/
  `option-then`/`result-or`/`result-map`/`result-then`/`option-of`, and
  `try-call` bridging the condition system into Result.
- Breaking: the `some` list quantifier in `lib/13` is renamed **`exists`**
  (`(exists #'evenp xs)`); `some` is Option's constructor now.
- `record-new` accepts zero field values (nullary constructors like
  `(none)`).

## Recursive records

- Self- and mutually-referential `defrecord` field types are NOMINAL now,
  not a silent `any`: `(defrecord node (val int64) (next node))` gives
  `node-next : (-> (NODE) NODE)`. Struct unification is by brand name;
  struct-into-row expansion re-resolves the definition through the
  registry; forward references get provisional definitions (a misspelled
  field type surfaces as a phantom brand at first unification instead of
  degrading silently). The blessed terminator idiom is Option:
  `(next option)` with `(none)`.

## Checker and rows

- Row types (Rémy-style, with a gradual `Any` frontier) ported into the
  checker; typed structs subsume into record rows; `declare-type!` declared
  schemes consulted at call sites; `see-type` reports
  TYPED/CHECKED/DECLARED/TYPE-ERROR/DYNAMIC verdicts as data.
- **Derived schemes at call sites**: the checker checks unknown lambda
  callees on demand (memoized, recursion-safe), so row types flow through
  helper functions with zero annotations.
- **One door**: plain `defun` quietly attempts typed compilation; opt out
  with `(declare (no-compile))` or `declaim`. `defun-typed`/`defun*` remain
  for explicit signatures/inference.

## Stack traces

- Runtime errors carry a **backtrace of named frames**: the toplevel prints
  `Error: boom` followed by `in: INNER ← MIDDLE ← OUTER` (innermost first),
  and handlers read the frames of the error they caught via
  `(last-backtrace)`. Tail calls collapse into one frame (Scheme-style TCO
  traces — a 100k-deep tail loop is a single entry). Recording is
  pay-mostly-on-error: overhead on hot benchmarks is within noise. Direct
  toplevel errors format exactly as before. Host API:
  `lamedh::format_error_with_backtrace`.

## Dynamic-extent guard fences (breaking)

- `with-fuel` and `with-capabilities` are KERNEL SPECIAL FORMS now, with a
  thread-local capability mask and RAII save/restore. Attenuation follows
  the CALL, not the fence's lexical body: **helpers called from inside a
  fence are fenced**, eval'd code is fenced, and kernel capability checks
  consult the mask on every gated operation. There is no Lisp-callable way
  to widen either state (`kernel-fuel-set!` is narrow-only inside a
  fence; no capability-mask setter exists).
- Escaped closures follow the same law in reverse: a closure created
  under a fence but called outside runs with the caller's authority —
  the semantics `spawn` always had, now uniform.
- Under an armed fuel budget, one-door native membranes take their
  interpreted fallback (compiled internal loops never returned to the
  metered trampoline — the fuel escape is closed); `jit-optimize` returns
  `COMPILE-DISABLED-BY-GUARD` and `defun-typed` errors while armed.
- The tick-instrumentation walker, lexical seal shadows, and eval
  re-sealing hatches are deleted from `lib/22` (~150 lines); the
  gated-builtin table remains for static `capabilities-needed` manifests.
- New read-only introspection: `(capability-mask-allows-p 'CAP)` — how
  custom (module-provided) capabilities attenuate through the same mask.

## Checker honesty

- Arithmetic and comparisons reject KNOWN non-numeric operands statically
  (`(+ "a" "b")` used to check as `string`); char arithmetic and char
  comparisons stay legal (evaluator parity), variables and `any` stay
  gradual — no scheme changes. Char literals now check as `char`.

## Modules

```lisp
(defmodule geometry (:export area) (:provides FAST-MATH))
(with-module geometry
  (defun helper (x) (* x 3))
  (defun area (r) (helper (* r r))))
(geometry:area 2)   ; => 12
(import geometry)   ; binds AREA
```

- A module is a NAMING DISCIPLINE plus metadata over the flat global
  namespace: `with-module` stores definitions as `MODULE:SYMBOL` (the
  reader now accepts `:` as a non-initial symbol constituent) and
  qualifies module-local references; `import` binds a module's exports
  globally (snapshot semantics); `module-of`/`module-functions`/
  `module-exports`/`module-requires`/`module-provides` introspect.
- **Modules can provide capabilities** — conservatively: a `(:provides
  CAP)` clause registers a NEW capability name into the attenuable
  vocabulary. It is held by registration at the outermost level, gates
  only explicit `(require-capability 'CAP)` checks, attenuates through
  `with-capabilities`/`sandboxed`/`spawn` like a built-in, and can never
  grant kernel abilities (READ-FS and friends stay host-granted). The
  fence now shadows `require-capability` so the gate attenuates with it.
- `(:requires CAP...)` records a module's needs for introspection and
  manifests.

## Regularity (breaking, deliberately)

One convention where there were several; the breaks are the point.

- **Hash operations are COLLECTION FIRST, one order**: `(gethash table
  key)`, `(remhash table key)` — matching `sethash`/`fetch`/`store`/
  `getp`. The either-order type-guessing (#246) is removed.
- **Comparisons are variadic monotone chains** like `+`/`*`: `(< a b c)`,
  `(= x y z)`, `(<= ...)`, `(>= ...)`.
- **`defun` supports `&optional` and `&key`** (with defaults; later
  defaults see earlier parameters; composes with `&rest`). Expanded at the
  defun layer to a variadic lambda + `LET*` prologue, so such functions
  stay on the dynamic tier. Bare `lambda`/`defmacro` still take only
  `&rest`.
- **`(set sym val)`**: the value-level global setter (both arguments
  evaluated) — the computed-symbol twin of the quoting `cset` macro.
- **H-suffix hex literals must start with a decimal digit** (`0FFh`, not
  `FFh`) — the assembly convention — so `ch`, `each`, `deadh` are ordinary
  symbols again.
- **The LABEL variable-read hack is gone**: a list VALUE headed by the
  symbol `label` no longer auto-evaluates when read from a variable (it
  made `(list 'label ...)` data explode and reserved `label` as a field/
  parameter name everywhere). The `label` special form in operator
  position is unchanged.
- **`-s` is repeatable** on the CLI; each string evaluates in order in one
  shared environment.

## explain-compile

- `(explain-compile 'f)` reports the execution tier as data —
  `((TIER . COMPILED) (SIGNATURE ...))`, or `CHECKED` with the SCHEME and
  the CONCRETE blocker keeping it off the native tier (ambiguous operand
  types, non-storable list/record schemes, a `(declare (no-compile))`
  pin), or `DYNAMIC`/`TYPE-ERROR`. Side-effect-free (a dry-run twin of the
  codegen path — explaining never installs anything).

## Instrumentation: trace / time / step-count — and ONE fuel ruler

- `(trace 'f)` / `(untrace 'f)`: real call tracing (args in, value out,
  indented by depth) — replacing the Lisp 1.5 flag-only stubs. Natively
  compiled internals count as one call.
- `(time form)` prints wall milliseconds + kernel steps and returns the
  value; `(step-count form)` returns `(steps . value)`.
- **Breaking — one ruler**: `with-fuel N` is now denominated in KERNEL
  STEPS (one trampoline iteration each), the exact unit `step-count`
  measures: `(car (step-count form))` sizes the budget, tight to a handful
  of steps. The old unit (function entries, ×256 kernel backstop) is gone,
  along with the tick-instrumentation walker — the kernel counter is the
  single meter (the no-compile rewrites remain: in-fence definitions stay
  interpreted so native loops can't escape metering). `fuel-remaining`
  reads the kernel counter. Nested fences clamp to and spend from the
  enclosing remainder; fence setup charges the enclosing budget.
- `spawn`'s `:fuel` was already kernel steps; it now agrees with
  `with-fuel` instead of being 256× finer.
- New builtin: `(monotonic-micros)`.
- Soundness fix (found by TRACE's wrapper): a `&rest` closure created
  under an intervening `LET` read the let's slots instead of enclosing
  variables — `&rest` call frames now contribute an Opaque scope level so
  outer `LocalGet` depths count them.

## Guards, fuel, and processes

- Composable guard fences, pure Lisp: `with-fuel`, `with-capabilities`,
  `sandboxed`; guarded code is introspective; static capability manifests
  (`capabilities-needed`) computed from the call graph; a kernel
  step-budget backstop makes fuel exhaustion un-catchable-by-accident.
- `spawn` / `spawn*` / `await`: share-nothing interpreter threads whose
  capabilities are the requested set intersected with the caller's
  effective set — attenuation all the way down (#140). Channels and
  `clone-interpreter` behind the `concurrency` feature.

## Conditions

- Restarts: `restart-case`, `invoke-restart`, `find-restart`,
  `compute-restarts`, `handler-bind`, plus `use-value`/`store-value`/
  `retry`/`abort-to-restart`/`with-retry-restart`. (Documented deviation:
  handler-bind handlers run post-unwind.)

## Patterns and the rulebook

- Structural pattern language: `pat-match` (`?x`, `??segments`, guards),
  `match`, `destructuring-bind`, `sgrep`/`sgrep-source`/`sgrep-file`
  (positioned hits via `read-all-positioned`), bottom-up `rewrite`.
- The rulebook optimizer: `defrule`/`list-rules`/`apply-rules` — optimizer
  passes as pattern data feeding `optimize-form`.
- Go-style interfaces: `definterface`, `implements?`/`implements!` verify
  method sets against checker verdicts with a row-aware unifier.

## Performance

- Slot frames with routing tables: sound compile-time lexical addressing
  for lambda params and LET binders (#200 M3); unified compile/tree-walker
  trampolines; typed-JIT tail calls (self, mutual, and general); lambda
  bodies pre-compiled at definition time; per-call allocation cuts
  (SmallVec operands, symbol-id frames, precomputed special-form dispatch,
  cached symbol flags, lazy defun analysis hooks); shallow binding for
  dynamic variables; optional `arc-val` atomic refcounting feature;
  COLLAPSE-FRAMES and purity-checker optimizer passes over a DEFUN call
  graph.

## Correctness and parity (the #210 audit and friends)

- Typed tiers now match the evaluator exactly on: Euclidean `MOD`,
  `OVERFLOW`/div-by-zero flag propagation through the membrane (including
  `MIN%-1` and flag-before-error ordering), `FETCH`/`STORE` out-of-bounds
  errors, `CODE-CHAR`/`CHAR-CODE`, variadic `AND`/`OR`, strict binary
  `/`/`MOD` arity, mutated array parameters written back.
- Soundness: gensym symbols bind by their own id in lambda params and
  `SETQ`; dynamic bindings preserved across tail calls; cross-namespace
  symbol-id remapping in local frames; checked `gcd`/`lcm`; reachable JIT
  panics converted to Lisp errors; reader recursion bounded;
  `RENAME-FILE` requires `READ-FS` in addition to `CREATE-FS`.

## Reader, CLI, and docs

- Reader: 1-based positions in parse errors, nesting block comments
  `#| |#`, CL-style radix literals (`#x`/`#b`/`#o`), shebang support.
- REPL: persistent history (saved on `(exit)` too) and symbol tab
  completion.
- Self-evaluating keyword symbols; runtime error messages include the
  offending value; `cargo doc` warning-free; docs refreshed to match
  behavior; classic OO patterns example on row types (`examples/`).

# v0.2.0

efficiency.

The typed JIT release: HM-lite type inference (#135), the typed membrane
with a native Cranelift backend (#124), `defun-typed`/`defstruct-typed`,
typed regions, and the `jit` feature on by default.
