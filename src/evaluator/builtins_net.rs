//! Networking: DNS/address, TCP, and UDP kernel substrate (issue #258, epic
//! #253).
//!
//! Every `NET-*`/`TCP-*`/`UDP-*` primitive here is a thin argument-parsing/
//! capability-check wrapper, exactly like `src/evaluator/builtins_ports.rs`
//! is for binary ports — the actual `std::net` I/O and the
//! `PortState::TcpStream`/`NetHandleObj` representations live in
//! `src/lib.rs`. The Lisp-facing names live in `lib/37-net.lisp`
//! (addresses/DNS, `NET` module), `lib/38-tcp.lisp` (`TCP` module), and
//! `lib/39-udp.lisp` (`UDP` module).
//!
//! **A connected TCP stream is an ordinary [`crate::LispVal::Port`]**
//! (`PortState::TcpStream`, issue #255's binary-port representation) — every
//! `PORTS`/`TCP` read/write/close/timeout operation therefore works on it
//! unchanged, and it is also the seam a future TLS layer (explicitly
//! deferred from this issue) can wrap without changing the port API.
//! Listeners and UDP sockets are NOT byte streams, so they get their own
//! opaque representation, [`crate::NetHandleObj`] (`LispVal::NetHandle`).
//!
//! **Capability model.** Three ambient authorities gate acquisition, exactly
//! like `READ-FS`/`CREATE-FS`/`IO` gate ports (same `require_*` helpers in
//! `builtins_core.rs`, same `cap_mask_allows` dynamic-extent mask so
//! `WITH-CAPABILITIES` fences attenuate them):
//!
//! - `NET-DNS` — explicit hostname resolution (`NET-RESOLVE*`).
//! - `NET-CONNECT` — outbound connections (`TCP-CONNECT*`, `UDP-CONNECT*`,
//!   `UDP-SEND-TO*`).
//! - `NET-LISTEN` — binding/listening for inbound traffic (`TCP-LISTEN*`,
//!   `UDP-BIND*`).
//!
//! In addition to (not instead of) the capability check, every
//! capability-gated operation also consults the host policy hook
//! ([`crate::environment::Environment::set_net_policy`]) with the operation
//! and the caller-supplied host/port, so an embedder can scope a granted
//! capability to specific destinations (e.g. an HTTP-client grant that
//! should not become unrestricted SSRF authority). The policy sees the
//! caller-supplied hostname/address, not a post-DNS-resolved IP — see that
//! function's doc comment for the resulting limitation.
//!
//! Once a resource is successfully acquired, using it performs no further
//! capability or policy check — acquisition is gated, continued use of an
//! already-acquired handle is not (the epic's "a successfully returned
//! handle is authority to continue" rule, same as ports).
//!
//! **Error data.** Every failure is a structured [`crate::LispVal::Error`]
//! whose `data` is an alist with at least `:OPERATION` (a string) and
//! `:CATEGORY` (a keyword symbol: `:TIMEOUT`, `:REFUSED`, `:RESET`, `:DNS`,
//! `:CLOSED`, `:POLICY-DENIED`, `:ADDR-IN-USE`, `:ADDR-NOT-AVAILABLE`, or
//! `:OTHER`), plus `:HOST`/`:PORT`/`:OS-ERROR` where applicable — see
//! [`net_error`].

use super::*;
use std::net::{Shutdown, SocketAddr, TcpListener, TcpStream, ToSocketAddrs, UdpSocket};
use std::time::Duration;

// ── Argument helpers (mirrors builtins_ports.rs's local set) ─────────────

fn expect_string(args: &[LispVal], i: usize, who: &str) -> Result<String, LispError> {
    match args.get(i) {
        Some(LispVal::String(s)) => Ok(s.clone()),
        Some(other) => Err(LispError::Generic(format!(
            "{}: expected a string, got {}",
            who.to_uppercase(),
            err_val(other)
        ))),
        None => Err(LispError::Generic(format!(
            "{who} requires a string argument"
        ))),
    }
}

fn expect_port_number(args: &[LispVal], i: usize, who: &str) -> Result<u16, LispError> {
    match args.get(i) {
        Some(LispVal::Number(n)) if (0..=65535).contains(n) => Ok(*n as u16),
        Some(other) => Err(LispError::Generic(format!(
            "{}: expected a port number 0-65535, got {}",
            who.to_uppercase(),
            err_val(other)
        ))),
        None => Err(LispError::Generic(format!(
            "{who} requires a port-number argument"
        ))),
    }
}

fn expect_nonneg(args: &[LispVal], i: usize, who: &str) -> Result<usize, LispError> {
    match args.get(i) {
        Some(LispVal::Number(n)) if *n >= 0 => Ok(*n as usize),
        Some(other) => Err(LispError::Generic(format!(
            "{}: expected a non-negative integer, got {}",
            who.to_uppercase(),
            err_val(other)
        ))),
        None => Err(LispError::Generic(format!(
            "{who} requires a non-negative integer argument"
        ))),
    }
}

/// `nil` or a non-negative integer (milliseconds); used for optional
/// timeouts. `nil` means "no timeout" (blocking), matching the underlying
/// `set_*_timeout(None)` semantics.
fn expect_optional_millis(
    args: &[LispVal],
    i: usize,
    who: &str,
) -> Result<Option<Duration>, LispError> {
    match args.get(i) {
        None | Some(LispVal::Nil) => Ok(None),
        Some(LispVal::Number(n)) if *n > 0 => Ok(Some(Duration::from_millis(*n as u64))),
        Some(other) => Err(LispError::Generic(format!(
            "{}: expected NIL or a positive integer (milliseconds), got {}",
            who.to_uppercase(),
            err_val(other)
        ))),
    }
}

fn expect_port<'a>(
    args: &'a [LispVal],
    i: usize,
    who: &str,
) -> Result<&'a Shared<PortObj>, LispError> {
    match args.get(i) {
        Some(LispVal::Port(p)) => Ok(p),
        Some(other) => Err(LispError::Generic(format!(
            "{}: expected a port, got {}",
            who.to_uppercase(),
            err_val(other)
        ))),
        None => Err(LispError::Generic(format!(
            "{who} requires a port argument"
        ))),
    }
}

fn expect_net_handle<'a>(
    args: &'a [LispVal],
    i: usize,
    who: &str,
) -> Result<&'a Shared<crate::NetHandleObj>, LispError> {
    match args.get(i) {
        Some(LispVal::NetHandle(h)) => Ok(h),
        Some(other) => Err(LispError::Generic(format!(
            "{}: expected a network handle, got {}",
            who.to_uppercase(),
            err_val(other)
        ))),
        None => Err(LispError::Generic(format!(
            "{who} requires a network-handle argument"
        ))),
    }
}

fn expect_shutdown_how(args: &[LispVal], i: usize, who: &str) -> Result<Shutdown, LispError> {
    match args.get(i) {
        Some(LispVal::Symbol(s)) => match s.borrow().name.as_str() {
            "READ" | ":READ" => Ok(Shutdown::Read),
            "WRITE" | ":WRITE" => Ok(Shutdown::Write),
            "BOTH" | ":BOTH" => Ok(Shutdown::Both),
            other => Err(LispError::Generic(format!(
                "{}: expected :READ, :WRITE, or :BOTH, got {other}",
                who.to_uppercase()
            ))),
        },
        Some(other) => Err(LispError::Generic(format!(
            "{}: expected a symbol (:READ/:WRITE/:BOTH), got {}",
            who.to_uppercase(),
            err_val(other)
        ))),
        None => Err(LispError::Generic(format!(
            "{who} requires a direction argument"
        ))),
    }
}

// ── Structured error construction ─────────────────────────────────────────

/// Classify an [`std::io::Error`] into one of this module's error
/// categories. Connection-level categories only — DNS/closed/policy-denied
/// are known from context, not from an `io::Error`, so callers set those
/// directly instead of routing through this classifier.
pub(super) fn classify_io_error(kind: std::io::ErrorKind) -> &'static str {
    use std::io::ErrorKind::*;
    match kind {
        // `WouldBlock` (EAGAIN) is what a `set_*_timeout`-configured read/
        // write actually returns on Linux when the timeout elapses --
        // `TimedOut` is documented but not what the OS surfaces in
        // practice. Every port/socket in this kernel is used synchronously
        // (issue #258's "synchronous, compatible with share-nothing
        // workers"; nothing ever calls `set_nonblocking`), so a `WouldBlock`
        // here can only mean "the configured timeout elapsed" -- safe to
        // fold into the same :TIMEOUT category callers actually want.
        TimedOut | WouldBlock => "TIMEOUT",
        ConnectionRefused => "REFUSED",
        ConnectionReset | ConnectionAborted | BrokenPipe => "RESET",
        AddrInUse => "ADDR-IN-USE",
        AddrNotAvailable => "ADDR-NOT-AVAILABLE",
        NotConnected => "NOT-CONNECTED",
        _ => "OTHER",
    }
}

/// Build a structured networking error: a `LispVal::Error` whose `data` is
/// an alist `((:operation . "...") (:category . :keyword) (:host . "...")
/// (:port . N) (:os-error . "..."))` — see this module's doc comment.
/// `host`/`port` are the caller-supplied values, not necessarily a resolved
/// address.
fn net_error(
    env: &Shared<Environment>,
    operation: &str,
    category: &str,
    host: &str,
    port: i64,
    detail: &str,
) -> LispError {
    let cons = |car: LispVal, cdr: LispVal| LispVal::Cons {
        car: Shared::new(car),
        cdr: Shared::new(cdr),
    };
    let sym = |s: &str| LispVal::Symbol(env.intern_symbol(s));
    let pair = |k: &str, v: LispVal| cons(sym(k), v);
    let data = cons(
        pair(":OPERATION", LispVal::String(operation.to_string())),
        cons(
            pair(":CATEGORY", sym(&format!(":{category}"))),
            cons(
                pair(":HOST", LispVal::String(host.to_string())),
                cons(
                    pair(":PORT", LispVal::Number(port)),
                    cons(
                        pair(":OS-ERROR", LispVal::String(detail.to_string())),
                        LispVal::Nil,
                    ),
                ),
            ),
        ),
    );
    let message = format!("{operation}: {category} ({host}:{port}): {detail}");
    LispError::Signaled(Box::new(LispVal::Error(Shared::new(crate::ErrorObj {
        message,
        data,
    }))))
}

fn io_net_error(
    env: &Shared<Environment>,
    operation: &str,
    host: &str,
    port: i64,
    e: std::io::Error,
) -> LispError {
    let category = classify_io_error(e.kind());
    net_error(env, operation, category, host, port, &e.to_string())
}

/// A structured error for use-after-close on a [`crate::NetHandleObj`] (TCP
/// listener or UDP socket), analogous to `PortObj`'s "port is closed"
/// (issue #255) but with an explicit `:CLOSED` category per this issue's
/// error-data requirement.
fn closed_handle_error(env: &Shared<Environment>, operation: &str, name: &str) -> LispError {
    net_error(
        env,
        operation,
        "CLOSED",
        name,
        0,
        "network handle is closed",
    )
}

// ── Address construction ──────────────────────────────────────────────────

fn family_keyword(addr: &SocketAddr) -> &'static str {
    if addr.is_ipv4() { ":IPV4" } else { ":IPV6" }
}

/// A `SocketAddr` as first-class, printable Lisp data: a 3-element list
/// `(family ip-string port)`, e.g. `(:IPV4 "127.0.0.1" 8080)`. Deliberately
/// raw kernel data, not a record — `lib/37-net.lisp`'s `NET:ADDRESS`
/// `defrecord` wraps this into the nominal, documented address type (this
/// issue's "first-class, printable IP/socket-address data represented
/// without exposing platform structs").
fn addr_to_triple(env: &Shared<Environment>, addr: SocketAddr) -> LispVal {
    let sym = |s: &str| LispVal::Symbol(env.intern_symbol(s));
    let cons = |car: LispVal, cdr: LispVal| LispVal::Cons {
        car: Shared::new(car),
        cdr: Shared::new(cdr),
    };
    cons(
        sym(family_keyword(&addr)),
        cons(
            LispVal::String(addr.ip().to_string()),
            cons(LispVal::Number(addr.port() as i64), LispVal::Nil),
        ),
    )
}

fn addrs_to_list(env: &Shared<Environment>, addrs: Vec<SocketAddr>) -> LispVal {
    let mut out = LispVal::Nil;
    for addr in addrs.into_iter().rev() {
        out = LispVal::Cons {
            car: Shared::new(addr_to_triple(env, addr)),
            cdr: Shared::new(out),
        };
    }
    out
}

fn bytes_to_char_array(bytes: Vec<u8>) -> LispVal {
    let items: Vec<LispVal> = bytes.into_iter().map(LispVal::Char).collect();
    LispVal::Array(Shared::new(SharedCell::new(items)))
}

#[inline(never)]
pub(super) fn apply_net_op(
    op: &BuiltinFunc,
    args: &[LispVal],
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    let t = || LispVal::Symbol(env.intern_symbol("T"));
    match op {
        // ── DNS / addresses ────────────────────────────────────────────
        BuiltinFunc::NetResolve => {
            require_net_dns(env)?;
            if args.is_empty() || args.len() > 2 {
                return Err(LispError::Generic(
                    "net-resolve* requires one or two arguments: host [port]".to_string(),
                ));
            }
            let host = expect_string(args, 0, "net-resolve*")?;
            let port = if args.len() == 2 {
                expect_port_number(args, 1, "net-resolve*")?
            } else {
                0
            };
            if !env.check_net_policy(crate::NetOperation::Resolve, &host, port) {
                return Err(net_error(
                    env,
                    "resolve",
                    "POLICY-DENIED",
                    &host,
                    port as i64,
                    "denied by host network policy",
                ));
            }
            let addrs: Vec<SocketAddr> = (host.as_str(), port)
                .to_socket_addrs()
                .map_err(|e| net_error(env, "resolve", "DNS", &host, port as i64, &e.to_string()))?
                .collect();
            if addrs.is_empty() {
                return Err(net_error(
                    env,
                    "resolve",
                    "DNS",
                    &host,
                    port as i64,
                    "no addresses found",
                ));
            }
            Ok(addrs_to_list(env, addrs))
        }
        BuiltinFunc::NetLocalAddr => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "net-local-addr* requires exactly one argument".to_string(),
                ));
            }
            match &args[0] {
                LispVal::Port(p) => {
                    let addr = p.tcp_local_addr().map_err(|e| {
                        net_error(env, "local-addr", "OTHER", &p.name, 0, &e.to_string())
                    })?;
                    Ok(addr_to_triple(env, addr))
                }
                LispVal::NetHandle(h) => {
                    if !h.is_open() {
                        return Err(closed_handle_error(env, "local-addr", &h.name));
                    }
                    let addr = h.local_addr().map_err(|e| {
                        net_error(env, "local-addr", "OTHER", &h.name, 0, &e.to_string())
                    })?;
                    Ok(addr_to_triple(env, addr))
                }
                other => Err(LispError::Generic(format!(
                    "NET-LOCAL-ADDR*: expected a port or network handle, got {}",
                    err_val(other)
                ))),
            }
        }
        BuiltinFunc::NetPeerAddr => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "net-peer-addr* requires exactly one argument".to_string(),
                ));
            }
            let port = expect_port(args, 0, "net-peer-addr*")?;
            let addr = port
                .tcp_peer_addr()
                .map_err(|e| net_error(env, "peer-addr", "OTHER", &port.name, 0, &e.to_string()))?;
            Ok(addr_to_triple(env, addr))
        }

        // ── TCP ─────────────────────────────────────────────────────────
        BuiltinFunc::TcpConnect => {
            require_net_connect(env)?;
            if args.len() < 2 || args.len() > 3 {
                return Err(LispError::Generic(
                    "tcp-connect* requires two or three arguments: host port [timeout-ms]"
                        .to_string(),
                ));
            }
            let host = expect_string(args, 0, "tcp-connect*")?;
            let port = expect_port_number(args, 1, "tcp-connect*")?;
            let timeout = expect_optional_millis(args, 2, "tcp-connect*")?;
            if !env.check_net_policy(crate::NetOperation::Connect, &host, port) {
                return Err(net_error(
                    env,
                    "connect",
                    "POLICY-DENIED",
                    &host,
                    port as i64,
                    "denied by host network policy",
                ));
            }
            let stream = match timeout {
                None => TcpStream::connect((host.as_str(), port))
                    .map_err(|e| io_net_error(env, "connect", &host, port as i64, e))?,
                Some(dur) => {
                    let addrs: Vec<SocketAddr> = (host.as_str(), port)
                        .to_socket_addrs()
                        .map_err(|e| {
                            net_error(env, "connect", "DNS", &host, port as i64, &e.to_string())
                        })?
                        .collect();
                    if addrs.is_empty() {
                        return Err(net_error(
                            env,
                            "connect",
                            "DNS",
                            &host,
                            port as i64,
                            "no addresses found",
                        ));
                    }
                    let mut last_err = None;
                    let mut connected = None;
                    for addr in addrs {
                        match TcpStream::connect_timeout(&addr, dur) {
                            Ok(s) => {
                                connected = Some(s);
                                break;
                            }
                            Err(e) => last_err = Some(e),
                        }
                    }
                    match connected {
                        Some(s) => s,
                        None => {
                            let e = last_err.expect("addrs was non-empty");
                            return Err(io_net_error(env, "connect", &host, port as i64, e));
                        }
                    }
                }
            };
            let name = stream
                .peer_addr()
                .map(|a| a.to_string())
                .unwrap_or_else(|_| format!("{host}:{port}"));
            Ok(LispVal::Port(crate::PortObj::tcp_stream(name, stream)))
        }
        BuiltinFunc::TcpListen => {
            require_net_listen(env)?;
            if args.len() != 3 {
                return Err(LispError::Generic(
                    "tcp-listen* requires exactly three arguments: host port backlog".to_string(),
                ));
            }
            let host = expect_string(args, 0, "tcp-listen*")?;
            let port = expect_port_number(args, 1, "tcp-listen*")?;
            // The backlog argument is accepted (and validated) for API
            // completeness with the issue's "bind/listen with explicit
            // address and backlog", but is NOT applied to the OS socket:
            // std::net::TcpListener has no backlog-customization API without
            // an additional crate (net2/socket2), and this issue ships zero
            // new dependencies. Flagged in the PR as a known limitation.
            let _backlog = expect_nonneg(args, 2, "tcp-listen*")?;
            if !env.check_net_policy(crate::NetOperation::Listen, &host, port) {
                return Err(net_error(
                    env,
                    "listen",
                    "POLICY-DENIED",
                    &host,
                    port as i64,
                    "denied by host network policy",
                ));
            }
            let listener = TcpListener::bind((host.as_str(), port))
                .map_err(|e| io_net_error(env, "listen", &host, port as i64, e))?;
            let name = listener
                .local_addr()
                .map(|a| a.to_string())
                .unwrap_or_else(|_| format!("{host}:{port}"));
            Ok(LispVal::NetHandle(crate::NetHandleObj::tcp_listener(
                name, listener,
            )))
        }
        BuiltinFunc::TcpAccept => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "tcp-accept* requires exactly one argument".to_string(),
                ));
            }
            let handle = expect_net_handle(args, 0, "tcp-accept*")?;
            if !handle.is_open() {
                return Err(closed_handle_error(env, "accept", &handle.name));
            }
            let (stream, peer) = handle.tcp_accept().map_err(|e| {
                net_error(
                    env,
                    "accept",
                    classify_io_error(e.kind()),
                    &handle.name,
                    0,
                    &e.to_string(),
                )
            })?;
            let port_val = LispVal::Port(crate::PortObj::tcp_stream(peer.to_string(), stream));
            let peer_val = addr_to_triple(env, peer);
            Ok(LispVal::Cons {
                car: Shared::new(port_val),
                cdr: Shared::new(peer_val),
            })
        }
        BuiltinFunc::TcpShutdown => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "tcp-shutdown* requires exactly two arguments: port how".to_string(),
                ));
            }
            let port = expect_port(args, 0, "tcp-shutdown*")?;
            let how = expect_shutdown_how(args, 1, "tcp-shutdown*")?;
            port.tcp_shutdown(how).map_err(|e| {
                net_error(
                    env,
                    "shutdown",
                    classify_io_error(e.kind()),
                    &port.name,
                    0,
                    &e.to_string(),
                )
            })?;
            Ok(t())
        }
        BuiltinFunc::TcpSetReadTimeout => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "tcp-set-read-timeout* requires exactly two arguments: port ms-or-nil"
                        .to_string(),
                ));
            }
            let port = expect_port(args, 0, "tcp-set-read-timeout*")?;
            let dur = expect_optional_millis(args, 1, "tcp-set-read-timeout*")?;
            port.tcp_set_read_timeout(dur).map_err(|e| {
                net_error(
                    env,
                    "set-read-timeout",
                    "OTHER",
                    &port.name,
                    0,
                    &e.to_string(),
                )
            })?;
            Ok(t())
        }
        BuiltinFunc::TcpSetWriteTimeout => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "tcp-set-write-timeout* requires exactly two arguments: port ms-or-nil"
                        .to_string(),
                ));
            }
            let port = expect_port(args, 0, "tcp-set-write-timeout*")?;
            let dur = expect_optional_millis(args, 1, "tcp-set-write-timeout*")?;
            port.tcp_set_write_timeout(dur).map_err(|e| {
                net_error(
                    env,
                    "set-write-timeout",
                    "OTHER",
                    &port.name,
                    0,
                    &e.to_string(),
                )
            })?;
            Ok(t())
        }

        // ── Generic network-handle operations (listener or UDP socket) ───
        BuiltinFunc::NetHandleClose => {
            let handle = expect_net_handle(args, 0, "net-handle-close*")?;
            // Idempotent: closing an already-closed handle is a silent
            // no-op, mirroring PORT-CLOSE*'s contract.
            handle.close();
            Ok(t())
        }
        BuiltinFunc::NetHandleOpenP => {
            let handle = expect_net_handle(args, 0, "net-handle-open-p*")?;
            Ok(if handle.is_open() { t() } else { LispVal::Nil })
        }
        BuiltinFunc::NetHandleP => {
            if args.len() != 1 {
                return Err(LispError::Generic(
                    "net-handle-p* requires exactly one argument".to_string(),
                ));
            }
            Ok(match &args[0] {
                LispVal::NetHandle(_) => t(),
                _ => LispVal::Nil,
            })
        }
        BuiltinFunc::NetHandleKind => {
            let handle = expect_net_handle(args, 0, "net-handle-kind*")?;
            Ok(LispVal::Symbol(
                env.intern_symbol(&handle.kind.to_uppercase()),
            ))
        }
        BuiltinFunc::NetHandleName => {
            let handle = expect_net_handle(args, 0, "net-handle-name*")?;
            Ok(LispVal::String(handle.name.clone()))
        }

        // ── UDP ─────────────────────────────────────────────────────────
        BuiltinFunc::UdpBind => {
            require_net_listen(env)?;
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "udp-bind* requires exactly two arguments: host port".to_string(),
                ));
            }
            let host = expect_string(args, 0, "udp-bind*")?;
            let port = expect_port_number(args, 1, "udp-bind*")?;
            if !env.check_net_policy(crate::NetOperation::Listen, &host, port) {
                return Err(net_error(
                    env,
                    "bind",
                    "POLICY-DENIED",
                    &host,
                    port as i64,
                    "denied by host network policy",
                ));
            }
            let socket = UdpSocket::bind((host.as_str(), port))
                .map_err(|e| io_net_error(env, "bind", &host, port as i64, e))?;
            let name = socket
                .local_addr()
                .map(|a| a.to_string())
                .unwrap_or_else(|_| format!("{host}:{port}"));
            Ok(LispVal::NetHandle(crate::NetHandleObj::udp_socket(
                name, socket,
            )))
        }
        BuiltinFunc::UdpConnect => {
            require_net_connect(env)?;
            if args.len() != 3 {
                return Err(LispError::Generic(
                    "udp-connect* requires exactly three arguments: socket host port".to_string(),
                ));
            }
            let handle = expect_net_handle(args, 0, "udp-connect*")?;
            let host = expect_string(args, 1, "udp-connect*")?;
            let port = expect_port_number(args, 2, "udp-connect*")?;
            if !handle.is_open() {
                return Err(closed_handle_error(env, "connect", &handle.name));
            }
            if !env.check_net_policy(crate::NetOperation::Connect, &host, port) {
                return Err(net_error(
                    env,
                    "connect",
                    "POLICY-DENIED",
                    &host,
                    port as i64,
                    "denied by host network policy",
                ));
            }
            handle
                .with_udp_socket(|s| s.connect((host.as_str(), port)))
                .map_err(|e| io_net_error(env, "connect", &host, port as i64, e))?;
            Ok(t())
        }
        BuiltinFunc::UdpSendTo => {
            require_net_connect(env)?;
            if args.len() != 4 {
                return Err(LispError::Generic(
                    "udp-send-to* requires exactly four arguments: socket host port bytes"
                        .to_string(),
                ));
            }
            let handle = expect_net_handle(args, 0, "udp-send-to*")?;
            let host = expect_string(args, 1, "udp-send-to*")?;
            let port = expect_port_number(args, 2, "udp-send-to*")?;
            let bytes = get_char_array_bytes(&args[3], "udp-send-to*")?;
            if !handle.is_open() {
                return Err(closed_handle_error(env, "send-to", &handle.name));
            }
            if !env.check_net_policy(crate::NetOperation::Connect, &host, port) {
                return Err(net_error(
                    env,
                    "send-to",
                    "POLICY-DENIED",
                    &host,
                    port as i64,
                    "denied by host network policy",
                ));
            }
            let n = handle
                .with_udp_socket(|s| s.send_to(&bytes, (host.as_str(), port)))
                .map_err(|e| io_net_error(env, "send-to", &host, port as i64, e))?;
            Ok(LispVal::Number(n as i64))
        }
        BuiltinFunc::UdpSend => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "udp-send* requires exactly two arguments: socket bytes".to_string(),
                ));
            }
            let handle = expect_net_handle(args, 0, "udp-send*")?;
            let bytes = get_char_array_bytes(&args[1], "udp-send*")?;
            if !handle.is_open() {
                return Err(closed_handle_error(env, "send", &handle.name));
            }
            let n = handle.with_udp_socket(|s| s.send(&bytes)).map_err(|e| {
                net_error(
                    env,
                    "send",
                    classify_io_error(e.kind()),
                    &handle.name,
                    0,
                    &e.to_string(),
                )
            })?;
            Ok(LispVal::Number(n as i64))
        }
        BuiltinFunc::UdpReceiveFrom => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "udp-receive-from* requires exactly two arguments: socket maxlen".to_string(),
                ));
            }
            let handle = expect_net_handle(args, 0, "udp-receive-from*")?;
            let maxlen = expect_nonneg(args, 1, "udp-receive-from*")?;
            if !handle.is_open() {
                return Err(closed_handle_error(env, "receive-from", &handle.name));
            }
            let mut buf = vec![0u8; maxlen];
            let (n, peer) = handle
                .with_udp_socket(|s| s.recv_from(&mut buf))
                .map_err(|e| {
                    net_error(
                        env,
                        "receive-from",
                        classify_io_error(e.kind()),
                        &handle.name,
                        0,
                        &e.to_string(),
                    )
                })?;
            buf.truncate(n);
            // Datagram-boundary truncation is inherently ambiguous with the
            // plain std::net API (no MSG_TRUNC without raw syscalls, which
            // this issue's zero-new-dependency, no-ioctl scope excludes):
            // if the received length equals the requested buffer size, the
            // caller cannot distinguish "exactly that many bytes" from
            // "truncated" -- documented here and in lib/39-udp.lisp; pass a
            // buffer larger than the expected payload to disambiguate.
            let possibly_truncated = n == maxlen && maxlen > 0;
            let bytes_val = bytes_to_char_array(buf);
            let peer_val = addr_to_triple(env, peer);
            let trunc_val = if possibly_truncated {
                t()
            } else {
                LispVal::Nil
            };
            Ok(LispVal::Cons {
                car: Shared::new(bytes_val),
                cdr: Shared::new(LispVal::Cons {
                    car: Shared::new(peer_val),
                    cdr: Shared::new(LispVal::Cons {
                        car: Shared::new(trunc_val),
                        cdr: Shared::new(LispVal::Nil),
                    }),
                }),
            })
        }
        BuiltinFunc::UdpSetTimeout => {
            if args.len() != 2 {
                return Err(LispError::Generic(
                    "udp-set-timeout* requires exactly two arguments: socket ms-or-nil".to_string(),
                ));
            }
            let handle = expect_net_handle(args, 0, "udp-set-timeout*")?;
            let dur = expect_optional_millis(args, 1, "udp-set-timeout*")?;
            if !handle.is_open() {
                return Err(closed_handle_error(env, "set-timeout", &handle.name));
            }
            handle
                .with_udp_socket(|s| {
                    s.set_read_timeout(dur)?;
                    s.set_write_timeout(dur)
                })
                .map_err(|e| {
                    net_error(env, "set-timeout", "OTHER", &handle.name, 0, &e.to_string())
                })?;
            Ok(t())
        }

        _ => Err(LispError::Generic("Not a networking operation".to_string())),
    }
}
