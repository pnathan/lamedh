;;; NET module — addresses and DNS resolution (issue #258, epic #253).
;;;
;;; WHY A MODULE: like lib/31-ports.lisp, this is a genuinely new facility
;;; layered on the kernel, not a completion of an existing flat name, so per
;;; the epic #253 namespace ruling it lives under a module.
;;;
;;; ADDRESSES: NET:ADDRESS is a DEFRECORD (the one record form as of 0.3 --
;;; see lib/20-condensation.lisp) with three fields: FAMILY (:IPV4 or :IPV6,
;;; a keyword-style symbol), IP (a String, e.g. "127.0.0.1" or "::1" --
;;; never bracketed; bracketing is ADDRESS->STRING's job on output, not part
;;; of the stored value), and PORT (an integer 0-65535). This satisfies the
;;; issue's "first-class, printable IP/socket-address data represented
;;; without exposing platform structs": the kernel (RUST-*, below) never
;;; returns a Rust std::net::SocketAddr to Lisp, only a raw
;;; (family ip port) triple, which this module wraps into the branded
;;; record. TCP:*/UDP:* re-export ADDRESS via LOCAL-ADDR/PEER-ADDR instead
;;; of duplicating the wrapper.
;;;
;;; CAPABILITIES: RESOLVE needs NET-DNS -- explicit hostname resolution,
;;; the epic's narrowest networking authority. LOCAL-ADDR/PEER-ADDR need no
;;; capability: inspecting an address on a resource you already hold is
;;; "continue" authority, exactly like reading PORT-NAME on an open port
;;; (issue #255's "a successfully returned handle is authority to continue"
;;; rule) -- see src/evaluator/builtins_net.rs's module header for the full
;;; capability/policy model shared by NET/TCP/UDP.
;;;
;;; The kernel primitives this wraps (NET-RESOLVE*, NET-LOCAL-ADDR*,
;;; NET-PEER-ADDR*) live in Rust because DNS resolution and the actual
;;; std::net address types are representation-access work the Lisp layer
;;; cannot do on its own; see src/evaluator/builtins_net.rs.
;;;
;;; REQUIRE-ABLE (issue #256): `(require 'net)` on a `with_prelude()`
;;; environment loads exactly this file. It requires 'modules and
;;; 'condensation (for DEFRECORD) first.

(require 'modules)
(require 'condensation)

(defmodule net
  (:export address address-p address-family address-ip address-port
           make-address address->string address-from-triple resolve
           local-addr peer-addr)
  (:requires net-dns))

(with-module net

  (defrecord address
    (family symbol)
    (ip string)
    (port int64)
    (:invariant (and (>= port 0) (<= port 65535))))

  (defun address-from-triple (triple)
    "TRIPLE is the kernel's raw (family ip port) list (NET-RESOLVE*,
NET-LOCAL-ADDR*, NET-PEER-ADDR*); wrap it as a NET:ADDRESS."
    (make-address (car triple) (cadr triple) (caddr triple)))

  (defun $triples->addresses (lst)
    (mapcar #'address-from-triple lst))

  (defun resolve (host &optional port)
    "Resolve HOST (a hostname or literal IPv4/IPv6 address string) and
optional service PORT (default 0) to an ordered list of NET:ADDRESS
records, via the system resolver. Requires NET-DNS. Signals a structured
error (:CATEGORY :DNS) if resolution fails -- e.g. an unknown or malformed
host; never depends on any specific external DNS answer, only on whether
resolution succeeds."
    ($triples->addresses (net-resolve* host (or port 0))))

  (defun local-addr (resource)
    "The local NET:ADDRESS a connected TCP port (see TCP:CONNECT/TCP:ACCEPT)
or a TCP/UDP network handle (TCP:LISTEN/UDP:BIND) is bound to."
    (address-from-triple (net-local-addr* resource)))

  (defun peer-addr (port)
    "The remote NET:ADDRESS a connected TCP PORT (see TCP:CONNECT/
TCP:ACCEPT) is connected to."
    (address-from-triple (net-peer-addr* port)))

  (defun address->string (addr)
    "Format ADDR as \"ip:port\", bracketing an IPv6 host per the usual
[ipv6]:port convention (e.g. \"[::1]:8080\"; an IPv4 host is unbracketed,
e.g. \"127.0.0.1:8080\")."
    (if (eq (address-family addr) ':ipv6)
        (concat "[" (address-ip addr) "]:" (princ-to-string (address-port addr)))
        (concat (address-ip addr) ":" (princ-to-string (address-port addr)))))

  )

(provide 'net '(net:address net:address-p net:address-family net:address-ip
                net:address-port net:make-address net:address->string
                net:address-from-triple net:resolve net:local-addr
                net:peer-addr))
