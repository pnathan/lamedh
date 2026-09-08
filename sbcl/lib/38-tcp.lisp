;;; TCP module — connect/bind/listen/accept over binary ports (issue #258,
;;; epic #253).
;;;
;;; A connected TCP stream is an ordinary PORTS binary port (issue #255):
;;; TCP:CONNECT and TCP:ACCEPT both return a value every PORTS operation
;;; (lib/31-ports.lisp: READ-BYTE!, WRITE-BYTES!, CLOSE!, WITH-OPEN-PORT,
;;; PORT-P, ...) already works on unchanged. This module adds only the
;;; TCP-specific operations PORTS has no notion of: connecting/listening/
;;; accepting, half-close (SHUTDOWN!), and read/write timeouts. This is also
;;; the seam a future TLS layer (explicitly deferred from this issue) wraps:
;;; a TLS client/server port would take/return this same PORTS port shape.
;;;
;;; LISTENERS are NOT byte streams (they yield new connections, not bytes),
;;; so they get their own opaque handle instead of being a port -- see
;;; src/lib.rs's LispVal::NetHandle and src/evaluator/builtins_net.rs.
;;;
;;; CAPABILITIES: CONNECT needs NET-CONNECT; LISTEN needs NET-LISTEN. ACCEPT,
;;; SHUTDOWN!, SET-READ-TIMEOUT!, SET-WRITE-TIMEOUT!, LOCAL-ADDR, PEER-ADDR,
;;; and CLOSE-LISTENER! need no further capability check -- using a resource
;;; you already hold is "continue" authority (issue #255's rule, same as
;;; PORTS). In addition to the capability, CONNECT/LISTEN also consult the
;;; host policy hook (see src/evaluator/builtins_net.rs's module header).
;;;
;;; REQUIRE-ABLE (issue #256): `(require 'tcp)` on a `with_prelude()`
;;; environment loads exactly this file. It requires 'modules and 'net
;;; first (NET:ADDRESS wraps ACCEPT's/LOCAL-ADDR's/PEER-ADDR's peer data).

(require 'modules)
(require 'net)

(defmodule tcp
  (:export connect listen accept shutdown! set-read-timeout! set-write-timeout!
           close-listener! listener-p listener-open-p local-addr peer-addr)
  (:requires net-connect net-listen))

(with-module tcp

  (defun connect (host port &optional timeout-ms)
    "Connect to HOST:PORT over TCP, returning a duplex binary PORTS port
(every PORTS operation works on it -- see lib/31-ports.lisp). Requires
NET-CONNECT. TIMEOUT-MS, if given, bounds the connect attempt; NIL
(the default) blocks without a timeout. Signals a structured error
distinguishing :TIMEOUT/:REFUSED/:RESET/:DNS/:POLICY-DENIED failures (see
src/evaluator/builtins_net.rs's module header)."
    (tcp-connect* host port timeout-ms))

  (defun listen (host port &optional (backlog 128))
    "Bind and listen on HOST:PORT for inbound TCP connections, returning a
listener handle. Requires NET-LISTEN. BACKLOG is accepted for API
completeness but is currently advisory only: std::net::TcpListener exposes
no OS backlog customization without an additional dependency, and this
issue ships none (see TCP-LISTEN*'s Rust doc comment)."
    (tcp-listen* host port backlog))

  (defun accept (listener)
    "Block until an inbound connection arrives on LISTENER, returning
(CONS port peer-address) -- port is a duplex PORTS port, peer-address a
NET:ADDRESS. Rejects use after CLOSE-LISTENER! with a :CLOSED error."
    (let ((result (tcp-accept* listener)))
      (cons (car result) (net:address-from-triple (cdr result)))))

  (defun shutdown! (port how)
    "Shut down PORT's read half, write half, or both (HOW: :READ, :WRITE, or
:BOTH) without closing it -- the peer sees EOF/a reset on the shutdown
half, but PORT itself stays open for the other direction and for CLOSE!."
    (tcp-shutdown* port how))

  (defun set-read-timeout! (port ms)
    "Set PORT's read timeout in milliseconds; NIL blocks without a timeout
(the default). A timed-out read signals a structured :TIMEOUT error."
    (tcp-set-read-timeout* port ms))

  (defun set-write-timeout! (port ms)
    "Set PORT's write timeout in milliseconds; NIL blocks without a timeout
(the default)."
    (tcp-set-write-timeout* port ms))

  (defun close-listener! (listener)
    "Close LISTENER. Idempotent, like PORTS:CLOSE!. A concurrent ACCEPT
blocked on another OS thread is not guaranteed to unblock (plain
std::net::TcpListener has no portable wakeup-on-close), but every
subsequent ACCEPT call on this LISTENER errors immediately with a
:CLOSED error -- that determinism, not OS-level wakeup, is this module's
documented close contract."
    (net-handle-close* listener))

  (defun listener-p (x)
    "T if X is a TCP listener handle (as returned by LISTEN)."
    (and (net-handle-p* x) (eq (net-handle-kind* x) 'tcp-listener)))

  (defun listener-open-p (listener)
    (net-handle-open-p* listener))

  (defun local-addr (resource)
    "The local NET:ADDRESS a connected TCP PORT or a LISTENER is bound to."
    (net:local-addr resource))

  (defun peer-addr (port)
    "The remote NET:ADDRESS a connected TCP PORT is connected to."
    (net:peer-addr port))

  )

(provide 'tcp '(tcp:connect tcp:listen tcp:accept tcp:shutdown!
                tcp:set-read-timeout! tcp:set-write-timeout!
                tcp:close-listener! tcp:listener-p tcp:listener-open-p
                tcp:local-addr tcp:peer-addr))
