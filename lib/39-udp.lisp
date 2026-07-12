;;; UDP module — bind/send-to/receive-from datagram sockets (issue #258,
;;; epic #253).
;;;
;;; A UDP socket is NOT a byte stream (datagram boundaries are meaningful,
;;; unlike TCP's byte stream), so it gets its own opaque handle instead of
;;; being a PORTS port -- the same LispVal::NetHandle representation
;;; TCP:LISTEN's listener uses; see src/lib.rs and
;;; src/evaluator/builtins_net.rs.
;;;
;;; CAPABILITIES: BIND needs NET-LISTEN -- even an ephemeral-port socket
;;; receives datagrams from any sender once bound, matching the epic's
;;; "binding/listening for inbound traffic" authority (there is no
;;; connection-acceptance step to gate separately, unlike TCP). CONNECT!
;;; and SEND-TO need NET-CONNECT (they name an arbitrary destination). SEND
;;; and RECEIVE-FROM need no further capability check: using a socket you
;;; already hold, at the peer already authorized by CONNECT!, is "continue"
;;; authority (issue #255's rule). In addition to the capability,
;;; BIND/CONNECT!/SEND-TO also consult the host policy hook (see
;;; src/evaluator/builtins_net.rs's module header).
;;;
;;; DATAGRAM TRUNCATION: plain std::net exposes no MSG_TRUNC indicator
;;; without raw syscalls (out of this issue's zero-new-dependency, no-ioctl
;;; scope). RECEIVE-FROM's third return value is therefore a best-effort
;;; "possibly truncated" flag (true when the received length exactly equals
;;; the requested MAXLEN) -- see RECEIVE-FROM's docstring.
;;;
;;; REQUIRE-ABLE (issue #256): `(require 'udp)` on a `with_prelude()`
;;; environment loads exactly this file. It requires 'modules and 'net
;;; first (NET:ADDRESS wraps RECEIVE-FROM's peer data).

(require 'modules)
(require 'net)

(defmodule udp
  (:export bind connect! send-to send receive-from close! socket-p
           socket-open-p local-addr set-timeout!)
  (:requires net-connect net-listen))

(with-module udp

  (defun bind (host port)
    "Bind a UDP socket to HOST:PORT (PORT 0 for an OS-assigned ephemeral
port). Requires NET-LISTEN."
    (udp-bind* host port))

  (defun connect! (socket host port)
    "Set SOCKET's default peer to HOST:PORT so SEND/RECEIVE-FROM can be used
without repeating the address on every call. Requires NET-CONNECT."
    (udp-connect* socket host port))

  (defun send-to (socket host port bytes)
    "Send BYTES (an Array<Char>, elements Char or integer 0-255 -- see
lib/31-ports.lisp's byte convention; use TEXT:STRING->UTF8 for a String) as
one datagram to HOST:PORT, returning the number of bytes sent. Requires
NET-CONNECT."
    (udp-send-to* socket host port bytes))

  (defun send (socket bytes)
    "Send BYTES as one datagram to SOCKET's connected peer (see CONNECT!),
returning the number of bytes sent."
    (udp-send* socket bytes))

  (defun receive-from (socket maxlen)
    "Block for one datagram of at most MAXLEN bytes, returning a 3-element
list (bytes peer-address possibly-truncated-p): BYTES is an Array<Char> of
the received payload (possibly shorter than MAXLEN -- datagram boundaries
are preserved, never coalesced with another datagram), PEER-ADDRESS is a
NET:ADDRESS, and POSSIBLY-TRUNCATED-P is true exactly when (= (length
bytes) maxlen) -- in that case the original datagram may have been larger
than MAXLEN and silently truncated by the OS; pass a MAXLEN comfortably
larger than the expected payload to disambiguate."
    (let ((result (udp-receive-from* socket maxlen)))
      (list (car result) (net:address-from-triple (cadr result)) (caddr result))))

  (defun close! (socket)
    "Close SOCKET. Idempotent, like PORTS:CLOSE!; every subsequent
SEND/SEND-TO/RECEIVE-FROM on this SOCKET errors immediately with a
:CLOSED error."
    (net-handle-close* socket))

  (defun socket-p (x)
    "T if X is a UDP socket handle (as returned by BIND)."
    (and (net-handle-p* x) (eq (net-handle-kind* x) 'udp-socket)))

  (defun socket-open-p (socket)
    (net-handle-open-p* socket))

  (defun local-addr (resource)
    "The local NET:ADDRESS SOCKET is bound to."
    (net:local-addr resource))

  (defun set-timeout! (socket ms)
    "Set SOCKET's read and write timeout in milliseconds; NIL blocks without
a timeout (the default). A timed-out RECEIVE-FROM/SEND/SEND-TO signals a
structured :TIMEOUT error."
    (udp-set-timeout* socket ms))

  )

(provide 'udp '(udp:bind udp:connect! udp:send-to udp:send udp:receive-from
                udp:close! udp:socket-p udp:socket-open-p udp:local-addr
                udp:set-timeout!))
