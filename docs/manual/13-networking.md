# 13. Networking

Chapter 11 covered synchronous binary I/O over files, memory buffers, and
standard streams. This chapter covers the same synchronous binary-port
model over the network: DNS resolution, TCP, and UDP — three optional
embedded libraries, `NET` (`lib/37-net.lisp`), `TCP` (`lib/38-tcp.lisp`),
and `UDP` (`lib/39-udp.lisp`), built entirely on `std::net` with zero new
crate dependencies. Pull them in with `(require 'net)`/`(require 'tcp)`/
`(require 'udp)` on a `with_prelude()`-style environment; `with_stdlib()`
environments (including the `lamedh` CLI) already have all three loaded.

**TLS** (`TLS`, `lib/43-tls.lisp`) wraps a connected TCP `Port` as a client
or server, behind the off-by-default `net-tls` cargo feature — see §13.8.

A connected TCP stream is an ordinary `PORTS` port (Chapter 11): every
`read-byte!`/`write-bytes!`/`close!`/`with-open-port`/`port-p` operation
already works on it. TCP listeners and UDP sockets are not byte streams —
binding/accepting/sending-to are their own operations — so they get their
own opaque handle instead, printed as `#<net:kind "name" open|closed>`.

## 13.1 Capabilities

Three ambient authorities, checked the same `with-capabilities`-attenuable
way as `READ-FS`/`CREATE-FS`/`IO` (Chapter 7):

| Capability | Gates |
|---|---|
| `NET-DNS` | Explicit hostname resolution: `net:resolve` |
| `NET-CONNECT` | Outbound connections: `tcp:connect`, `udp:connect!`, `udp:send-to` |
| `NET-LISTEN` | Binding/listening for inbound traffic: `tcp:listen`, `udp:bind` |

`udp:bind` needs `NET-LISTEN`, not `NET-CONNECT`, even for a socket you
only intend to send from: a bound UDP socket receives datagrams from any
sender, with no connection-acceptance step to gate separately the way TCP
has one — so binding is "inbound traffic" authority regardless of intent.

```console
$ target/debug/lamedh --sandbox -s "(net:resolve \"localhost\")"
Error: NET-DNS capability is not enabled (grant it via --capability NET-DNS or the host API)
  in: NET:RESOLVE
```

Each capability is independent — granting one does not unlock the
others — and every one is attenuated by `with-capabilities` fences exactly
like every other host builtin (Chapter 7 §7.1):

```console
$ target/debug/lamedh -s "(with-capabilities '() (tcp-connect* \"127.0.0.1\" 1 100))"
Error: capability denied: NET-CONNECT (attenuated by an enclosing fence)
```

Once a resource is acquired, continued use needs no further capability
check — reading/writing an open TCP port, or sending on an already-
`connect!`ed UDP socket, is unrestricted (Chapter 11's "an open handle is
authority to keep using it" rule, applied to networking).

Embedding hosts have one more lever Lisp code cannot reach: a policy
callback that scopes a granted capability to specific hosts/ports (e.g. so
a `NET-CONNECT` grant for an HTTP-client library is not unrestricted SSRF
authority). See `docs/embedding.md`'s "Networking policy" section —
there is no Lisp-facing API for it by design.

## 13.2 Addresses and DNS

`net:address` is a `defrecord` (Chapter 4) with three fields — `family`
(`:ipv4` or `:ipv6`), `ip` (a string, never bracketed), and `port` — so it
prints and compares like any other record:

```console
$ target/debug/lamedh -s "(net:resolve \"localhost\")"
(#S(NET:ADDRESS :IPV6 "::1" 0) #S(NET:ADDRESS :IPV4 "127.0.0.1" 0))
```

The exact order and address family mix depends on the host's resolver
configuration — do not depend on which comes first. `net:resolve` takes
an optional second argument (service port, default 0) and needs no live
network for `"localhost"`; it is answered locally on every platform this
was tested on.

```lisp
(net:address-family addr)   ; => :IPV4 or :IPV6
(net:address-ip addr)       ; => "127.0.0.1"
(net:address-port addr)     ; => 8080
(net:address->string addr)  ; => "127.0.0.1:8080", or "[::1]:8080" for IPv6
```

`net:local-addr` and `net:peer-addr` inspect an already-acquired resource
(a connected TCP port, a listener, or a UDP socket) and need no
capability of their own:

```console
$ target/debug/lamedh -s "(let ((l (tcp:listen \"127.0.0.1\" 0))) (net:address->string (tcp:local-addr l)))"
"127.0.0.1:35515"
```

(The port number is OS-assigned — port `0` on bind always is — so it will
differ on every run; only the shape is stable.)

A malformed or unresolvable host signals a structured `:DNS` error rather
than a bare string, so callers can dispatch on it (§13.5):

```console
$ target/debug/lamedh -s "(handler-case (net:resolve \"\") (error (e) (error-data e)))"
((:OPERATION . "resolve") (:CATEGORY . :DNS) (:HOST . "") (:PORT . 0) (:OS-ERROR . "failed to lookup address information: Name or service not known"))
```

(The exact `:OS-ERROR` text is platform-dependent; `:CATEGORY` is the
portable part to match on.)

## 13.3 TCP

`tcp:connect host port &optional timeout-ms` connects, returning an
ordinary duplex `PORTS` port. `tcp:listen host port &optional backlog`
binds and listens, returning a listener handle; `tcp:accept listener`
blocks for the next inbound connection and returns `(cons port
peer-address)`.

A single process can drive both sides over loopback because a completed
TCP handshake queues in the OS backlog even before the application calls
`accept` — `connect` returns as soon as the OS-level handshake finishes,
so `connect` then `accept` (in either order relative to each other, as
long as `listen` came first) never deadlocks on loopback:

```console
$ target/debug/lamedh -s "(progn
    (let* ((l (tcp:listen \"127.0.0.1\" 0))
           (port (net:address-port (tcp:local-addr l))))
      (let ((c (tcp:connect \"127.0.0.1\" port)))
        (let* ((pair (tcp:accept l))
               (s (car pair)))
          (ports:write-bytes! c (text:string->utf8 \"hi\"))
          (ports:flush! c)
          (let ((msg (text:utf8->string (ports:read-bytes! s 2))))
            (ports:close! c)
            (ports:close! s)
            (tcp:close-listener! l)
            msg)))))"
"hi"
```

A real client and server are of course two separate processes (or, in a
single embedding process, two OS threads — see `tests/test_net.rs` for
examples using a plain `std::net` peer on a spawned thread).

Other TCP-specific operations, none of which need a further capability
check once you hold the resource:

- `(tcp:shutdown! port how)` — `how` is `:read`, `:write`, or `:both`.
  Shuts down one or both directions without closing `port`; the peer sees
  EOF (read shutdown) or a reset (write shutdown while data is
  in flight) on the shut-down side, but `port` itself stays usable for
  the other direction and for `close!`.
- `(tcp:set-read-timeout! port ms)` / `(tcp:set-write-timeout! port ms)` —
  `ms` a positive integer, or `nil` to block without a timeout (the
  default). A timed-out read/write signals a structured `:TIMEOUT` error.
- `(tcp:close-listener! listener)` — idempotent, like `ports:close!`.
  Every subsequent `tcp:accept` on this listener errors immediately with
  a `:CLOSED` error; a concurrent `accept` already blocked on another OS
  thread is *not* guaranteed to unblock (plain `std::net::TcpListener`
  has no portable wakeup-on-close) — that immediate-rejection guarantee,
  not OS-level wakeup, is the documented close contract.
- `(tcp:listener-p x)` / `(tcp:listener-open-p listener)` — predicates.

`backlog` (the `tcp:listen` argument) is accepted for API completeness
with the OS-level connection-queue concept, but is currently **advisory
only**: `std::net::TcpListener` has no backlog-customization API without
an additional crate, and this release ships zero new dependencies.

## 13.4 UDP

`udp:bind host port` returns a socket handle (port `0` for an
OS-assigned ephemeral port). `udp:send-to socket host port bytes` sends
one datagram to an explicit destination; `udp:connect! socket host port`
sets a default peer so `udp:send socket bytes` can omit the address on
every call afterward. `udp:receive-from socket maxlen` blocks for one
datagram and returns a 3-element list `(bytes peer-address
possibly-truncated-p)`.

```console
$ target/debug/lamedh -s "(progn
    (let* ((a (udp:bind \"127.0.0.1\" 0))
           (b (udp:bind \"127.0.0.1\" 0))
           (a-port (net:address-port (udp:local-addr a))))
      (udp:send-to b \"127.0.0.1\" a-port (text:string->utf8 \"ping\"))
      (let ((result (udp:receive-from a 32)))
        (udp:close! a)
        (udp:close! b)
        (text:utf8->string (car result)))))"
"ping"
```

Datagram boundaries are preserved — two independent sends always arrive
as two independent `receive-from` calls, never coalesced, and never split
across calls. `possibly-truncated-p` is `T` exactly when the received
length equals `maxlen`: plain `std::net` exposes no `MSG_TRUNC` indicator
without raw syscalls (out of this release's no-new-dependency, no-ioctl
scope), so a length equal to the buffer size is ambiguous — it might be
the whole datagram, or the OS might have silently dropped the rest. Pass
a `maxlen` comfortably larger than the expected payload to disambiguate
in practice.

`(udp:close! socket)` is idempotent; every subsequent send/receive on a
closed socket errors immediately with a `:CLOSED` error. `(udp:socket-p
x)` / `(udp:socket-open-p socket)` are predicates. `(udp:set-timeout!
socket ms)` sets both the read and write timeout together (`nil` blocks,
the default).

## 13.5 Structured errors

Every networking failure is a structured `LispVal::Error` (Chapter 6),
never just an English string. The `data` alist always includes
`:OPERATION` and `:CATEGORY`; `:HOST`/`:PORT`/`:OS-ERROR` are present
where they apply. `:CATEGORY` is one of:

| Category | Meaning |
|---|---|
| `:TIMEOUT` | A configured read/write/connect timeout elapsed |
| `:REFUSED` | The peer actively refused the connection |
| `:RESET` | The connection was reset/aborted, or the peer closed the write side |
| `:DNS` | Hostname resolution failed |
| `:CLOSED` | The port/handle was already closed |
| `:POLICY-DENIED` | The host's `set_net_policy` callback denied the operation (`docs/embedding.md`) |
| `:ADDR-IN-USE` | `tcp:listen`/`udp:bind` on an already-bound address/port |
| `:ADDR-NOT-AVAILABLE` | The requested bind address is not available on this host |
| `:OTHER` | Anything not classified above |

A connected TCP port's ordinary `PORTS` byte operations (`read-byte!`,
`write-bytes!`, ...) get the same `:CATEGORY` classification as
`tcp:connect`/`tcp:listen` — a timed-out `read-byte!` on a TCP port and a
timed-out `tcp:connect` both signal `:CATEGORY :TIMEOUT`:

```lisp
(handler-case (ports:read-byte! slow-tcp-port)
  (error (e) (cdr (assoc ':category (error-data e)))))
; => :TIMEOUT (if tcp:set-read-timeout! was set and elapsed)
```

## 13.6 Host policy hook

`docs/embedding.md`'s "Networking policy" section documents
`Environment::set_net_policy` — a Rust-only callback consulted, in
addition to the capability check, before every resolve/connect/bind, so a
host can scope a broad grant to specific destinations. There is no
Lisp-facing equivalent; this is deliberately an embedder-only lever, like
`add_module_search_path` (Chapter 10).

## 13.7 HTTP (client and server)

`HTTP` (`lib/40-http.lisp`, issue #259) is an HTTP/1.1 client and server
written entirely in Lisp on top of `TCP`/`PORTS` and the `URL`/`MIME`/
`JSON` codec modules — zero new crate dependencies, zero new Rust kernel
surface, HTTP/2 and HTTP/3 out of scope. Pull it in with
`(require 'http)`; `with_stdlib()` environments (including the CLI) have
it already. **`http://` always; `https://` client support when the
`net-tls` cargo feature is compiled in** (§13.8): an `https://` URL — given
directly or arriving as a redirect `Location` — connects via `tcp:connect`
then wraps with `tls:connect` (verification on, SNI, ALPN `"http/1.1"`)
before speaking HTTP/1.1, exactly the same way as `http://` from every
other function's point of view. With `net-tls` compiled out, `https://`
signals a structured error with `:CATEGORY :HTTPS-UNSUPPORTED` naming the
`net-tls` feature, never a silent downgrade; redirects never silently cross
schemes either way. Requiring `HTTP` grants no network authority: the
client needs `NET-CONNECT` and the server needs `NET-LISTEN`, both enforced
by the underlying `tcp:connect`/`tcp:listen` gates (§13.1) — `HTTP` adds no
new capability of its own (an `https://` client additionally rides
`tls:connect`'s own gates — see §13.8 — but that is `READ-FS`, only if you
pass a path `:extra-roots`, not a new networking capability). Server-side
`https://` (wrapping an accepted connection before `serve`/`serve-one!`
parses it) is out of scope: `tls:wrap-server` already composes directly
with `tcp:accept` for a caller that wants a TLS server without this
module's own accept loop.

### Client

`http:request`, with `http:get`/`http:post` sugar. Requests and responses
are plain alists; headers are `MIME`'s ordered `(name . value)` list
(repeats preserved, lookup case-insensitive — Chapter 12's conventions).

```console
$ target/debug/lamedh
> (require 'http)
HTTP
> (def r (http:get "http://127.0.0.1:8080/hello"))
...
> (http:response-status r)
200
> (http:collect-string (http:response-body r))
"hello world"
```

The response's `:BODY` is an **unread body stream** — a framing-aware
reader (`Content-Length` exact bytes / chunked decoding with hex size
lines and trailer skipping / read-to-close / no body for HEAD, 1xx, 204,
304) that never over-reads into the connection and never buffers without
bound. Read it incrementally with `http:stream-read!` /
`http:stream-eof-p` / `http:stream-close!`, or collect it bounded (10 MiB
default, `:max-bytes` to change) with `http:collect-bytes` /
`http:collect-string` / `http:collect-json`. Bodies are bytes; text
decoding is an explicit UTF-8 step (`collect-string`, strict by default,
`:lossy t` for replacement characters), and `collect-json` composes with
`json:parse` (Chapter 12) rather than being an HTTP primitive.

Request bodies: `:body` is `NIL`, a String, an `Array<Char>`, or a
readable `PORTS` port — the port case streams out via chunked
transfer-encoding. The client always sends `Connection: close` (no
connection reuse; every hop is a fresh connection), adds `Host`
(with the port iff non-default) and `Content-Length`/`Transfer-Encoding`
automatically, and never overrides a header you set explicitly.

Timeouts: `:connect-timeout-ms` bounds the TCP connect;
`:read-timeout-ms` bounds every individual socket read (a stalled server
signals `:CATEGORY :TIMEOUT`); `:overall-timeout-ms` is a coarse
wall-clock deadline checked between connection phases and redirect hops.

Redirects: 301/302/303/307/308 followed by default (`:follow-redirects
nil` to disable), capped at `:max-redirects` (default 5, exceeding it is
`:CATEGORY :TOO-MANY-REDIRECTS`). Method rules: 303 always becomes GET;
301/302 downgrade POST to GET and drop the body; 307/308 preserve method
and body (a streamed port body cannot be replayed — that is a clear
error, not a silent empty resend). A cross-origin hop strips
`Authorization`/`Cookie`/`Proxy-Authorization`. A `Location` naming an
unsupported scheme is never followed silently: an `https://` redirect is
followed exactly like an `https://` initial request (§13.7's own rule —
the `:HTTPS-UNSUPPORTED` error above with `net-tls` off, a real TLS-wrapped
hop with it on), anything else is `:UNSUPPORTED-SCHEME`.
Bare relative references (`foo`, `../foo`) are an explicit
`:UNSUPPORTED-REDIRECT` error — absolute, protocol-relative, and
absolute-path forms are resolved. No proxy support: ambient proxy
environment variables are deliberately ignored.

### Server

```lisp
(require 'http)                                   ; needs NET-LISTEN to listen
(def listener (tcp:listen "127.0.0.1" 8080))
(http:serve listener
  (lambda (req)
    (cond
      ((equal (http:request-path req) "/hello")
       (http:respond 200
         :headers (list (cons "Content-Type" "text/plain; charset=utf-8"))
         :body "hello world"))
      ((equal (http:request-method req) "POST")
       (http:respond 200
         :body (concat "you said: "
                       (http:collect-string (http:request-body req)))))
      (t (http:respond 404 :body "not found")))))
```

The handler receives a request alist (`http:request-method`, `-path`,
`-query`, `-headers`, `-header`, `-body` — an unread body stream with the
same framing awareness as the client's, `Content-Length` and chunked
alike — `-version`, `-peer-addr`, `-target`) and returns a response built
with `http:respond` (status, `:headers`, `:body` as `NIL`/String/
`Array<Char>`/readable port — the port case streams out chunked;
`Content-Length` is set automatically otherwise).

Execution model: synchronous and serial — one connection is served fully
(every keep-alive request on it, in order: `Connection: keep-alive` is
honored, unread request-body bytes are drained between requests, and
`Connection: close` or HTTP/1.0 ends the connection) before the next is
accepted. Concurrent serving belongs to the isolated-worker design
(issue #140), not this module. `http:serve-one!` accepts and serves
exactly one connection (useful for tests); `http:serve` loops with
`:max-requests` (a connection-count bound) and `:stop-p` (a shutdown
predicate consulted between connections — it cannot interrupt a blocking
accept; see `tcp:close-listener!`'s documented limitation).

Limits and failure behavior: request lines and header lines are bounded
(`:max-line-bytes`, default 8 KiB), header count is bounded
(`:max-header-count`, default 200), and a request body larger than
`:max-body-bytes` (default 10 MiB) is refused with `413` without invoking
the handler. An uncaught handler error becomes a generic
`500 Internal Server Error` that never carries the condition's message or
data to the peer; pass `:on-error` to receive the structured condition
host-side for diagnostics.

Resource-cleanup rules: the server closes each accepted connection port
itself (via `unwind-protect`), including when the handler errors. On the
client side, collecting a response body to the end leaves the connection
to be dropped with the response value; call `http:stream-close!` on a
response body you abandon partway to close its connection deterministically
rather than waiting for `Drop` (Chapter 11's ownership rule).

Verification paths: the default build and `--no-default-features` both
carry `HTTP` (it is embedded Lisp, not a Cargo feature) — `cargo test`
and `cargo test --no-default-features` both run its suite
(`tests/test_http.rs`), loopback-only. `https://` support specifically
needs `cargo test --features net-tls` — see §13.8.

## 13.8 TLS

`TLS` (`lib/43-tls.lisp`, issue #365) wraps a connected TCP `Port` as a
client or server, behind a new **`net-tls` cargo feature that is NOT in the
default feature set** — build with `cargo build --features net-tls` (or
`cargo run --features net-tls`, `cargo test --features net-tls`) to use it.
The default build's behavior and dependency tree are unchanged: nothing
here pulls in `rustls`/`webpki-roots`/`rustls-pemfile` unless you ask for
this feature. Backed by `rustls` (#364/#365: not
`native-tls`, so nothing links system OpenSSL/SChannel/SecureTransport),
with the `ring` crypto provider (not `aws-lc-rs`: no cmake/nasm build-tool
requirement).

```console
$ target/debug/lamedh -s "(tls:available-p)"
NIL
$ cargo build --features net-tls && target/debug/lamedh -s "(tls:available-p)"
T
```

**`(require 'tls)` always works, regardless of the feature** — `lib/43-tls.lisp`
is embedded like every other optional module, so the file loads and every
`tls:*` name resolves either way. `tls:available-p` reports whether
`net-tls` is actually compiled in; every other `tls:*` operation signals a
structured `:CATEGORY :TLS-UNAVAILABLE` error instead of doing any work
when it is not, rather than an unbound-variable error — a program can
`(require 'tls)` unconditionally and branch on `tls:available-p`.

A TLS stream is an ordinary `PORTS` port (§13 intro, Chapter 11): every
read/write/close/`with-open-port`/`port-p` operation, and every `TCP`
out-of-band operation (`tcp:shutdown!`, `tcp:set-read-timeout!`/
`set-write-timeout!`, `net:local-addr`/`net:peer-addr`) already works on it
unchanged — `PortObj`'s TCP-specific methods were generalized to match a
TLS-wrapped socket's underlying `.sock` too.

**Wrapping consumes the plaintext port**: `tls:wrap-client port
:hostname host &key alpn extra-roots` and `tls:wrap-server port cert key
&key alpn` take ownership of the underlying TCP connection out of `port` —
that original `Port` value becomes `CLOSED` (errors like any other closed
port) the instant it is wrapped, so there is never a moment where the same
socket is reachable as both a plaintext port and a TLS port. `tls:connect
host port &key connect-timeout-ms handshake-timeout-ms alpn extra-roots`
is `tcp:connect` + `wrap-client` sugar (`:hostname` defaults to `host`);
`:handshake-timeout-ms`, if given, sets both the read and write timeout on
the TCP port before wrapping, so a stalled handshake times out instead of
blocking forever — this is "handshake timeout via the underlying socket's
read/write timeouts."

```console
$ target/debug/lamedh
> (require 'tls)
TLS
> (def listener (tcp:listen "127.0.0.1" 0))
...
```

Verification is on by default: the certificate chain is checked against the
default root store (Mozilla's set, via `webpki-roots`) plus any
`:extra-roots` you supply — each a PEM source, a String path (`READ-FS`
capability, checked the same way `read-file` is) or an `Array<Char>` of raw
bytes — trusted in addition to the default store; this is also how a test
harness trusts a throwaway self-signed CA. The certificate is checked
against `:hostname`, which doubles as the SNI server name sent during the
handshake. A verification failure signals a structured `:CATEGORY
:TLS-VERIFY-FAILED` error; other handshake failures (malformed records, a
peer that isn't speaking TLS at all, ...) are `:TLS-HANDSHAKE`;
misconfiguration (a bad hostname, unparseable PEM data, ...) is
`:TLS-CONFIG` — every other §13.5 category (`:TIMEOUT`, `:RESET`, ...)
applies unchanged for the underlying transport.

**No insecure bypass without an explicitly named API, and a host opt-in.**
`tls:connect-insecure!`/`tls:wrap-client-insecure!` are the *only* way to
skip certificate verification — there is no keyword flag on `tls:connect`
that silently does it — and calling them signals a structured
`:CATEGORY :POLICY-DENIED` error unless the *embedding host* has separately
called the new `Environment::set_allow_insecure_tls` (Rust-only, default
`false`; see `docs/embedding.md`). This mirrors `set_net_policy`'s "Lisp
cannot install or inspect this" shape one level further: a policy callback
can only narrow an already-granted capability, while this flag is a second,
independent gate the host must explicitly widen. Lisp code alone can never
disable verification, no matter what it calls.

```console
$ target/debug/lamedh -s "(tls:connect-insecure! \"127.0.0.1\" 1)"
Error: tls-wrap-client-insecure*: POLICY-DENIED: tls:connect-insecure! requires host opt-in (Environment::set_allow_insecure_tls) -- Lisp code alone cannot disable certificate verification
  in: TLS:CONNECT-INSECURE!
```

ALPN and peer-certificate diagnostics:

```lisp
(tls:alpn-protocol port)              ; => "http/1.1", or NIL if none negotiated
(tls:peer-certificates port)          ; => list of Array<Char> DER, leaf first, or NIL
(tls:peer-certificate-summary port)   ; => ((:count . 1) (:leaf-der-length . 851) (:leaf-der . #<array>))
(tls:sni-hostname port)               ; => the SNI a client offered, server-side only
```

`tls:peer-certificate-summary` is deliberately structural, not parsed: this
dependency ruling adds no X.509 parser (`rustls` + `webpki-roots` +
`rustls-pemfile` only), so there are no subject/issuer/expiry fields — a
caller that needs those pulls in its own X.509 parser and works from
`tls:peer-certificates`'s raw DER.

Server-side wrapping mirrors the client: `cert` may be a full chain
(leaf first) in one PEM source; `key` is the matching private key; no
client-certificate authentication is requested.

```console
$ target/debug/lamedh
> (require 'tls)
TLS
> (let* ((accepted (tcp:accept my-listener))
         (plain (car accepted))
         (tls (tls:wrap-server plain "cert.pem" "key.pem" :alpn (list "http/1.1"))))
    (ports:write-bytes! tls (text:string->utf8 "hi"))
    (ports:close! tls))
```

**https:// through HTTP** (§13.7): with `net-tls` compiled in, `http:request`
(and `get`/`post`) connect an `https://` URL — initial request or redirect
`Location` alike — via `tcp:connect` then `tls:connect` (ALPN `"http/1.1"`)
before speaking HTTP/1.1 over the resulting port; `:extra-roots` on
`request`/`get`/`post` forwards verbatim to `tls:connect`. With `net-tls`
compiled out, `https://` keeps the same `:CATEGORY :HTTPS-UNSUPPORTED`
error either way — never a silent downgrade or a scheme silently crossed on
redirect. Server-side `https://` (wrapping an accepted connection before
`serve`/`serve-one!` parses it) is out of scope for `HTTP` itself —
`tls:wrap-server` already composes directly with `tcp:accept`.

Verification paths: `cargo test --features net-tls` runs
`tests/test_tls.rs` (loopback-only, a throwaway self-signed CA generated
per test via the `rcgen` **dev-dependency** — never a build/normal
dependency). The default build and `--no-default-features` both still pass
`cargo test`/`cargo test --no-default-features`: `TLS` loads either way
(behavior above), and the feature-off assertions live in
`tests/test_net.rs`/`tests/test_http.rs`.

## 13.9 Summary

- A connected TCP stream is an ordinary `PORTS` port; a listener or a UDP
  socket is its own opaque handle (`net:*`/`tcp:*`/`udp:*` predicates,
  never a raw file descriptor or integer). A TLS-wrapped stream (§13.8) is
  the same `PORTS` port shape too.
- `NET-DNS`/`NET-CONNECT`/`NET-LISTEN` are independent, fence-attenuated
  capabilities; an embedder can further scope any of them with
  `set_net_policy` (Rust-only).
- Structured errors carry a portable `:CATEGORY` (`:TIMEOUT`/`:REFUSED`/
  `:RESET`/`:DNS`/`:CLOSED`/`:POLICY-DENIED`/`:ADDR-IN-USE`/
  `:ADDR-NOT-AVAILABLE`/`:OTHER`), including for a TCP port's ordinary
  `PORTS` read/write operations; TLS adds `:TLS-VERIFY-FAILED`/
  `:TLS-HANDSHAKE`/`:TLS-CONFIG`/`:TLS-UNAVAILABLE`.
- `tcp:listen`'s `backlog` argument and `udp:receive-from`'s truncation
  flag are both honestly-documented, std-library-shaped limitations, not
  full OS-level control — see §13.3/§13.4.
- `HTTP` (§13.7) is a pure-Lisp HTTP/1.1 client and server over the TCP
  substrate — streaming framing-aware bodies, bounded collection,
  redirect policy, serial keep-alive serving; `http://` always, `https://`
  client support with `net-tls`; no new capability of its own.
- `TLS` (§13.8) wraps a connected TCP port as a client or server, behind
  the off-by-default `net-tls` cargo feature (rustls/ring) — verification
  on by default, `:extra-roots` for private CAs, no insecure bypass without
  both an explicitly named API and a host-only opt-in, and the same
  `PORTS` port shape `TCP` already established.
