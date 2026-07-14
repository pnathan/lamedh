#![cfg(feature = "net-tls")]
//! TLS port wrapping (issue #365, epic #253): lib/43-tls.lisp over
//! src/evaluator/builtins_tls.rs / `PortState::TlsClient`/`TlsServer`
//! (src/lib.rs), behind the off-by-default `net-tls` cargo feature. This
//! whole file only exists/runs with `--features net-tls`; feature-OFF
//! behavior (the TLS module still loads, but every operation except
//! `tls:available-p` signals a structured `:tls-unavailable` error, and
//! https:// keeps its pre-#365 structured error) is covered in the normal
//! suites instead -- see `tests/test_net.rs`'s
//! `tls_module_loads_but_every_operation_is_unavailable_without_the_net_tls_feature`
//! and `tests/test_http.rs`'s `https_url_is_a_clear_structured_error_naming_issue_365`
//! (both `#[cfg(not(feature = "net-tls"))]`).
//!
//! Loopback-only, no external network dependency: a throwaway self-signed
//! CA + a "localhost" leaf certificate, freshly generated per test via the
//! `rcgen` dev-dependency (never a build/normal dependency -- see
//! Cargo.toml). Both peers are our own `tls:` module, each in its own
//! independent `Environment::with_stdlib()` on its own OS thread (a fresh
//! `Environment` never crosses a thread boundary, only raw TCP bytes do),
//! so these tests exercise `tls:wrap-server` and `tls:wrap-client`/
//! `tls:connect` against each other -- both directions of issue #365's
//! "client wrap of a connected TCP port" and "server-side TLS wrapping for
//! accepted TCP ports".
//!
//! Coverage: handshake + byte round trip, negotiated ALPN inspection,
//! peer-certificate summary, verification FAILURE against an unknown CA
//! (default root store, no `:extra-roots`), `tls:connect-insecure!` denied
//! without the host opt-in and working with it, and `https://` end-to-end
//! through `http:request` (`:extra-roots` trusting the same throwaway CA).

use lamedh::environment::Environment;
use lamedh::{Shared, eval_line, eval_str};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc;

// ── Throwaway self-signed CA + "localhost" leaf cert ──────────────────────

static DIR_COUNTER: AtomicU64 = AtomicU64::new(0);

/// A PKI fixture written to a scratch directory under the OS temp dir,
/// removed on drop. Paths are absolute `PathBuf`s so a spawned thread can
/// move them freely.
struct TestPki {
    dir: PathBuf,
    ca_pem: PathBuf,
    cert_pem: PathBuf,
    key_pem: PathBuf,
}

impl Drop for TestPki {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn make_test_pki() -> TestPki {
    let ca_key = rcgen::KeyPair::generate().expect("generate CA key");
    let mut ca_params = rcgen::CertificateParams::new(Vec::<String>::new()).expect("CA params");
    ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    let ca_cert = ca_params.self_signed(&ca_key).expect("self-sign CA");

    let leaf_key = rcgen::KeyPair::generate().expect("generate leaf key");
    let leaf_params =
        rcgen::CertificateParams::new(vec!["localhost".to_string()]).expect("leaf params");
    let leaf_cert = leaf_params
        .signed_by(&leaf_key, &ca_cert, &ca_key)
        .expect("sign leaf with CA");

    let n = DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("lamedh-tls-test-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create scratch dir");
    let ca_pem = dir.join("ca.pem");
    let cert_pem = dir.join("leaf-cert.pem");
    let key_pem = dir.join("leaf-key.pem");
    std::fs::write(&ca_pem, ca_cert.pem()).expect("write ca.pem");
    std::fs::write(&cert_pem, leaf_cert.pem()).expect("write leaf-cert.pem");
    std::fs::write(&key_pem, leaf_key.serialize_pem()).expect("write leaf-key.pem");

    TestPki {
        dir,
        ca_pem,
        cert_pem,
        key_pem,
    }
}

fn lisp_path(p: &Path) -> String {
    // A Rust Debug-quoted string is a valid Lamedh string literal for any
    // path this fixture produces (plain ASCII temp-dir names, no embedded
    // quotes).
    format!("{:?}", p.display().to_string())
}

// ── Env helpers ────────────────────────────────────────────────────────

fn env_with_tls() -> Shared<Environment> {
    let env = Environment::with_stdlib();
    assert_eq!(eval_line("(require 'tls)", &env), "TLS");
    env
}

fn grant(env: &Shared<Environment>, caps: &[&str]) {
    for c in caps {
        env.enable_feature(c);
    }
}

/// Spawn a TLS echo server on its own thread/environment: accepts exactly
/// one connection on an OS-assigned loopback port, wraps it with
/// `tls:wrap-server` using `pki`'s cert/key, reads exactly 5 bytes,
/// upper-cases and echoes them back, then closes. Returns the bound port
/// and the thread's `JoinHandle` (join it after the client side finishes).
fn spawn_tls_echo_server(
    cert: &Path,
    key: &Path,
    alpn: &[&str],
) -> (u16, std::thread::JoinHandle<()>) {
    let (tx, rx) = mpsc::channel();
    let cert = lisp_path(cert);
    let key = lisp_path(key);
    let alpn_list = format!(
        "(list {})",
        alpn.iter()
            .map(|p| format!("{p:?}"))
            .collect::<Vec<_>>()
            .join(" ")
    );
    let handle = std::thread::spawn(move || {
        let env = env_with_tls();
        grant(&env, &["NET-LISTEN", "READ-FS"]);
        eval_str("(def $srv (tcp:listen \"127.0.0.1\" 0))", &env).unwrap();
        let port: u16 = eval_line("(net:address-port (tcp:local-addr $srv))", &env)
            .parse()
            .expect("listener port");
        tx.send(port).unwrap();
        // A read timeout on the plaintext port (inherited by the TLS wrap's
        // underlying socket) bounds the handshake so a client that connects
        // at the TCP level but never completes a handshake (e.g. an
        // insecure-connect attempt this test suite denies before any TLS
        // bytes are sent) fails this thread promptly with a structured
        // :TIMEOUT error instead of blocking it -- and `server.join()` with
        // it -- forever.
        let code = format!(
            "(let* ((accepted (tcp:accept $srv))
                     (plain (car accepted)))
               (tcp:set-read-timeout! plain 20000)
               (let ((tls (tls:wrap-server plain {cert} {key} :alpn {alpn_list})))
                 (let ((msg (text:utf8->string (ports:read-bytes! tls 5))))
                   (ports:write-bytes! tls (text:string->utf8 (string-upcase msg)))
                   (ports:flush! tls)
                   (ports:close! tls)
                   msg)))"
        );
        let out = eval_line(&code, &env);
        assert_eq!(out, "\"hello\"", "server-side echo body mismatch: {out}");
    });
    let port = rx.recv().expect("server did not report its port");
    (port, handle)
}

// ── Round trip, both directions of the wrap ────────────────────────────

#[test]
fn client_server_round_trip_over_tls_with_alpn_and_peer_cert_summary() {
    let pki = make_test_pki();
    let (port, server) = spawn_tls_echo_server(&pki.cert_pem, &pki.key_pem, &["http/1.1"]);

    let env = env_with_tls();
    grant(&env, &["NET-CONNECT", "READ-FS"]);
    let code = format!(
        "(let ((p (tls:connect \"localhost\" {port}
                     :extra-roots (list {ca})
                     :alpn (list \"http/1.1\"))))
           (tcp:set-read-timeout! p 20000)
           (ports:write-bytes! p (text:string->utf8 \"hello\"))
           (ports:flush! p)
           (let* ((reply (text:utf8->string (ports:read-bytes! p 5)))
                  (proto (tls:alpn-protocol p))
                  (summary (tls:peer-certificate-summary p))
                  (count (cdr (assoc ':count summary)))
                  (leaf-len (cdr (assoc ':leaf-der-length summary)))
                  (leaf (cdr (assoc ':leaf-der summary))))
             (ports:close! p)
             (list reply proto count (> leaf-len 0) (= (array-length* leaf) leaf-len))))",
        ca = lisp_path(&pki.ca_pem)
    );
    let out = eval_line(&code, &env);
    assert_eq!(out, "(\"HELLO\" \"http/1.1\" 1 T T)", "got: {out}");
    server.join().unwrap();
}

#[test]
fn server_sees_negotiated_alpn_and_client_offered_sni() {
    let pki = make_test_pki();
    let (tx, rx) = mpsc::channel();
    let cert = lisp_path(&pki.cert_pem);
    let key = lisp_path(&pki.key_pem);
    let server = std::thread::spawn(move || {
        let env = env_with_tls();
        grant(&env, &["NET-LISTEN", "READ-FS"]);
        eval_str("(def $srv (tcp:listen \"127.0.0.1\" 0))", &env).unwrap();
        let port: u16 = eval_line("(net:address-port (tcp:local-addr $srv))", &env)
            .parse()
            .unwrap();
        tx.send(port).unwrap();
        let code = format!(
            "(let* ((accepted (tcp:accept $srv))
                     (plain (car accepted)))
               (tcp:set-read-timeout! plain 20000)
               (let* ((tls (tls:wrap-server plain {cert} {key} :alpn (list \"h2\" \"http/1.1\")))
                      (result (list (tls:alpn-protocol tls) (tls:sni-hostname tls))))
                 (ports:close! tls)
                 result))"
        );
        let out = eval_line(&code, &env);
        assert_eq!(
            out, "(\"http/1.1\" \"localhost\")",
            "server-observed ALPN/SNI mismatch: {out}"
        );
    });
    let port = rx.recv().unwrap();

    let env = env_with_tls();
    grant(&env, &["NET-CONNECT", "READ-FS"]);
    let code = format!(
        "(let ((p (tls:connect \"localhost\" {port}
                     :extra-roots (list {ca})
                     :alpn (list \"http/1.1\"))))
           (ports:close! p)
           t)",
        ca = lisp_path(&pki.ca_pem)
    );
    let out = eval_line(&code, &env);
    assert_eq!(out, "T", "got: {out}");
    server.join().unwrap();
}

// ── Verification ────────────────────────────────────────────────────────

#[test]
fn verification_fails_against_an_unknown_ca_with_the_default_root_store() {
    let pki = make_test_pki();
    let (port, server) = spawn_tls_echo_server(&pki.cert_pem, &pki.key_pem, &[]);

    let env = env_with_tls();
    grant(&env, &["NET-CONNECT"]);
    // No :extra-roots -- the default (webpki-roots) store does not trust
    // this throwaway CA, so the handshake must fail with a structured
    // verification error, never silently succeed.
    let out = eval_line(
        &format!(
            "(handler-case (tls:connect \"localhost\" {port})
                (error (e) (cdr (assoc ':category (error-data e)))))"
        ),
        &env,
    );
    assert_eq!(out, ":TLS-VERIFY-FAILED", "got: {out}");
    // The server's accept blocks forever if the client never completes a
    // handshake attempt at the TCP level; connect a raw socket to unstick
    // it since the failed Lisp handshake above did perform the TCP
    // connect (verification fails after the TLS handshake starts).
    drop(std::net::TcpStream::connect(("127.0.0.1", port)));
    let _ = server.join();
}

#[test]
fn connect_insecure_is_denied_without_host_opt_in() {
    let pki = make_test_pki();
    // The denial happens after TCP:CONNECT (a real TLS attempt always opens
    // the transport first), so the server side still sees -- and fails --
    // one connection attempt; that failure is expected and not asserted on
    // here (`spawn_tls_echo_server`'s own body panics its thread on a
    // mismatched echo, so this test only checks the client-side error, not
    // the server thread's outcome).
    let (port, server) = spawn_tls_echo_server(&pki.cert_pem, &pki.key_pem, &[]);

    let denied_env = env_with_tls();
    grant(&denied_env, &["NET-CONNECT"]);
    let out = eval_line(
        &format!(
            "(handler-case (tls:connect-insecure! \"localhost\" {port})
                (error (e) (cdr (assoc ':category (error-data e)))))"
        ),
        &denied_env,
    );
    assert_eq!(out, ":POLICY-DENIED", "got: {out}");
    let _ = server.join();
}

#[test]
fn connect_insecure_works_once_the_host_opts_in() {
    let pki = make_test_pki();
    let (port, server) = spawn_tls_echo_server(&pki.cert_pem, &pki.key_pem, &[]);

    // With the host opt-in, it works even though the cert is untrusted.
    let allowed_env = env_with_tls();
    grant(&allowed_env, &["NET-CONNECT"]);
    allowed_env.set_allow_insecure_tls(true);
    let out = eval_line(
        &format!(
            "(let ((p (tls:connect-insecure! \"localhost\" {port})))
               (tcp:set-read-timeout! p 20000)
               (ports:write-bytes! p (text:string->utf8 \"hello\"))
               (ports:flush! p)
               (let ((reply (text:utf8->string (ports:read-bytes! p 5))))
                 (ports:close! p)
                 reply))"
        ),
        &allowed_env,
    );
    assert_eq!(out, "\"HELLO\"", "got: {out}");
    server.join().unwrap();
}

// ── https:// end to end through http:request ───────────────────────────

#[test]
fn https_end_to_end_through_http_request() {
    let pki = make_test_pki();
    let (tx, rx) = mpsc::channel();
    let cert = lisp_path(&pki.cert_pem);
    let key = lisp_path(&pki.key_pem);
    let server = std::thread::spawn(move || {
        let env = env_with_tls();
        grant(&env, &["NET-LISTEN", "READ-FS"]);
        eval_str("(def $srv (tcp:listen \"127.0.0.1\" 0))", &env).unwrap();
        let port: u16 = eval_line("(net:address-port (tcp:local-addr $srv))", &env)
            .parse()
            .unwrap();
        tx.send(port).unwrap();
        // Drains the request line + headers before responding -- closing a
        // TLS/TCP port with unread bytes still sitting in the kernel receive
        // buffer makes Linux send RST instead of a clean FIN, which the
        // client sees as a spurious "broken pipe" while it is still trying
        // to write its own request. PORTS:READ-LINE! (unlike HTTP's own
        // internal line reader) does not strip a trailing CR, so the blank
        // line terminating the headers reads back as "\r", not "" -- both
        // must be treated as "done" here.
        eval_str(
            "(defun $test-drain-headers! (port)
               (let ((line (ports:read-line! port)))
                 (if (or (null line) (string-empty-p line) (equal line \"\\r\")) nil
                     ($test-drain-headers! port))))",
            &env,
        )
        .unwrap();
        let code = format!(
            "(let* ((accepted (tcp:accept $srv))
                     (plain (car accepted)))
               (tcp:set-read-timeout! plain 20000)
               (let ((tls (tls:wrap-server plain {cert} {key} :alpn (list \"http/1.1\"))))
                 ($test-drain-headers! tls)
                 (ports:write-string! tls \"HTTP/1.1 200 OK\\r\\nContent-Length: 11\\r\\nConnection: close\\r\\n\\r\\nhello world\")
                 (ports:flush! tls)
                 (ports:close! tls)))"
        );
        eval_str(&code, &env).unwrap();
    });
    let port = rx.recv().unwrap();

    let env = env_with_tls();
    assert_eq!(eval_line("(require 'http)", &env), "HTTP");
    grant(&env, &["NET-CONNECT", "READ-FS"]);
    let out = eval_line(
        &format!(
            "(let* ((r (http:request \"GET\" \"https://localhost:{port}/\"
                          :extra-roots (list {ca}) :read-timeout-ms 20000))
                    (status (http:response-status r))
                    (body (http:collect-string (http:response-body r))))
               (list status body))",
            ca = lisp_path(&pki.ca_pem)
        ),
        &env,
    );
    assert_eq!(out, "(200 \"hello world\")", "got: {out}");
    server.join().unwrap();
}
