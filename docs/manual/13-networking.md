# 13. Networking

Chapter 11 covered synchronous binary I/O over files, memory buffers, and
standard streams. This chapter covers the same synchronous binary-port
model over the network: DNS resolution, TCP, and UDP — three optional
embedded libraries, `NET` (`lib/37-net.lisp`), `TCP` (`lib/38-tcp.lisp`),
and `UDP` (`lib/39-udp.lisp`), built entirely on `std::net` with zero new
crate dependencies. Pull them in with `(require 'net)`/`(require 'tcp)`/
`(require 'udp)` on a `with_prelude()`-style environment; `with_stdlib()`
environments (including the `lamedh` CLI) already have all three loaded.

**TLS is not covered here** — it is explicitly out of scope for this
release; see §13.7.

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
$ target/debug/lamedh -s "(net:resolve \"localhost\")"
Error: NET-DNS capability is not enabled (grant it via --capability NET-DNS or the host API)
  in: NET:RESOLVE
```

Each capability is independent — granting one does not unlock the
others — and every one is attenuated by `with-capabilities` fences exactly
like every other host builtin (Chapter 7 §7.1):

```console
$ target/debug/lamedh --capability NET-CONNECT -s "(with-capabilities '() (tcp-connect* \"127.0.0.1\" 1 100))"
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
$ target/debug/lamedh --capability NET-DNS -s "(net:resolve \"localhost\")"
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
$ target/debug/lamedh --capability NET-LISTEN -s "(let ((l (tcp:listen \"127.0.0.1\" 0))) (net:address->string (tcp:local-addr l)))"
"127.0.0.1:35515"
```

(The port number is OS-assigned — port `0` on bind always is — so it will
differ on every run; only the shape is stable.)

A malformed or unresolvable host signals a structured `:DNS` error rather
than a bare string, so callers can dispatch on it (§13.5):

```console
$ target/debug/lamedh --capability NET-DNS -s "(handler-case (net:resolve \"\") (error (e) (error-data e)))"
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
$ target/debug/lamedh --capability NET-LISTEN --capability NET-CONNECT -s "(progn
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
$ target/debug/lamedh --capability NET-LISTEN --capability NET-CONNECT -s "(progn
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
it already. **Plaintext `http://` only for now**: an `https://` URL — given
directly or arriving as a redirect `Location` — signals a structured error
with `:CATEGORY :HTTPS-UNSUPPORTED` naming issue #365 (the pending TLS
dependency ruling), never a silent downgrade. Requiring `HTTP` grants no
network authority: the client needs `NET-CONNECT` and the server needs
`NET-LISTEN`, both enforced by the underlying `tcp:connect`/`tcp:listen`
gates (§13.1) — `HTTP` adds no new capability of its own.

### Client

`http:request`, with `http:get`/`http:post` sugar. Requests and responses
are plain alists; headers are `MIME`'s ordered `(name . value)` list
(repeats preserved, lookup case-insensitive — Chapter 12's conventions).

```console
$ target/debug/lamedh --capability NET-CONNECT
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
`Authorization`/`Cookie`/`Proxy-Authorization`. A `Location` naming a
different scheme is never followed silently: `https://` is the
`:HTTPS-UNSUPPORTED` error above, anything else `:UNSUPPORTED-SCHEME`.
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
(`tests/test_http.rs`), loopback-only.

## 13.8 TLS (not in this release)

Wrapping a connected TCP port as a TLS client/server port — certificate
verification, ALPN/SNI, handshake timeouts — is explicitly out of scope
for this release: it forces a TLS-crate dependency decision that belongs
to the project owner, and this release ships zero new dependencies (the
ruling is tracked as issue #365). The design here is the seam a future
TLS layer wraps without an API change: a TLS port would be, like a TCP
stream, an ordinary `PORTS` port, taking and returning the same shape
`tcp:connect`/`tcp:accept` already do — and `HTTP` (§13.7) would gain
`https://` support at its single scheme checkpoint without any API
change.

## 13.9 Summary

- A connected TCP stream is an ordinary `PORTS` port; a listener or a UDP
  socket is its own opaque handle (`net:*`/`tcp:*`/`udp:*` predicates,
  never a raw file descriptor or integer).
- `NET-DNS`/`NET-CONNECT`/`NET-LISTEN` are independent, fence-attenuated
  capabilities; an embedder can further scope any of them with
  `set_net_policy` (Rust-only).
- Structured errors carry a portable `:CATEGORY` (`:TIMEOUT`/`:REFUSED`/
  `:RESET`/`:DNS`/`:CLOSED`/`:POLICY-DENIED`/`:ADDR-IN-USE`/
  `:ADDR-NOT-AVAILABLE`/`:OTHER`), including for a TCP port's ordinary
  `PORTS` read/write operations.
- `tcp:listen`'s `backlog` argument and `udp:receive-from`'s truncation
  flag are both honestly-documented, std-library-shaped limitations, not
  full OS-level control — see §13.3/§13.4.
- `HTTP` (§13.7) is a pure-Lisp HTTP/1.1 client and server over the TCP
  substrate — streaming framing-aware bodies, bounded collection,
  redirect policy, serial keep-alive serving; plaintext `http://` only,
  no new capability of its own.
- TLS is deferred to a follow-up that can make its own dependency
  decision (issue #365); the port-wrapping design here is the seam it
  will use, and `HTTP`'s single scheme checkpoint is where `https://`
  slots in.
