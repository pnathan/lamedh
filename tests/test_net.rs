//! Capability-gated DNS, TCP, and UDP over binary ports (issue #258, epic
//! #253): the kernel NET-*/TCP-*/UDP-* primitives
//! (src/evaluator/builtins_net.rs, NetHandleObj/PortState::TcpStream in
//! src/lib.rs) wrapped by lib/37-net.lisp/lib/38-tcp.lisp/lib/39-udp.lisp.
//!
//! Coverage: capability gating (NET-DNS/NET-CONNECT/NET-LISTEN independently
//! enforced) and fence attenuation, the host policy hook, DNS resolution
//! (offline-safe only -- localhost and a guaranteed-invalid name, never
//! external network dependencies), TCP client/server round-trip over
//! loopback (peers are plain std::net on a spawned std::thread, per the
//! epic's synchronous single-threaded-evaluator model), accept's peer
//! address, shutdown semantics, read timeouts, bind conflicts, UDP
//! bind/send-to/receive-from round-trip and datagram-boundary preservation,
//! use-after-close rejection, and the Drop backstop.
//!
//! CRITICAL TEST HYGIENE: every bind uses port 0 (OS-assigned) -- no
//! hardcoded ports anywhere, including the bind-conflict test (which binds
//! a first listener to port 0, then deliberately reuses that OS-assigned
//! port for the conflict).

use lamedh::environment::Environment;
use lamedh::{LispVal, NetOperation, Shared, eval_line, eval_str};
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, UdpSocket};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

fn env_with_net() -> Shared<Environment> {
    Environment::with_stdlib()
}

fn grant(env: &Shared<Environment>, caps: &[&str]) {
    for c in caps {
        env.enable_feature(c);
    }
}

// ── Capability gating ───────────────────────────────────────────────────

#[test]
fn net_resolve_requires_net_dns() {
    let env = env_with_net();
    let out = eval_line("(net:resolve \"localhost\")", &env);
    assert!(
        out.contains("NET-DNS capability") && out.contains("not enabled"),
        "got: {out}"
    );
}

#[test]
fn tcp_connect_requires_net_connect() {
    let env = env_with_net();
    let out = eval_line("(tcp:connect \"127.0.0.1\" 1)", &env);
    assert!(
        out.contains("NET-CONNECT capability") && out.contains("not enabled"),
        "got: {out}"
    );
}

#[test]
fn tcp_listen_requires_net_listen() {
    let env = env_with_net();
    let out = eval_line("(tcp:listen \"127.0.0.1\" 0)", &env);
    assert!(
        out.contains("NET-LISTEN capability") && out.contains("not enabled"),
        "got: {out}"
    );
}

#[test]
fn udp_bind_requires_net_listen() {
    let env = env_with_net();
    let out = eval_line("(udp:bind \"127.0.0.1\" 0)", &env);
    assert!(
        out.contains("NET-LISTEN capability") && out.contains("not enabled"),
        "got: {out}"
    );
}

#[test]
fn udp_send_to_requires_net_connect_even_with_listen_granted() {
    // Binding (NET-LISTEN) does not imply send-to authority (NET-CONNECT) --
    // the two capabilities are independently enforced, per the issue.
    let env = env_with_net();
    grant(&env, &["NET-LISTEN"]);
    let out = eval_line(
        "(let ((s (udp:bind \"127.0.0.1\" 0))) (udp:send-to s \"127.0.0.1\" 1 (list->array '(1 2 3))))",
        &env,
    );
    assert!(
        out.contains("NET-CONNECT capability") && out.contains("not enabled"),
        "got: {out}"
    );
}

#[test]
fn capabilities_are_independently_enforced() {
    // Granting NET-DNS alone must not unlock NET-CONNECT/NET-LISTEN.
    let env = env_with_net();
    grant(&env, &["NET-DNS"]);
    let out = eval_line("(net:resolve \"localhost\")", &env);
    assert!(!out.contains("capability"), "resolve should work: {out}");
    let out = eval_line("(tcp:connect \"127.0.0.1\" 1)", &env);
    assert!(out.contains("NET-CONNECT capability"), "got: {out}");
    let out = eval_line("(tcp:listen \"127.0.0.1\" 0)", &env);
    assert!(out.contains("NET-LISTEN capability"), "got: {out}");
}

#[test]
fn fence_attenuates_net_connect_even_with_cli_grant() {
    // issue #258's required fence-attenuation probe: a networking operation
    // inside (with-capabilities () ...) must fail even though the host
    // granted the capability, exactly like every other gated builtin
    // (#320/#325, and issue #255's identical probe for ports).
    let env = env_with_net();
    grant(&env, &["NET-CONNECT"]);
    let out = eval_line(
        "(with-capabilities '() (tcp-connect* \"127.0.0.1\" 1 100))",
        &env,
    );
    assert!(
        out.contains("capability denied: NET-CONNECT") && out.contains("attenuated"),
        "got: {out}"
    );
}

#[test]
fn fence_attenuates_net_listen_and_net_dns_too() {
    let env = env_with_net();
    grant(&env, &["NET-LISTEN", "NET-DNS"]);
    let out = eval_line(
        "(with-capabilities '() (tcp-listen* \"127.0.0.1\" 0 16))",
        &env,
    );
    assert!(out.contains("capability denied: NET-LISTEN"), "got: {out}");
    let out = eval_line("(with-capabilities '() (net-resolve* \"localhost\"))", &env);
    assert!(out.contains("capability denied: NET-DNS"), "got: {out}");
}

#[test]
fn new_sandboxed_has_no_net_capabilities_by_default() {
    let env = Environment::new_sandboxed();
    assert!(!env.feature_enabled("NET-DNS"));
    assert!(!env.feature_enabled("NET-CONNECT"));
    assert!(!env.feature_enabled("NET-LISTEN"));
}

// ── Host policy hook ────────────────────────────────────────────────────

#[test]
fn policy_hook_denies_even_with_capability_granted() {
    let env = env_with_net();
    grant(&env, &["NET-CONNECT"]);
    env.set_net_policy(|op, _host, _port| op != NetOperation::Connect);
    let out = eval_line("(tcp:connect \"127.0.0.1\" 1)", &env);
    assert!(
        out.contains(":POLICY-DENIED") || out.contains("POLICY-DENIED"),
        "got: {out}"
    );
    assert!(out.contains("denied by host network policy"), "got: {out}");
}

#[test]
fn policy_hook_can_scope_by_host() {
    // The canonical use case from the issue: an HTTP-client-shaped grant
    // that must not become unrestricted SSRF authority.
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener); // free the port; we only wanted an ephemeral number

    let env = env_with_net();
    grant(&env, &["NET-CONNECT"]);
    env.set_net_policy(|op, host, _port| {
        !(op == NetOperation::Connect && host == "169.254.169.254")
    });
    let out = eval_line(
        &format!("(tcp:connect \"169.254.169.254\" {port} 50)"),
        &env,
    );
    assert!(out.contains("POLICY-DENIED"), "got: {out}");
}

#[test]
fn no_policy_installed_allows_by_default() {
    let env = env_with_net();
    grant(&env, &["NET-DNS"]);
    let out = eval_line("(net:resolve \"localhost\")", &env);
    assert!(!out.contains("POLICY-DENIED"), "got: {out}");
}

// ── DNS (offline-safe only) ──────────────────────────────────────────────

#[test]
fn resolve_localhost_returns_addresses() {
    let env = env_with_net();
    grant(&env, &["NET-DNS"]);
    let result = eval_str("(net:resolve \"localhost\")", &env).unwrap();
    let items = list_items(&result);
    assert!(!items.is_empty(), "expected at least one address");
    for addr in &items {
        assert!(matches!(addr, LispVal::Struct(_)));
    }
}

#[test]
fn resolve_invalid_host_is_structured_dns_error() {
    let env = env_with_net();
    grant(&env, &["NET-DNS"]);
    let out = eval_line(
        "(handler-case (net:resolve \"\") (error (e) (list ':caught (cdr (assoc ':category (error-data e))))))",
        &env,
    );
    assert!(
        out.contains(":CAUGHT") && out.contains(":DNS"),
        "got: {out}"
    );
}

// ── TCP: Lisp connects to a plain-Rust server thread ─────────────────────

#[test]
fn tcp_connect_round_trip_to_rust_server() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (mut stream, _peer) = listener.accept().unwrap();
        let mut buf = [0u8; 5];
        stream.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"hello");
        stream.write_all(b"world").unwrap();
    });

    let env = env_with_net();
    grant(&env, &["NET-CONNECT"]);
    let out = eval_line(
        &format!(
            "(let ((p (tcp:connect \"127.0.0.1\" {port})))
               (ports:write-bytes! p (text:string->utf8 \"hello\"))
               (ports:flush! p)
               (let ((reply (text:utf8->string (ports:read-bytes! p 5))))
                 (ports:close! p)
                 reply))"
        ),
        &env,
    );
    assert_eq!(out, "\"world\"", "got: {out}");
    server.join().unwrap();
}

#[test]
fn tcp_connect_result_is_an_ordinary_port() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let _ = listener.accept().unwrap();
    });
    let env = env_with_net();
    grant(&env, &["NET-CONNECT"]);
    let out = eval_line(
        &format!("(port-p* (tcp:connect \"127.0.0.1\" {port}))"),
        &env,
    );
    assert_eq!(out, "T", "got: {out}");
    server.join().unwrap();
}

#[test]
fn tcp_ipv6_loopback_round_trip() {
    // IPv6 loopback is not guaranteed available in every sandboxed CI
    // environment; skip gracefully rather than failing the suite if the OS
    // refuses the bind (AddrNotAvailable and similar), per the acceptance
    // criteria's "IPv4, IPv6, ... cases are covered" without depending on
    // external network access.
    let listener = match TcpListener::bind("[::1]:0") {
        Ok(l) => l,
        Err(e) => {
            eprintln!("skipping tcp_ipv6_loopback_round_trip: IPv6 loopback unavailable: {e}");
            return;
        }
    };
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        let (mut stream, _peer) = listener.accept().unwrap();
        let mut buf = [0u8; 4];
        stream.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"ipv6");
    });

    let env = env_with_net();
    grant(&env, &["NET-CONNECT"]);
    let out = eval_line(
        &format!(
            "(let ((p (tcp:connect \"::1\" {port})))
               (ports:write-bytes! p (text:string->utf8 \"ipv6\"))
               (ports:flush! p)
               (let ((family (net:address-family (net:peer-addr p))))
                 (ports:close! p)
                 family))"
        ),
        &env,
    );
    assert_eq!(out, ":IPV6", "got: {out}");
    server.join().unwrap();
}

#[test]
fn udp_ipv6_loopback_round_trip() {
    let a = match UdpSocket::bind("[::1]:0") {
        Ok(s) => s,
        Err(e) => {
            eprintln!("skipping udp_ipv6_loopback_round_trip: IPv6 loopback unavailable: {e}");
            return;
        }
    };
    let a_port = a.local_addr().unwrap().port();
    let peer_thread = std::thread::spawn(move || {
        let mut buf = [0u8; 16];
        let (n, from) = a.recv_from(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"v6ping");
        a.send_to(b"v6pong", from).unwrap();
    });

    let env = env_with_net();
    grant(&env, &["NET-LISTEN", "NET-CONNECT"]);
    let out = eval_line(
        &format!(
            "(let ((s (udp:bind \"::1\" 0)))
               (udp:send-to s \"::1\" {a_port} (text:string->utf8 \"v6ping\"))
               (let ((result (udp:receive-from s 16)))
                 (udp:close! s)
                 (list (text:utf8->string (car result)) (net:address-family (cadr result)))))"
        ),
        &env,
    );
    assert!(out.contains("\"v6pong\""), "got: {out}");
    assert!(out.contains(":IPV6"), "got: {out}");
    peer_thread.join().unwrap();
}

#[test]
fn tcp_connect_refused_is_structured_error() {
    // Bind, discover the ephemeral port, then close it -- nothing is
    // listening there, so the connect attempt should be refused near-
    // instantly (no hardcoded port; test-hygiene requirement).
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let env = env_with_net();
    grant(&env, &["NET-CONNECT"]);
    let out = eval_line(
        &format!(
            "(handler-case (tcp:connect \"127.0.0.1\" {port})
               (error (e) (list ':caught (cdr (assoc ':category (error-data e))))))"
        ),
        &env,
    );
    assert!(out.contains(":CAUGHT"), "got: {out}");
    assert!(
        out.contains(":REFUSED") || out.contains(":OTHER"),
        "got: {out}"
    );
}

#[test]
fn tcp_connect_timeout_fires() {
    // A private/reserved address that (almost always) silently black-holes
    // SYN packets would be network-dependent and flaky; instead, use a
    // loopback listener with a full accept backlog substitute: connect to
    // a bound-but-never-accepting listener is not guaranteed to time out on
    // loopback either. Use the documented-safe TEST-NET block instead
    // (192.0.2.0/24, RFC 5737) with a short timeout, which routers drop
    // without a response -- offline-safe (no real host is ever reached) and
    // reliably slow rather than instantly refused.
    let env = env_with_net();
    grant(&env, &["NET-CONNECT"]);
    let start = std::time::Instant::now();
    let out = eval_line(
        "(handler-case (tcp:connect \"192.0.2.1\" 9 300)
           (error (e) (list ':caught (cdr (assoc ':category (error-data e))))))",
        &env,
    );
    let elapsed = start.elapsed();
    assert!(out.contains(":CAUGHT"), "got: {out}");
    assert!(
        elapsed < Duration::from_secs(5),
        "connect-timeout took too long: {elapsed:?}"
    );
}

// ── TCP: Lisp listens, a plain-Rust client thread connects ───────────────

#[test]
fn tcp_listen_accept_round_trip_from_rust_client() {
    let env = env_with_net();
    grant(&env, &["NET-LISTEN"]);
    let local_port = eval_str(
        "(let ((l (tcp:listen \"127.0.0.1\" 0)))
           (set '$test-listener l)
           (net:address-port (tcp:local-addr l)))",
        &env,
    )
    .unwrap();
    let port = match local_port {
        LispVal::Number(n) => n as u16,
        other => panic!("expected a port number, got {other:?}"),
    };

    let client = std::thread::spawn(move || {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        stream.write_all(b"ping").unwrap();
        let mut buf = [0u8; 4];
        stream.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, b"pong");
    });

    let out = eval_line(
        "(let* ((pair (tcp:accept $test-listener))
                (p (car pair))
                (peer (cdr pair)))
           (let ((msg (text:utf8->string (ports:read-bytes! p 4))))
             (ports:write-bytes! p (text:string->utf8 \"pong\"))
             (ports:flush! p)
             (ports:close! p)
             (list msg (net:address-family peer) (net:address-ip peer))))",
        &env,
    );
    assert!(out.contains("\"ping\""), "got: {out}");
    assert!(out.contains(":IPV4"), "got: {out}");
    assert!(out.contains("\"127.0.0.1\""), "got: {out}");
    client.join().unwrap();
}

#[test]
fn tcp_shutdown_write_sends_eof_to_peer() {
    let env = env_with_net();
    grant(&env, &["NET-LISTEN"]);
    let local_port = eval_str(
        "(let ((l (tcp:listen \"127.0.0.1\" 0)))
           (set '$test-listener2 l)
           (net:address-port (tcp:local-addr l)))",
        &env,
    )
    .unwrap();
    let port = match local_port {
        LispVal::Number(n) => n as u16,
        other => panic!("expected a port number, got {other:?}"),
    };

    let client = std::thread::spawn(move || {
        let mut stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        let mut buf = Vec::new();
        // The peer shuts down its write half after sending this; read_to_end
        // must observe EOF instead of blocking forever.
        stream.read_to_end(&mut buf).unwrap();
        assert_eq!(buf, b"bye");
    });

    let out = eval_line(
        "(let* ((pair (tcp:accept $test-listener2))
                (p (car pair)))
           (ports:write-bytes! p (text:string->utf8 \"bye\"))
           (ports:flush! p)
           (tcp:shutdown! p ':write)
           (ports:close! p)
           'done)",
        &env,
    );
    assert_eq!(out, "DONE", "got: {out}");
    client.join().unwrap();
}

#[test]
fn tcp_read_timeout_fires_without_hanging() {
    let env = env_with_net();
    grant(&env, &["NET-LISTEN"]);
    let local_port = eval_str(
        "(let ((l (tcp:listen \"127.0.0.1\" 0)))
           (set '$test-listener3 l)
           (net:address-port (tcp:local-addr l)))",
        &env,
    )
    .unwrap();
    let port = match local_port {
        LispVal::Number(n) => n as u16,
        other => panic!("expected a port number, got {other:?}"),
    };

    // A client that connects and then never sends anything.
    let stop = Arc::new(AtomicBool::new(false));
    let stop2 = stop.clone();
    let client = std::thread::spawn(move || {
        let stream = TcpStream::connect(("127.0.0.1", port)).unwrap();
        while !stop2.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(20));
        }
        drop(stream);
    });

    let start = std::time::Instant::now();
    let out = eval_line(
        "(let* ((pair (tcp:accept $test-listener3))
                (p (car pair)))
           (tcp:set-read-timeout! p 200)
           (handler-case (ports:read-byte! p)
             (error (e) (progn (ports:close! p) (list ':caught (cdr (assoc ':category (error-data e))))))))",
        &env,
    );
    let elapsed = start.elapsed();
    stop.store(true, Ordering::Relaxed);
    client.join().unwrap();
    assert!(out.contains(":CAUGHT"), "got: {out}");
    assert!(out.contains(":TIMEOUT"), "got: {out}");
    assert!(
        elapsed < Duration::from_secs(3),
        "read-timeout took too long: {elapsed:?}"
    );
}

#[test]
fn tcp_bind_conflict_is_structured_error() {
    let env = env_with_net();
    grant(&env, &["NET-LISTEN"]);
    // Bind the first listener to an OS-assigned port (never hardcoded),
    // then deliberately reuse that exact port for the conflicting bind.
    let first_port = eval_str(
        "(let ((l (tcp:listen \"127.0.0.1\" 0)))
           (set '$test-listener4 l)
           (net:address-port (tcp:local-addr l)))",
        &env,
    )
    .unwrap();
    let port = match first_port {
        LispVal::Number(n) => n,
        other => panic!("expected a port number, got {other:?}"),
    };
    let out = eval_line(
        &format!(
            "(handler-case (tcp:listen \"127.0.0.1\" {port})
               (error (e) (list ':caught (cdr (assoc ':category (error-data e))))))"
        ),
        &env,
    );
    assert!(out.contains(":CAUGHT"), "got: {out}");
    assert!(out.contains(":ADDR-IN-USE"), "got: {out}");
}

#[test]
fn tcp_accept_and_ops_reject_use_after_close() {
    let env = env_with_net();
    grant(&env, &["NET-LISTEN"]);
    let out = eval_line(
        "(let ((l (tcp:listen \"127.0.0.1\" 0)))
           (tcp:close-listener! l)
           (tcp:close-listener! l) ; idempotent
           (list (tcp:listener-open-p l)
                 (handler-case (tcp:accept l) (error (e) ':caught))))",
        &env,
    );
    assert!(out.contains("()") && out.contains(":CAUGHT"), "got: {out}");
}

// ── UDP ──────────────────────────────────────────────────────────────────

#[test]
fn udp_send_to_receive_from_round_trip_with_rust_peer() {
    let peer = UdpSocket::bind("127.0.0.1:0").unwrap();
    let peer_port = peer.local_addr().unwrap().port();
    let peer_thread = std::thread::spawn(move || {
        let mut buf = [0u8; 32];
        let (n, from) = peer.recv_from(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"ping");
        peer.send_to(b"pong", from).unwrap();
    });

    let env = env_with_net();
    grant(&env, &["NET-LISTEN", "NET-CONNECT"]);
    let out = eval_line(
        &format!(
            "(let ((s (udp:bind \"127.0.0.1\" 0)))
               (udp:send-to s \"127.0.0.1\" {peer_port} (text:string->utf8 \"ping\"))
               (let ((result (udp:receive-from s 32)))
                 (udp:close! s)
                 (list (text:utf8->string (car result)) (caddr result))))"
        ),
        &env,
    );
    assert!(out.contains("\"pong\""), "got: {out}");
    assert!(out.contains("()"), "got (should not be truncated): {out}");
    peer_thread.join().unwrap();
}

#[test]
fn udp_connect_then_send_round_trip() {
    let peer = UdpSocket::bind("127.0.0.1:0").unwrap();
    let peer_port = peer.local_addr().unwrap().port();
    let peer_thread = std::thread::spawn(move || {
        let mut buf = [0u8; 32];
        let (n, from) = peer.recv_from(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"connected-ping");
        peer.send_to(b"ack", from).unwrap();
    });

    let env = env_with_net();
    grant(&env, &["NET-LISTEN", "NET-CONNECT"]);
    let out = eval_line(
        &format!(
            "(let ((s (udp:bind \"127.0.0.1\" 0)))
               (udp:connect! s \"127.0.0.1\" {peer_port})
               (udp:send s (text:string->utf8 \"connected-ping\"))
               (let ((result (udp:receive-from s 32)))
                 (udp:close! s)
                 (text:utf8->string (car result))))"
        ),
        &env,
    );
    assert_eq!(out, "\"ack\"", "got: {out}");
    peer_thread.join().unwrap();
}

#[test]
fn udp_datagram_boundaries_are_preserved() {
    let env = env_with_net();
    grant(&env, &["NET-LISTEN", "NET-CONNECT"]);
    let out = eval_line(
        "(let ((a (udp:bind \"127.0.0.1\" 0))
               (b (udp:bind \"127.0.0.1\" 0)))
           (let ((a-port (net:address-port (udp:local-addr a))))
             (udp:send-to b \"127.0.0.1\" a-port (text:string->utf8 \"first\"))
             (udp:send-to b \"127.0.0.1\" a-port (text:string->utf8 \"second-datagram\"))
             (let ((r1 (udp:receive-from a 64))
                   (r2 (udp:receive-from a 64)))
               (udp:close! a)
               (udp:close! b)
               (list (text:utf8->string (car r1)) (text:utf8->string (car r2))))))",
        &env,
    );
    // Two independent sends must arrive as two independent, correctly
    // ordered/bounded datagrams -- never coalesced.
    assert!(out.contains("\"first\""), "got: {out}");
    assert!(out.contains("\"second-datagram\""), "got: {out}");
}

#[test]
fn udp_receive_from_flags_possible_truncation() {
    let env = env_with_net();
    grant(&env, &["NET-LISTEN", "NET-CONNECT"]);
    let out = eval_line(
        "(let ((a (udp:bind \"127.0.0.1\" 0))
               (b (udp:bind \"127.0.0.1\" 0)))
           (let ((a-port (net:address-port (udp:local-addr a))))
             (udp:send-to b \"127.0.0.1\" a-port (text:string->utf8 \"toolong\"))
             (let ((result (udp:receive-from a 3)))
               (udp:close! a)
               (udp:close! b)
               (list (text:utf8->string (car result)) (caddr result)))))",
        &env,
    );
    // Buffer smaller than the datagram: length == maxlen, so the
    // possibly-truncated flag must be true.
    assert!(out.contains("\"too\""), "got: {out}");
    assert!(out.contains(" T)"), "got: {out}");
}

#[test]
fn udp_ops_reject_use_after_close() {
    let env = env_with_net();
    grant(&env, &["NET-LISTEN", "NET-CONNECT"]);
    let out = eval_line(
        "(let ((s (udp:bind \"127.0.0.1\" 0)))
           (udp:close! s)
           (udp:close! s) ; idempotent
           (list (udp:socket-open-p s)
                 (handler-case (udp:send-to s \"127.0.0.1\" 1 (list->array '(1)))
                   (error (e) ':caught))
                 (handler-case (udp:receive-from s 8)
                   (error (e) ':caught))))",
        &env,
    );
    assert!(
        out.contains("()") && out.matches(":CAUGHT").count() == 2,
        "got: {out}"
    );
}

// ── Drop backstop ────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
fn open_fd_count() -> usize {
    std::fs::read_dir("/proc/self/fd")
        .map(|d| d.count())
        .unwrap_or(0)
}

#[cfg(target_os = "linux")]
#[test]
fn dropping_an_unclosed_listener_still_releases_its_fd() {
    let env = env_with_net();
    grant(&env, &["NET-LISTEN"]);
    let before = open_fd_count();
    for _ in 0..200 {
        eval_line("(tcp:listen \"127.0.0.1\" 0)", &env);
    }
    let after = open_fd_count();
    assert!(
        after <= before + 5,
        "fds leaked: before={before} after={after} (Drop backstop not releasing TcpListener)"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn dropping_an_unclosed_udp_socket_still_releases_its_fd() {
    let env = env_with_net();
    grant(&env, &["NET-LISTEN"]);
    let before = open_fd_count();
    for _ in 0..200 {
        eval_line("(udp:bind \"127.0.0.1\" 0)", &env);
    }
    let after = open_fd_count();
    assert!(
        after <= before + 5,
        "fds leaked: before={before} after={after} (Drop backstop not releasing UdpSocket)"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn dropping_an_unclosed_tcp_stream_still_releases_its_fd() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        for _ in 0..50 {
            if let Ok((s, _)) = listener.accept() {
                drop(s);
            } else {
                break;
            }
        }
    });

    let env = env_with_net();
    grant(&env, &["NET-CONNECT"]);
    let before = open_fd_count();
    for _ in 0..50 {
        eval_line(&format!("(tcp:connect \"127.0.0.1\" {port})"), &env);
    }
    let after = open_fd_count();
    assert!(
        after <= before + 5,
        "fds leaked: before={before} after={after} (Drop backstop not releasing TcpStream)"
    );
    drop(TcpStream::connect(("127.0.0.1", port))); // nudge accept loop past its last iteration if still waiting
    let _ = server.join();
}

// ── TLS off-behavior (issue #365) ───────────────────────────────────────
//
// With the `net-tls` cargo feature compiled out, `(require 'tls)` still
// loads cleanly (lib/43-tls.lisp is embedded like every other optional
// module, regardless of the feature -- see its own file header),
// `tls:available-p` reports NIL, and every other TLS:* operation signals a
// structured `:CATEGORY :TLS-UNAVAILABLE` error instead of doing any TLS
// work. See tests/test_tls.rs for the feature-ON behavior (loopback TLS
// round trips) -- this test is necessarily feature-off-only, since with
// net-tls on, TLS:AVAILABLE-P is T and TLS:WRAP-CLIENT would instead fail
// differently (a bad-port-argument error, not :TLS-UNAVAILABLE).

#[cfg(not(feature = "net-tls"))]
#[test]
fn tls_module_loads_but_every_operation_is_unavailable_without_the_net_tls_feature() {
    let env = env_with_net();
    assert_eq!(eval_line("(require 'tls)", &env), "TLS");
    let available = eval_line("(tls:available-p)", &env);
    assert!(available == "NIL" || available == "()", "got: {available}");
    let out = eval_line(
        "(handler-case (tls:wrap-client nil :hostname \"example.com\")
            (error (e) (cdr (assoc ':category (error-data e)))))",
        &env,
    );
    assert_eq!(out, ":TLS-UNAVAILABLE", "got: {out}");
    let out2 = eval_line(
        "(handler-case (tls:wrap-client-insecure! nil :hostname \"example.com\")
            (error (e) (cdr (assoc ':category (error-data e)))))",
        &env,
    );
    assert_eq!(out2, ":TLS-UNAVAILABLE", "got: {out2}");
}

// ── helpers ──────────────────────────────────────────────────────────────

fn list_items(v: &LispVal) -> Vec<LispVal> {
    let mut out = Vec::new();
    let mut cur = v.clone();
    loop {
        match cur {
            LispVal::Nil => break,
            LispVal::Cons { car, cdr } => {
                out.push(car.as_ref().clone());
                cur = cdr.as_ref().clone();
            }
            _ => break,
        }
    }
    out
}
