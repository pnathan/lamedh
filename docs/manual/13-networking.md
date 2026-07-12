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

## 13.7 TLS (not in this release)

Wrapping a connected TCP port as a TLS client/server port — certificate
verification, ALPN/SNI, handshake timeouts — is explicitly out of scope
for this release: it forces a TLS-crate dependency decision that belongs
to the project owner, and this release ships zero new dependencies. The
design here is the seam a future TLS layer wraps without an API change:
a TLS port would be, like a TCP stream, an ordinary `PORTS` port, taking
and returning the same shape `tcp:connect`/`tcp:accept` already do.

## 13.8 Summary

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
- TLS is deferred to a follow-up that can make its own dependency
  decision; the port-wrapping design here is the seam it will use.
