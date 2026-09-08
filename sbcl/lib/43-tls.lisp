;;; TLS module — client/server wrap of a connected TCP port (issue #365,
;;; epic #253), behind the off-by-default `net-tls` cargo feature.
;;;
;;; DEPENDENCY RULING (owner, binding, #364/#365): rustls + webpki-roots +
;;; rustls-pemfile, behind a Cargo feature that is NOT in the default
;;; feature set -- see Cargo.toml's own comment. The default build's
;;; behavior and dependency tree are unchanged; nothing here pulls in a TLS
;;; crate unless the embedder/CLI builds with `--features net-tls`.
;;;
;;; A TLS stream is an ordinary PORTS binary port (issue #255), exactly like
;;; a plain TCP stream (issue #258): every PORTS operation (READ-BYTE!,
;;; WRITE-BYTES!, CLOSE!, WITH-OPEN-PORT, PORT-P, ...) and every TCP
;;; out-of-band operation (SHUTDOWN!, SET-READ-TIMEOUT!, SET-WRITE-TIMEOUT!,
;;; via NET:LOCAL-ADDR/NET:PEER-ADDR) already works on it unchanged -- this
;;; module adds only the TLS-specific operations: wrapping, and reading back
;;; the negotiated ALPN protocol / peer certificates / SNI hostname.
;;;
;;; WRAPPING CONSUMES THE PLAINTEXT PORT: WRAP-CLIENT/WRAP-SERVER take
;;; ownership of the underlying TCP connection out of the PORT you pass in
;;; -- that original PORT value becomes CLOSED (any further PORTS operation
;;; on it errors, exactly like after CLOSE!) the instant it is wrapped; use
;;; the newly returned PORT from here on.
;;;
;;; VERIFICATION IS ON BY DEFAULT. WRAP-CLIENT/CONNECT verify the peer
;;; certificate chain against the default root store (Mozilla's set, via
;;; webpki-roots) plus any :EXTRA-ROOTS you supply (PEM data, as a String
;;; path -- READ-FS gated -- or an Array<Char> of raw bytes; this is also
;;; how a test harness trusts a throwaway self-signed CA) and checks the
;;; certificate against :HOSTNAME (also used for SNI).
;;;
;;; NO INSECURE BYPASS WITHOUT AN EXPLICITLY NAMED API, AND A HOST OPT-IN.
;;; CONNECT-INSECURE!/WRAP-CLIENT-INSECURE! are the only way to skip
;;; certificate verification -- there is no keyword flag on WRAP-CLIENT/
;;; CONNECT that silently does it. Even so, calling them signals a
;;; structured `:POLICY-DENIED` error unless the *host* embedding this
;;; interpreter separately called `Environment::set_allow_insecure_tls`
;;; (Rust-only; see docs/embedding.md) -- Lisp code alone can never disable
;;; verification, no matter what it calls.
;;;
;;; SERVER-SIDE CERT/KEY are supplied the same way as :EXTRA-ROOTS above: a
;;; String path (READ-FS gated) or an Array<Char> of raw PEM bytes.
;;;
;;; FEATURE-OFF BEHAVIOR: this file loads unconditionally (like every other
;;; optional module) regardless of how the crate was built, so `(require
;;; 'tls)` and every TLS:* name always resolve. `(tls:available-p)` reports
;;; whether the `net-tls` feature is actually compiled in; every other TLS:*
;;; operation signals a structured `:CATEGORY :TLS-UNAVAILABLE` error
;;; instead of doing any work when it is not (checked at the kernel-builtin
;;; level, `src/evaluator/builtins_tls.rs`, so this stays true regardless of
;;; how a caller reaches these functions).
;;;
;;; PEER-CERTIFICATE DIAGNOSTICS ARE HONESTLY SCOPED: no X.509 parser is
;;; part of this dependency ruling, so PEER-CERTIFICATE-SUMMARY reports
;;; structural data only (count, leaf DER length, leaf DER bytes) -- not
;;; parsed subject/issuer/expiry fields. PEER-CERTIFICATES always returns
;;; the full raw DER chain for a caller that wants to pull in its own X.509
;;; parser.
;;;
;;; REQUIRE-ABLE (issue #256): `(require 'tls)` on a `with_prelude()`
;;; environment loads exactly this file. It requires 'modules and 'tcp
;;; first (a TLS port is layered directly on a TCP:CONNECT/TCP:ACCEPT port).

(require 'modules)
(require 'tcp)

(defmodule tls
  (:export available-p
           wrap-client wrap-client-insecure!
           wrap-server
           connect connect-insecure!
           alpn-protocol peer-certificates peer-certificate-summary sni-hostname)
  (:requires read-fs))

(with-module tls

  (defun available-p ()
    "T if this build of lamedh was compiled with the `net-tls` cargo
feature (rustls); NIL otherwise. Every TLS:* name is bound either way --
see the file header's FEATURE-OFF BEHAVIOR note."
    (tls-available-p*))

  (defun wrap-client (port &key hostname (alpn ()) (extra-roots ()))
    "Wrap PORT -- an already-connected TCP:CONNECT PORT -- as a TLS client,
performing the handshake now (blocking; bound it with
TCP:SET-READ-TIMEOUT!/TCP:SET-WRITE-TIMEOUT! on PORT beforehand if you want
a handshake timeout) and returning a new PORTS port. Consumes PORT (it
becomes CLOSED). :HOSTNAME is required -- used both for SNI and for
certificate-chain hostname verification. :ALPN is a list of protocol-name
Strings to offer (see ALPN-PROTOCOL for what was negotiated). :EXTRA-ROOTS
is a list of PEM cert sources (String path, READ-FS gated, or Array<Char>
bytes) trusted in addition to the default (webpki-roots) root store -- this
is how a test harness trusts a throwaway self-signed CA. Signals a
structured error (`:CATEGORY` one of `:TLS-VERIFY-FAILED`/`:TLS-HANDSHAKE`/
`:TLS-CONFIG`/`:TIMEOUT`/`:RESET`/...) on failure -- see the file header."
    (if (null hostname)
        (error "TLS:WRAP-CLIENT requires :HOSTNAME (used for SNI and certificate verification)"
               (list (cons ':category ':tls-config)))
        (tls-wrap-client* port hostname alpn extra-roots)))

  (defun wrap-client-insecure! (port &key (hostname "") (alpn ()))
    "Like WRAP-CLIENT, but skips certificate-chain verification entirely
-- the peer's certificate is accepted no matter who issued it or whether it
matches :HOSTNAME (still used for SNI). See the file header's NO INSECURE
BYPASS note: this ALWAYS signals a structured `:POLICY-DENIED` error unless
the host embedding this interpreter has separately opted in via
`Environment::set_allow_insecure_tls` (Rust-only)."
    (tls-wrap-client-insecure* port hostname alpn ()))

  (defun wrap-server (port cert key &key (alpn ()))
    "Wrap PORT -- an already-accepted TCP:ACCEPT PORT -- as a TLS server,
performing the handshake now (blocking) and returning a new PORTS port.
Consumes PORT (it becomes CLOSED). CERT and KEY are each a PEM source
(String path, READ-FS gated, or Array<Char> bytes); CERT may be a full
chain (leaf first). :ALPN is a list of protocol-name Strings this server
supports (see ALPN-PROTOCOL for what was negotiated with a given client).
No client-certificate authentication is requested."
    (tls-wrap-server* port cert key alpn))

  (defun connect (host port &key (connect-timeout-ms nil) (handshake-timeout-ms nil)
                  (alpn ()) (extra-roots ()))
    "TCP:CONNECT to HOST:PORT, then WRAP-CLIENT the result with :HOSTNAME
defaulting to HOST -- the common case of both steps together.
:CONNECT-TIMEOUT-MS bounds the TCP connect; :HANDSHAKE-TIMEOUT-MS, if
given, is set as both the read and write timeout on the TCP port before
wrapping (so a stalled handshake times out instead of blocking forever)."
    (let ((tcp-port (tcp:connect host port connect-timeout-ms)))
      (if handshake-timeout-ms
          (progn (tcp:set-read-timeout! tcp-port handshake-timeout-ms)
                 (tcp:set-write-timeout! tcp-port handshake-timeout-ms))
          nil)
      (wrap-client tcp-port :hostname host :alpn alpn :extra-roots extra-roots)))

  (defun connect-insecure! (host port &key (connect-timeout-ms nil) (handshake-timeout-ms nil)
                             (alpn ()))
    "TCP:CONNECT to HOST:PORT, then WRAP-CLIENT-INSECURE! the result -- the
insecure sibling of CONNECT. See WRAP-CLIENT-INSECURE!'s host-opt-in note:
this is denied by default no matter what Lisp code does."
    (let ((tcp-port (tcp:connect host port connect-timeout-ms)))
      (if handshake-timeout-ms
          (progn (tcp:set-read-timeout! tcp-port handshake-timeout-ms)
                 (tcp:set-write-timeout! tcp-port handshake-timeout-ms))
          nil)
      (wrap-client-insecure! tcp-port :hostname host :alpn alpn)))

  (defun alpn-protocol (port)
    "The ALPN protocol negotiated on TLS PORT (a String), or NIL if none
was negotiated (or none was offered)."
    (tls-alpn-protocol* port))

  (defun peer-certificates (port)
    "The peer's certificate chain on TLS PORT, leaf first, as a list of
Array<Char> raw DER bytes -- or NIL if no certificates were presented (a
client with no client-cert auth requested, or the connection is not TLS).
See the file header's PEER-CERTIFICATE DIAGNOSTICS note: no X.509 parser is
part of this module, so these are opaque DER bytes, not parsed fields."
    (tls-peer-certificates* port))

  (defun peer-certificate-summary (port)
    "A structural summary alist for TLS PORT's peer certificate chain:
`((:count . N) (:leaf-der-length . M) (:leaf-der . bytes))`, or NIL if no
certificates were presented. See PEER-CERTIFICATES for the full chain, and
the file header for why this has no parsed subject/issuer/expiry fields."
    (tls-peer-certificate-summary* port))

  (defun sni-hostname (port)
    "The SNI hostname a TLS client offered when connecting to server-side
PORT, or NIL if none was sent (or PORT is a client-side TLS port)."
    (tls-sni-hostname* port))

  )

(provide 'tls
  '(tls:available-p
    tls:wrap-client tls:wrap-client-insecure!
    tls:wrap-server
    tls:connect tls:connect-insecure!
    tls:alpn-protocol tls:peer-certificates tls:peer-certificate-summary tls:sni-hostname))
