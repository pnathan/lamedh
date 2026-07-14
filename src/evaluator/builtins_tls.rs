//! TLS port wrapping (issue #365, epic #253): client/server wrap of a
//! connected TCP [`crate::LispVal::Port`], certificate verification on by
//! default, SNI/ALPN, peer-certificate diagnostics, and no insecure bypass
//! without both an explicitly-named Lisp API AND a host opt-in. Behind the
//! off-by-default `net-tls` cargo feature -- see `Cargo.toml`'s own comment
//! for the dependency ruling (rustls, not native-tls; `ring`, not
//! `aws-lc-rs`, as the crypto provider).
//!
//! **A TLS stream is an ordinary [`crate::LispVal::Port`]** (new
//! `PortState::TlsClient`/`TlsServer` variants in `src/lib.rs`, gated by the
//! same feature): every `PORTS` read/write/close operation and every
//! `TCP-*` out-of-band operation (`shutdown`/timeouts/peer-addr/local-addr
//! -- `PortObj::with_tcp_stream` was generalized to match these two
//! variants too) works on it unchanged, exactly like issue #258 documented
//! this seam in advance. Lisp-facing names live in `lib/43-tls.lisp` (the
//! `TLS` module).
//!
//! **Wrapping consumes the plaintext port.** `tls:wrap-client`/
//! `tls:wrap-server` take ownership of the underlying `TcpStream` out of the
//! plaintext `Port` (`PortObj::take_tcp_stream`), leaving that original
//! `Port` `Closed` -- there is never a moment where the same file descriptor
//! is reachable as both a plaintext port and a TLS port.
//!
//! **FEATURE-OFF BEHAVIOR.** Every `TLS-*` kernel builtin is registered
//! unconditionally (`src/environment.rs`), so `(require 'tls)` and every
//! `tls:*` name always resolve regardless of how this crate was built. With
//! `net-tls` compiled out, every one of them except `TLS-AVAILABLE-P*`
//! signals a structured error --
//! `((:operation . "...") (:category . :tls-unavailable) (:detail . "..."))`
//! -- instead of doing any TLS work; see the `#[cfg(not(feature =
//! "net-tls"))]` half of this file below the real implementation.
//!
//! **No insecure bypass without an explicitly named API, and a host
//! opt-in.** `tls:connect-insecure!` (`TLS-WRAP-CLIENT-INSECURE*`) is the
//! only way to skip certificate verification, and it additionally checks
//! [`crate::environment::Environment::allow_insecure_tls`] -- a Rust-only
//! flag, default `false`, that only the embedding host can set
//! ([`crate::environment::Environment::set_allow_insecure_tls`]). Lisp code
//! alone can never disable verification, no matter what it calls.
//!
//! **Key material is never printed.** Cert/key *source* arguments (paths or
//! byte arrays) and the resulting `rustls::ClientConfig`/`ServerConfig` are
//! never stored on the returned `Port` value or exposed to any printer path
//! -- the `Port` only ever exposes the already-negotiated connection (ALPN,
//! peer certificates, SNI hostname), never key material.
//!
//! **Peer-certificate diagnostics are honestly scoped.** This crate adds no
//! X.509 parser (out of the dependency ruling's scope: rustls +
//! webpki-roots + rustls-pemfile only), so `tls:peer-certificate-summary`
//! cannot report parsed fields like subject/issuer/expiry -- it reports
//! structural data only (count, leaf DER length, leaf DER bytes).
//! `tls:peer-certificates` always has the full raw DER chain for a caller
//! that pulls in its own X.509 parser.

use super::*;

/// Human-readable name for a `TLS-*` `BuiltinFunc`, for error messages. Only
/// used by the feature-off half of this file below (the feature-on half
/// already has a `who: &str` in hand at every call site).
#[cfg(not(feature = "net-tls"))]
fn op_name(op: &BuiltinFunc) -> &'static str {
    match op {
        BuiltinFunc::TlsAvailableP => "tls-available-p*",
        BuiltinFunc::TlsWrapClient => "tls-wrap-client*",
        BuiltinFunc::TlsWrapClientInsecure => "tls-wrap-client-insecure*",
        BuiltinFunc::TlsWrapServer => "tls-wrap-server*",
        BuiltinFunc::TlsAlpnProtocol => "tls-alpn-protocol*",
        BuiltinFunc::TlsPeerCertificates => "tls-peer-certificates*",
        BuiltinFunc::TlsPeerCertificateSummary => "tls-peer-certificate-summary*",
        BuiltinFunc::TlsSniHostname => "tls-sni-hostname*",
        _ => "tls-op*",
    }
}

/// Build a structured TLS error: a `LispVal::Error` whose `data` is an
/// alist `((:operation . "...") (:category . :keyword) (:detail . "..."))`
/// -- mirrors `builtins_net.rs`'s `net_error` shape (`:operation`/
/// `:category` are this codebase's portable networking-error vocabulary;
/// `:host`/`:port` don't apply to a TLS wrap, which always operates on an
/// already-connected port, so this is a strict subset instead of always
/// carrying `nil`/`0` placeholders).
fn tls_error(
    env: &Shared<Environment>,
    operation: &str,
    category: &str,
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
                pair(":DETAIL", LispVal::String(detail.to_string())),
                LispVal::Nil,
            ),
        ),
    );
    let message = format!("{operation}: {category}: {detail}");
    LispError::Signaled(Box::new(LispVal::Error(Shared::new(crate::ErrorObj {
        message,
        data,
    }))))
}

#[cfg(not(feature = "net-tls"))]
pub(super) fn apply_tls_op(
    op: &BuiltinFunc,
    _args: &[LispVal],
    env: &Shared<Environment>,
) -> Result<LispVal, LispError> {
    if matches!(op, BuiltinFunc::TlsAvailableP) {
        return Ok(LispVal::Nil);
    }
    Err(tls_error(
        env,
        op_name(op),
        "TLS-UNAVAILABLE",
        "this build of lamedh was compiled without the `net-tls` cargo feature -- rebuild with `--features net-tls` to use TLS",
    ))
}

#[cfg(feature = "net-tls")]
mod real {
    use super::*;
    use rustls::pki_types::{CertificateDer, ServerName};
    use std::net::TcpStream;
    use std::sync::Arc;

    // ── Argument helpers (mirrors builtins_net.rs's/builtins_ports.rs's
    // local sets) ─────────────────────────────────────────────────────────

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

    fn bytes_to_char_array(bytes: Vec<u8>) -> LispVal {
        let items: Vec<LispVal> = bytes.into_iter().map(LispVal::Char).collect();
        LispVal::Array(Shared::new(SharedCell::new(items)))
    }

    /// A cert/key/root *source* argument: a `String` path (read under
    /// `READ-FS`) or an `Array<Char>` of raw bytes -- issue #365's "supplied
    /// as String paths or byte arrays under the corresponding
    /// filesystem/resource capabilities".
    fn source_bytes(
        env: &Shared<Environment>,
        v: &LispVal,
        who: &str,
    ) -> Result<Vec<u8>, LispError> {
        match v {
            LispVal::String(path) => {
                require_read_fs(env)?;
                std::fs::read(path)
                    .map_err(|e| tls_error(env, who, "OTHER", &format!("reading {path:?}: {e}")))
            }
            LispVal::Array(_) => get_char_array_bytes(v, who),
            other => Err(LispError::Generic(format!(
                "{}: expected a string path or byte array, got {}",
                who.to_uppercase(),
                err_val(other)
            ))),
        }
    }

    /// A NIL-or-proper-list of Strings (ALPN protocol names, ASCII).
    fn list_to_alpn(v: &LispVal, who: &str) -> Result<Vec<Vec<u8>>, LispError> {
        let mut out = Vec::new();
        let mut cur = v.clone();
        loop {
            match cur {
                LispVal::Nil => return Ok(out),
                LispVal::Cons { car, cdr } => {
                    match &*car {
                        LispVal::String(s) => out.push(s.clone().into_bytes()),
                        other => {
                            return Err(LispError::Generic(format!(
                                "{}: ALPN list must contain strings, got {}",
                                who.to_uppercase(),
                                err_val(other)
                            )));
                        }
                    }
                    cur = (*cdr).clone();
                }
                other => {
                    return Err(LispError::Generic(format!(
                        "{}: expected a proper list of strings for ALPN, got {}",
                        who.to_uppercase(),
                        err_val(&other)
                    )));
                }
            }
        }
    }

    /// A NIL-or-proper-list of cert/root sources (each a `source_bytes`
    /// argument, PEM-encoded, possibly a multi-cert bundle), parsed and
    /// flattened into DER certificates.
    fn list_to_der_certs(
        env: &Shared<Environment>,
        v: &LispVal,
        who: &str,
    ) -> Result<Vec<CertificateDer<'static>>, LispError> {
        let mut out = Vec::new();
        let mut cur = v.clone();
        loop {
            match cur {
                LispVal::Nil => return Ok(out),
                LispVal::Cons { car, cdr } => {
                    let bytes = source_bytes(env, &car, who)?;
                    let certs = rustls_pemfile::certs(&mut std::io::Cursor::new(bytes.as_slice()))
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|e| {
                            tls_error(
                                env,
                                who,
                                "TLS-CONFIG",
                                &format!("parsing PEM certificate data: {e}"),
                            )
                        })?;
                    out.extend(certs);
                    cur = (*cdr).clone();
                }
                other => {
                    return Err(LispError::Generic(format!(
                        "{}: expected a proper list for extra-roots, got {}",
                        who.to_uppercase(),
                        err_val(&other)
                    )));
                }
            }
        }
    }

    // ── Crypto provider / config construction ──────────────────────────────

    fn crypto_provider() -> Arc<rustls::crypto::CryptoProvider> {
        Arc::new(rustls::crypto::ring::default_provider())
    }

    fn build_root_store(
        env: &Shared<Environment>,
        extra_roots: &LispVal,
        who: &str,
    ) -> Result<rustls::RootCertStore, LispError> {
        let mut roots = rustls::RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        for der in list_to_der_certs(env, extra_roots, who)? {
            roots.add(der).map_err(|e| {
                tls_error(
                    env,
                    who,
                    "TLS-CONFIG",
                    &format!("adding extra root certificate: {e}"),
                )
            })?;
        }
        Ok(roots)
    }

    /// The "accept any certificate" verifier behind `tls:connect-insecure!`
    /// (issue #365: "no insecure certificate bypass without an explicitly
    /// named API"). Signature verification is still delegated to the real
    /// crypto provider -- only the *chain-of-trust* check is skipped -- so
    /// this cannot be used to accept a connection with a forged signature,
    /// only one whose certificate is untrusted/expired/hostname-mismatched.
    #[derive(Debug)]
    struct NoCertVerification(Arc<rustls::crypto::CryptoProvider>);

    impl rustls::client::danger::ServerCertVerifier for NoCertVerification {
        fn verify_server_cert(
            &self,
            _end_entity: &CertificateDer<'_>,
            _intermediates: &[CertificateDer<'_>],
            _server_name: &ServerName<'_>,
            _ocsp_response: &[u8],
            _now: rustls::pki_types::UnixTime,
        ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
            Ok(rustls::client::danger::ServerCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self,
            message: &[u8],
            cert: &CertificateDer<'_>,
            dss: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            rustls::crypto::verify_tls12_signature(
                message,
                cert,
                dss,
                &self.0.signature_verification_algorithms,
            )
        }

        fn verify_tls13_signature(
            &self,
            message: &[u8],
            cert: &CertificateDer<'_>,
            dss: &rustls::DigitallySignedStruct,
        ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
            rustls::crypto::verify_tls13_signature(
                message,
                cert,
                dss,
                &self.0.signature_verification_algorithms,
            )
        }

        fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
            self.0.signature_verification_algorithms.supported_schemes()
        }
    }

    fn build_client_config(
        env: &Shared<Environment>,
        who: &str,
        roots: rustls::RootCertStore,
        alpn: Vec<Vec<u8>>,
    ) -> Result<Arc<rustls::ClientConfig>, LispError> {
        let provider = crypto_provider();
        let mut cfg = rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(|e| tls_error(env, who, "TLS-CONFIG", &e.to_string()))?
            .with_root_certificates(roots)
            .with_no_client_auth();
        cfg.alpn_protocols = alpn;
        Ok(Arc::new(cfg))
    }

    fn build_insecure_client_config(
        env: &Shared<Environment>,
        who: &str,
        alpn: Vec<Vec<u8>>,
    ) -> Result<Arc<rustls::ClientConfig>, LispError> {
        let provider = crypto_provider();
        let verifier = Arc::new(NoCertVerification(provider.clone()));
        let mut cfg = rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(|e| tls_error(env, who, "TLS-CONFIG", &e.to_string()))?
            .dangerous()
            .with_custom_certificate_verifier(verifier)
            .with_no_client_auth();
        cfg.alpn_protocols = alpn;
        Ok(Arc::new(cfg))
    }

    // ── Handshake driving + error classification ───────────────────────────

    /// Classify a handshake I/O error: a rustls protocol-level error (chain
    /// verification failure, malformed record, ...) wrapped inside the
    /// `io::Error` is reported as `:TLS-VERIFY-FAILED`/`:TLS-HANDSHAKE`;
    /// anything else (timeout, reset, ...) reuses `builtins_net.rs`'s own
    /// classifier so a timed-out handshake and a timed-out plain TCP
    /// connect signal the same `:TIMEOUT` category.
    fn classify_handshake_io_error(
        env: &Shared<Environment>,
        who: &str,
        e: std::io::Error,
    ) -> LispError {
        if let Some(rustls_err) = e
            .get_ref()
            .and_then(|inner| inner.downcast_ref::<rustls::Error>())
        {
            let category = match rustls_err {
                rustls::Error::InvalidCertificate(_) => "TLS-VERIFY-FAILED",
                _ => "TLS-HANDSHAKE",
            };
            return tls_error(env, who, category, &rustls_err.to_string());
        }
        let category = super::super::builtins_net::classify_io_error(e.kind());
        tls_error(env, who, category, &e.to_string())
    }

    fn drive_client_handshake(
        env: &Shared<Environment>,
        who: &str,
        conn: rustls::ClientConnection,
        sock: TcpStream,
    ) -> Result<rustls::StreamOwned<rustls::ClientConnection, TcpStream>, LispError> {
        let mut stream = rustls::StreamOwned::new(conn, sock);
        while stream.conn.is_handshaking() {
            if let Err(e) = stream.conn.complete_io(&mut stream.sock) {
                return Err(classify_handshake_io_error(env, who, e));
            }
        }
        Ok(stream)
    }

    fn drive_server_handshake(
        env: &Shared<Environment>,
        who: &str,
        conn: rustls::ServerConnection,
        sock: TcpStream,
    ) -> Result<rustls::StreamOwned<rustls::ServerConnection, TcpStream>, LispError> {
        let mut stream = rustls::StreamOwned::new(conn, sock);
        while stream.conn.is_handshaking() {
            if let Err(e) = stream.conn.complete_io(&mut stream.sock) {
                return Err(classify_handshake_io_error(env, who, e));
            }
        }
        Ok(stream)
    }

    // ── Builtin bodies ──────────────────────────────────────────────────────

    fn tls_wrap_client(
        args: &[LispVal],
        env: &Shared<Environment>,
        insecure: bool,
    ) -> Result<LispVal, LispError> {
        let who = if insecure {
            "tls-wrap-client-insecure*"
        } else {
            "tls-wrap-client*"
        };
        if args.len() != 4 {
            return Err(LispError::Generic(format!(
                "{who} requires exactly four arguments: port hostname alpn-list extra-roots-list"
            )));
        }
        let port = expect_port(args, 0, who)?;
        let hostname = expect_string(args, 1, who)?;
        let alpn = list_to_alpn(&args[2], who)?;

        if insecure && !env.allow_insecure_tls() {
            // Close the still-plaintext port ourselves rather than leaving
            // it to whenever the Lisp environment happens to drop its last
            // reference (e.g. backtrace retention can keep a `let` frame,
            // and therefore this port, alive well past this call) -- a
            // peer blocked reading a ClientHello that will now never arrive
            // deserves an immediate, deterministic disconnect, not one that
            // depends on Lisp-level GC timing.
            port.close();
            return Err(tls_error(
                env,
                who,
                "POLICY-DENIED",
                "tls:connect-insecure! requires host opt-in (Environment::set_allow_insecure_tls) -- Lisp code alone cannot disable certificate verification",
            ));
        }

        let server_name = ServerName::try_from(hostname.clone())
            .map_err(|e| {
                tls_error(
                    env,
                    who,
                    "TLS-CONFIG",
                    &format!("invalid hostname {hostname:?}: {e}"),
                )
            })?
            .to_owned();

        let config = if insecure {
            build_insecure_client_config(env, who, alpn)?
        } else {
            let roots = build_root_store(env, &args[3], who)?;
            build_client_config(env, who, roots, alpn)?
        };

        let conn = rustls::ClientConnection::new(config, server_name)
            .map_err(|e| tls_error(env, who, "TLS-CONFIG", &e.to_string()))?;
        let tcp = port
            .take_tcp_stream()
            .map_err(|e| tls_error(env, who, "OTHER", &e.to_string()))?;
        let name = tcp
            .peer_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|_| hostname.clone());
        let stream = drive_client_handshake(env, who, conn, tcp)?;
        Ok(LispVal::Port(crate::PortObj::tls_client(name, stream)))
    }

    fn tls_wrap_server(args: &[LispVal], env: &Shared<Environment>) -> Result<LispVal, LispError> {
        let who = "tls-wrap-server*";
        if args.len() != 4 {
            return Err(LispError::Generic(format!(
                "{who} requires exactly four arguments: port cert-source key-source alpn-list"
            )));
        }
        let port = expect_port(args, 0, who)?;
        let cert_bytes = source_bytes(env, &args[1], who)?;
        let key_bytes = source_bytes(env, &args[2], who)?;
        let alpn = list_to_alpn(&args[3], who)?;

        let certs = rustls_pemfile::certs(&mut std::io::Cursor::new(cert_bytes.as_slice()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| {
                tls_error(
                    env,
                    who,
                    "TLS-CONFIG",
                    &format!("parsing PEM certificate: {e}"),
                )
            })?;
        if certs.is_empty() {
            return Err(tls_error(
                env,
                who,
                "TLS-CONFIG",
                "no certificates found in cert source",
            ));
        }
        let key = rustls_pemfile::private_key(&mut std::io::Cursor::new(key_bytes.as_slice()))
            .map_err(|e| {
                tls_error(
                    env,
                    who,
                    "TLS-CONFIG",
                    &format!("parsing PEM private key: {e}"),
                )
            })?
            .ok_or_else(|| {
                tls_error(env, who, "TLS-CONFIG", "no private key found in key source")
            })?;

        let provider = crypto_provider();
        let mut config = rustls::ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .map_err(|e| tls_error(env, who, "TLS-CONFIG", &e.to_string()))?
            .with_no_client_auth()
            .with_single_cert(certs, key)
            .map_err(|e| tls_error(env, who, "TLS-CONFIG", &e.to_string()))?;
        config.alpn_protocols = alpn;

        let conn = rustls::ServerConnection::new(Arc::new(config))
            .map_err(|e| tls_error(env, who, "TLS-CONFIG", &e.to_string()))?;
        let tcp = port
            .take_tcp_stream()
            .map_err(|e| tls_error(env, who, "OTHER", &e.to_string()))?;
        let name = tcp
            .peer_addr()
            .map(|a| a.to_string())
            .unwrap_or_else(|_| "tls-server".to_string());
        let stream = drive_server_handshake(env, who, conn, tcp)?;
        Ok(LispVal::Port(crate::PortObj::tls_server(name, stream)))
    }

    fn tls_alpn_protocol(
        args: &[LispVal],
        env: &Shared<Environment>,
    ) -> Result<LispVal, LispError> {
        let who = "tls-alpn-protocol*";
        if args.len() != 1 {
            return Err(LispError::Generic(format!(
                "{who} requires exactly one argument"
            )));
        }
        let port = expect_port(args, 0, who)?;
        let proto = port
            .tls_alpn_protocol()
            .map_err(|e| tls_error(env, who, "OTHER", &e.to_string()))?;
        Ok(match proto {
            Some(bytes) => LispVal::String(String::from_utf8_lossy(&bytes).into_owned()),
            None => LispVal::Nil,
        })
    }

    fn tls_peer_certificates(
        args: &[LispVal],
        env: &Shared<Environment>,
    ) -> Result<LispVal, LispError> {
        let who = "tls-peer-certificates*";
        if args.len() != 1 {
            return Err(LispError::Generic(format!(
                "{who} requires exactly one argument"
            )));
        }
        let port = expect_port(args, 0, who)?;
        let certs = port
            .tls_peer_certificates()
            .map_err(|e| tls_error(env, who, "OTHER", &e.to_string()))?;
        Ok(match certs {
            None => LispVal::Nil,
            Some(list) => {
                let mut out = LispVal::Nil;
                for der in list.into_iter().rev() {
                    out = LispVal::Cons {
                        car: Shared::new(bytes_to_char_array(der)),
                        cdr: Shared::new(out),
                    };
                }
                out
            }
        })
    }

    fn tls_peer_certificate_summary(
        args: &[LispVal],
        env: &Shared<Environment>,
    ) -> Result<LispVal, LispError> {
        let who = "tls-peer-certificate-summary*";
        if args.len() != 1 {
            return Err(LispError::Generic(format!(
                "{who} requires exactly one argument"
            )));
        }
        let port = expect_port(args, 0, who)?;
        let certs = port
            .tls_peer_certificates()
            .map_err(|e| tls_error(env, who, "OTHER", &e.to_string()))?;
        let cons = |car: LispVal, cdr: LispVal| LispVal::Cons {
            car: Shared::new(car),
            cdr: Shared::new(cdr),
        };
        let sym = |s: &str| LispVal::Symbol(env.intern_symbol(s));
        let pair = |k: &str, v: LispVal| cons(sym(k), v);
        Ok(match certs {
            None => LispVal::Nil,
            Some(list) => {
                let count = list.len() as i64;
                let leaf = list.into_iter().next().unwrap_or_default();
                let leaf_len = leaf.len() as i64;
                cons(
                    pair(":COUNT", LispVal::Number(count)),
                    cons(
                        pair(":LEAF-DER-LENGTH", LispVal::Number(leaf_len)),
                        cons(pair(":LEAF-DER", bytes_to_char_array(leaf)), LispVal::Nil),
                    ),
                )
            }
        })
    }

    fn tls_sni_hostname(args: &[LispVal], env: &Shared<Environment>) -> Result<LispVal, LispError> {
        let who = "tls-sni-hostname*";
        if args.len() != 1 {
            return Err(LispError::Generic(format!(
                "{who} requires exactly one argument"
            )));
        }
        let port = expect_port(args, 0, who)?;
        let name = port
            .tls_sni_hostname()
            .map_err(|e| tls_error(env, who, "OTHER", &e.to_string()))?;
        Ok(match name {
            Some(s) => LispVal::String(s),
            None => LispVal::Nil,
        })
    }

    #[inline(never)]
    pub(in super::super) fn apply_tls_op(
        op: &BuiltinFunc,
        args: &[LispVal],
        env: &Shared<Environment>,
    ) -> Result<LispVal, LispError> {
        let t = || LispVal::Symbol(env.intern_symbol("T"));
        match op {
            BuiltinFunc::TlsAvailableP => Ok(t()),
            BuiltinFunc::TlsWrapClient => tls_wrap_client(args, env, false),
            BuiltinFunc::TlsWrapClientInsecure => tls_wrap_client(args, env, true),
            BuiltinFunc::TlsWrapServer => tls_wrap_server(args, env),
            BuiltinFunc::TlsAlpnProtocol => tls_alpn_protocol(args, env),
            BuiltinFunc::TlsPeerCertificates => tls_peer_certificates(args, env),
            BuiltinFunc::TlsPeerCertificateSummary => tls_peer_certificate_summary(args, env),
            BuiltinFunc::TlsSniHostname => tls_sni_hostname(args, env),
            _ => Err(LispError::Generic("Not a TLS operation".to_string())),
        }
    }
}

#[cfg(feature = "net-tls")]
pub(super) use real::apply_tls_op;
