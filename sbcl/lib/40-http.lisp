;;; HTTP module — capability-gated streaming HTTP/1.1 client and server
;;; (issue #259, epic #253), layered entirely on TCP (lib/38-tcp.lisp,
;;; issue #258), PORTS (lib/31-ports.lisp, issue #255), and the URL/MIME/
;;; JSON codecs (lib/34-url.lisp, lib/36-mime.lisp, lib/35-json.lisp, issue
;;; #257).
;;;
;;; SCOPE RULINGS (binding, recorded here so the "why" survives the code):
;;;
;;; - https:// CLIENT SUPPORT WHEN net-tls IS COMPILED IN (issue #365). The
;;;   TLS dependency ruling landed rustls behind the off-by-default net-tls
;;;   cargo feature (lib/43-tls.lisp, the TLS module) -- $HTTP-CHECK-SCHEME!
;;;   (the single scheme checkpoint the earlier plaintext-only design
;;;   deliberately isolated for exactly this) now checks
;;;   `(tls:available-p)` at request time: with the feature compiled in, an
;;;   https:// URL -- the initial request or a redirect Location alike --
;;;   connects via TCP:CONNECT then wraps with TLS:CONNECT (certificate
;;;   verification on, SNI/ALPN "http/1.1") before speaking HTTP/1.1 over
;;;   the resulting port -- the same PORTS port shape, so nothing else in
;;;   this file changes. With the feature compiled out, https:// still
;;;   signals the same clear structured `:HTTPS-UNSUPPORTED` error as
;;;   before (now naming the `net-tls` cargo feature instead of issue #365,
;;;   since #365 is what added the feature). Redirects never silently cross
;;;   from https:// to http:// or vice versa -- $HTTP-CHECK-SCHEME! is
;;;   still the one enforcement point, run again on every redirect hop.
;;;   SERVER-SIDE https:// (wrapping an accepted connection before SERVE/
;;;   SERVE-ONE! parses it) is explicitly out of scope here: the ruling
;;;   text scopes this addition to the client ("after tcp:connect, wrap via
;;;   tls: before speaking HTTP"), and TLS:WRAP-SERVER already composes
;;;   with TCP:ACCEPT directly for a caller that wants a TLS server without
;;;   going through this module's SERVE/SERVE-ONE! (which only ever accepts
;;;   plaintext connections themselves).
;;;
;;; - ZERO NEW CRATE DEPENDENCIES, PURE LISP. The ticket's own text asks
;;;   for "a maintained Rust HTTP implementation" behind a Cargo feature;
;;;   the epic's #259 disposition (binding, overriding the ticket here)
;;;   is the opposite: no new Cargo dependency, HTTP/1.1 framing done in
;;;   the Lisp layer over TCP/PORTS exactly like URL/MIME/JSON/BASE64/HEX
;;;   already are. Status-line/header parsing, chunked transfer-coding
;;;   (size line, chunk data, trailers, terminator), and request/response
;;;   writing are all ordinary Lisp string/byte work -- nothing here proved
;;;   "genuinely impractical" in Lisp, so no new Rust kernel builtin was
;;;   added for any of it (recording the decision either way, per the
;;;   scope ruling).
;;;
;;; - NO NEW CAPABILITY. Requiring this module grants no network authority
;;;   by itself: the client's only host-facing operation is TCP:CONNECT
;;;   (gated on NET-CONNECT), the server's is TCP:LISTEN/TCP:ACCEPT (gated
;;;   on NET-LISTEN) -- both already-existing gates from #258, enforced
;;;   exactly as TCP enforces them (including WITH-CAPABILITIES fence
;;;   attenuation, since this module never bypasses TCP:CONNECT/TCP:LISTEN
;;;   to reach the socket layer directly).
;;;
;;; - STREAMING BODIES ARE A LISP-LEVEL PSEUDO-PORT, NOT A PORTS:PORT-P
;;;   VALUE. The ticket asks for response/request bodies "exposed as a
;;;   readable Port." Making that a literal new PortState variant is Rust
;;;   kernel surface this ticket's pure-Lisp/zero-new-Rust-surface framing
;;;   scope doesn't call for and the time budget doesn't afford; instead a
;;;   body stream here is a small mutable Lisp object (a 5-slot Array; see
;;;   $HTTP-MAKE-STREAM) with its own STREAM-READ!/STREAM-READ-ALL!/
;;;   STREAM-EOF-P/STREAM-CLOSE! operations that mirror PORTS' naming and
;;;   semantics closely enough to be a drop-in mental model, while making
;;;   the framing-awareness (Content-Length exact / chunked / read-to-close
;;;   / none) explicit instead of hidden inside a Rust enum variant. Wiring
;;;   an actual PortState::HttpBody variant so PORTS:PORT-P (and every
;;;   other PORTS operation) recognizes it too is a reasonable Rust-side
;;;   follow-up, explicitly out of scope here.
;;;
;;; - CLIENT: always `Connection: close`, no connection pooling/reuse. This
;;;   is a deliberate simplification, not laziness: with no keep-alive
;;;   reuse, every redirect hop is a fresh TCP:CONNECT, so there is never a
;;;   need to drain an unwanted response body before reusing a socket, and
;;;   the client never has to lie about a Connection header it can't honor.
;;;   Framing-correct reads (never read-to-EOF when Content-Length/chunked
;;;   framing says otherwise) still matter regardless -- a keep-alive
;;;   SERVER on the other end will not close just because this client
;;;   intends to.
;;;
;;; - SERVER: real keep-alive. One accepted connection is served fully
;;;   (every pipelined/keep-alive request on it, in order) before the next
;;;   is accepted -- "a minimal implementation may serve serially for
;;;   correctness" is the ticket's own text; concurrent serving via
;;;   share-nothing workers is issue #140's business, explicitly out of
;;;   scope. Unread request-body bytes are drained after the handler runs
;;;   so the next pipelined request-line parses cleanly.
;;;
;;; - GRACEFUL SHUTDOWN, HONESTLY SCOPED. SERVE's :STOP-P predicate is
;;;   consulted BETWEEN connections (never during one, and never able to
;;;   interrupt an in-progress blocking ACCEPT -- there is no portable
;;;   wakeup-on-close for that without a second OS thread; see
;;;   TCP:CLOSE-LISTENER!'s own documented limitation). Because serving is
;;;   already fully serial, "drain active requests" is automatic: there is
;;;   never more than one in flight. :MAX-REQUESTS bounds the loop
;;;   deterministically for tests.
;;;
;;; - REDIRECT RESOLUTION IS SCOPED, NOT SILENTLY WRONG. Location headers
;;;   are resolved as: fully absolute (has a scheme), protocol-relative
;;;   ("//host/path", inherits the current scheme), or absolute-path
;;;   ("/path", inherits the current origin). A bare relative reference
;;;   ("foo", "../foo") -- full RFC 3986 §5.3 merge/normalize -- is an
;;;   explicit, named error rather than a guessed resolution; real-world
;;;   Location headers are overwhelmingly absolute or absolute-path.
;;;
;;; REQUIRE-ABLE (issue #256): `(require 'http)` on a `with_prelude()`
;;; environment loads exactly this file (after 'modules 'text 'ports 'url
;;; 'mime 'tcp 'tls 'json, each already require-idempotent). 'tls loads
;;; unconditionally regardless of the net-tls cargo feature (see its own
;;; file header) -- requiring it here grants no capability and costs
;;; nothing when the feature is off.

(require 'modules)
(require 'text)
(require 'ports)
(require 'url)
(require 'mime)
(require 'tls)
(require 'tcp)
(require 'json)

;; GET is deliberately NOT exported: the Prelude already binds flat GET (a
;; builtin alias of GETP, the plist reader), and IMPORT binds every export
;; globally by value -- exporting ours would silently clobber it (the exact
;; precedent of lib/31-ports.lisp's unexported POSITION). Call (HTTP:GET url)
;; qualified; every other name here has no flat collision and is exported
;; normally.
(defmodule http
  (:export request post
           response-status response-reason response-version response-headers
           response-header response-body
           request-method request-target request-path request-query
           request-headers request-header request-body request-version
           request-peer-addr request-url
           stream-read! stream-read-all! stream-eof-p stream-close!
           collect-bytes collect-string collect-json
           serve serve-one! respond default-reason)
  (:requires net-connect net-listen))

(with-module http

;;; ==== tunable defaults ====================================================

(def $http-default-max-line-bytes 8192)
(def $http-default-max-header-count 200)
(def $http-default-max-body-bytes (* 10 1024 1024))
(def $http-default-max-redirects 5)
(def $http-default-idle-timeout-ms 30000)
(def $http-redirect-statuses '(301 302 303 307 308))

;;; ==== bounded CRLF line reader (client status/header lines, server
;;; request/header lines, chunk-size lines -- one reader, everywhere) ========
;;;
;;; Reads one byte at a time via PORTS:READ-BYTE! (mirrors lib/31-ports.lisp's
;;; own $READ-LINE-ACC! pattern exactly, proven tail-recursive/stack-safe at
;;; this evaluator's TCO), stopping at LF, stripping an optional trailing CR,
;;; and erroring (rather than buffering without bound) if MAX-BYTES is
;;; exceeded before a terminator is found -- the "no unbounded buffering"
;;; security requirement applied uniformly to both sides.

(defun $http-strip-trailing-cr (bytes)
  (if (null bytes)
      bytes
      (let ((rev (reverse bytes)))
        (if (= (car rev) 13) (reverse (cdr rev)) bytes))))

(defun $http-finish-line (bytes)
  (text:utf8->string-lossy (list->array (mapcar #'make-char ($http-strip-trailing-cr bytes)))))

(defun $http-read-line-acc! (port acc n max-bytes)
  (if (> n max-bytes)
      (error (concat "HTTP: a line exceeded " (princ-to-string max-bytes) " bytes without a terminating LF")
             (list (cons ':category ':line-too-long) (cons ':max-bytes max-bytes)))
      (let ((b (ports:read-byte! port)))
        (cond
          ((null b) (if (= n 0) nil ($http-finish-line (reverse acc))))
          ((= b 10) ($http-finish-line (reverse acc)))
          (t ($http-read-line-acc! port (cons b acc) (+ n 1) max-bytes))))))

(defun $http-read-line! (port max-bytes)
  "Read one CRLF- or LF-terminated line from PORT (a TCP PORTS port),
decoded UTF-8 (lossy), with the terminator (and any trailing CR) stripped.
NIL only at true EOF (no bytes read at all); a final unterminated line at
EOF is still returned once, matching PORTS:READ-LINE!'s own convention."
  ($http-read-line-acc! port () 0 max-bytes))

;;; ==== header block parse/build (values via MIME's ordered multi-value
;;; alist representation -- lib/36-mime.lisp) ================================

(defun $http-split-header-line (line)
  (let ((idx (string-index-of line ":")))
    (if (null idx)
        (error (concat "HTTP: malformed header line (missing ':'): " (prin1-to-string line))
               (list (cons ':category ':bad-header) (cons ':line line)))
        (cons (string-trim (substring line 0 idx))
              (string-trim (substring line (+ idx 1) (string-length* line)))))))

(defun $http-read-headers-acc! (port max-line-bytes max-count acc count)
  (let ((line ($http-read-line! port max-line-bytes)))
    (cond
      ((null line) (error "HTTP: connection closed while reading headers"
                           (list (cons ':category ':closed))))
      ((string-empty-p line) (reverse acc))
      ((>= count max-count)
       (error (concat "HTTP: too many header fields (limit " (princ-to-string max-count) ")")
              (list (cons ':category ':too-many-headers) (cons ':limit max-count))))
      (t ($http-read-headers-acc! port max-line-bytes max-count
                                   (cons ($http-split-header-line line) acc) (+ count 1))))))

(defun $http-read-headers! (port max-line-bytes max-count)
  ($http-read-headers-acc! port max-line-bytes max-count () 0))

(defun $http-write-headers! (port hdrs)
  (if (null hdrs)
      nil
      (progn
        (ports:write-string! port (concat (car (car hdrs)) ": " (princ-to-string (cdr (car hdrs))) "\r\n"))
        ($http-write-headers! port (cdr hdrs)))))

;;; ==== byte-buffer concatenation: O(total-length), tail-recursive =========
;;; Used by STREAM-READ-ALL! to merge accumulated read chunks into one
;;; Array<Char> without repeated whole-array copies.

(defun $http-chunks-total-len (chunks acc)
  (if (null chunks) acc ($http-chunks-total-len (cdr chunks) (+ acc (array-length* (car chunks))))))

(defun $http-copy-array-into! (dest offset src i n)
  (if (= i n)
      nil
      (progn (aset dest (+ offset i) (aref src i))
             ($http-copy-array-into! dest offset src (+ i 1) n))))

(defun $http-copy-chunks-into! (dest offset chunks)
  (if (null chunks)
      dest
      (let* ((src (car chunks)) (n (array-length* src)))
        ($http-copy-array-into! dest offset src 0 n)
        ($http-copy-chunks-into! dest (+ offset n) (cdr chunks)))))

(defun $http-concat-byte-chunks (chunks)
  (let ((dest (make-array ($http-chunks-total-len chunks 0))))
    ($http-copy-chunks-into! dest 0 chunks)
    dest))

;;; ==== chunk-size hex parsing (a numeral, not a byte codec -- HEX:DECODE
;;; is bytes-from-hex-pairs, a different operation) ===========================

(defun $http-hex-digit-value (c)
  (let ((code (char->code c)))
    (cond
      ((and (>= code 48) (<= code 57)) (- code 48))
      ((and (>= code 97) (<= code 102)) (+ 10 (- code 97)))
      ((and (>= code 65) (<= code 70)) (+ 10 (- code 65)))
      (t (error (concat "HTTP: invalid chunk-size hex digit " (prin1-to-string c))
                (list (cons ':category ':bad-chunk-size)))))))

(defun $http-hex-chars->number-acc (chars acc)
  (if (null chars)
      acc
      ($http-hex-chars->number-acc (cdr chars) (+ (* acc 16) ($http-hex-digit-value (car chars))))))

(defun $http-parse-chunk-size-line (line)
  (let* ((semi (string-index-of line ";"))
         (size-str (string-trim (if semi (substring line 0 semi) line))))
    (if (string-empty-p size-str)
        (error "HTTP: empty chunk-size line" (list (cons ':category ':bad-chunk-size)))
        ($http-hex-chars->number-acc (string->list size-str) 0))))

(defun $http-number->hex-acc (n acc)
  (if (= n 0) acc ($http-number->hex-acc (/ n 16) (concat (char-at "0123456789abcdef" (mod n 16)) acc))))

(defun $http-number->hex-lower (n)
  (if (= n 0) "0" ($http-number->hex-acc n "")))

;;; ==== body streams: a small mutable "pseudo-port" over PORTS + framing ===
;;;
;;; Representation: a 5-slot Array [KIND PORT STATE-A STATE-B OWNS-PORT-P].
;;; KIND is :CONTENT-LENGTH / :CHUNKED / :CLOSE / :NONE. STATE-A is the
;;; mutable remaining-byte count (Content-Length) or remaining-in-current-
;;; chunk count (chunked). STATE-B is the chunked "done" flag (T once the
;;; zero-length terminator chunk and trailers have been consumed).
;;; OWNS-PORT-P controls whether STREAM-CLOSE! closes the underlying TCP
;;; port: true for a client response body (the client never reuses a
;;; connection), false for a server request body (the connection stays
;;; open to serve the response and possibly further keep-alive requests).

(defun $http-make-stream (kind port state-a state-b owns-port)
  (let ((s (make-array 5)))
    (aset s 0 kind) (aset s 1 port) (aset s 2 state-a) (aset s 3 state-b) (aset s 4 owns-port)
    s))

(defun $http-stream-kind (s) (aref s 0))
(defun $http-stream-port (s) (aref s 1))
(defun $http-stream-a (s) (aref s 2))
(defun $http-stream-b (s) (aref s 3))
(defun $http-stream-owns (s) (aref s 4))
(defun $http-stream-set-a! (s v) (aset s 2 v))
(defun $http-stream-set-b! (s v) (aset s 3 v))

(defun $http-consume-chunk-crlf! (port)
  (let ((b1 (ports:read-byte! port)))
    (cond
      ((null b1) (error "HTTP: connection closed mid-chunk-terminator" (list (cons ':category ':truncated-body))))
      ((= b1 13)
       (let ((b2 (ports:read-byte! port)))
         (if (and b2 (= b2 10))
             nil
             (error "HTTP: malformed chunk terminator (expected CRLF)" (list (cons ':category ':bad-chunk-terminator))))))
      ((= b1 10) nil)
      (t (error "HTTP: malformed chunk terminator (expected CRLF)" (list (cons ':category ':bad-chunk-terminator)))))))

(defun $http-read-trailers! (port)
  (let ((line ($http-read-line! port $http-default-max-line-bytes)))
    (cond
      ((null line) nil)
      ((string-empty-p line) nil)
      (t ($http-read-trailers! port)))))

(defun $http-stream-read-chunk-body! (s n)
  (let* ((remaining ($http-stream-a s))
         (want (if (< n remaining) n remaining))
         (got (ports:read-bytes! ($http-stream-port s) want))
         (got-n (array-length* got)))
    (if (and (= got-n 0) (> remaining 0))
        (error "HTTP: connection closed mid-chunk" (list (cons ':category ':truncated-body)))
        (progn
          ($http-stream-set-a! s (- remaining got-n))
          (if (= (- remaining got-n) 0) ($http-consume-chunk-crlf! ($http-stream-port s)) nil)
          got))))

(defun $http-stream-begin-chunk! (s n)
  (let* ((port ($http-stream-port s))
         (line ($http-read-line! port $http-default-max-line-bytes)))
    (if (null line)
        (error "HTTP: connection closed while reading a chunk size" (list (cons ':category ':truncated-body)))
        (let ((size ($http-parse-chunk-size-line line)))
          (if (= size 0)
              (progn ($http-read-trailers! port) ($http-stream-set-b! s t) (list->array ()))
              (progn ($http-stream-set-a! s size) ($http-stream-read-chunk-body! s n)))))))

(defun $http-stream-read-chunked! (s n)
  (cond
    (($http-stream-b s) (list->array ()))
    ((> ($http-stream-a s) 0) ($http-stream-read-chunk-body! s n))
    (t ($http-stream-begin-chunk! s n))))

(defun $http-stream-read-cl! (s n)
  (let ((remaining ($http-stream-a s)))
    (if (<= remaining 0)
        (list->array ())
        (let* ((want (if (< n remaining) n remaining))
               (got (ports:read-bytes! ($http-stream-port s) want))
               (got-n (array-length* got)))
          (if (and (= got-n 0) (> remaining 0))
              (error "HTTP: connection closed before Content-Length body was fully read"
                     (list (cons ':category ':truncated-body) (cons ':remaining remaining)))
              (progn ($http-stream-set-a! s (- remaining got-n)) got))))))

(defun stream-read! (s n)
  "Read up to N bytes from body stream S, honoring its message framing
(Content-Length exact / chunked / read-to-close / no body) -- returning a
fresh Array<Char> that may be shorter than N (including empty exactly at
the logical end of THIS message's body), mirroring PORTS:READ-BYTES!'s own
convention. Never reads past this message's body into whatever the
connection carries next."
  (cond
    ((eq ($http-stream-kind s) ':none) (list->array ()))
    ((eq ($http-stream-kind s) ':content-length) ($http-stream-read-cl! s n))
    ((eq ($http-stream-kind s) ':chunked) ($http-stream-read-chunked! s n))
    ((eq ($http-stream-kind s) ':close) (ports:read-bytes! ($http-stream-port s) n))
    (t (error "HTTP: internal error: unknown body stream kind" (list (cons ':category ':internal))))))

(defun stream-eof-p (s)
  "T if body stream S has reached the logical end of its message body.
For :CLOSE-framed bodies (no Content-Length/chunked framing given) this is
best-effort: definitive only after a STREAM-READ! call has returned an
empty Array<Char>."
  (cond
    ((eq ($http-stream-kind s) ':none) t)
    ((eq ($http-stream-kind s) ':content-length) (<= ($http-stream-a s) 0))
    ((eq ($http-stream-kind s) ':chunked) ($http-stream-b s))
    (t nil)))

(defun stream-close! (s)
  "Close body stream S. For a client response body this closes the
underlying TCP connection (the client never reuses one); for a server
request body this is a no-op -- the connection is owned by the request/
response cycle, not the body stream (see the file header)."
  (if ($http-stream-owns s) (ports:close! ($http-stream-port s)) nil))

;;; ==== bounded collectors (String, Array<Char>, or parsed JSON) ===========

(defun $http-stream-read-all-acc! (s buf-size max-bytes chunks total)
  (if (stream-eof-p s)
      ($http-concat-byte-chunks (reverse chunks))
      (let* ((got (stream-read! s buf-size)) (got-n (array-length* got)))
        (cond
          ((= got-n 0) ($http-concat-byte-chunks (reverse chunks)))
          ((> (+ total got-n) max-bytes)
           (error (concat "HTTP: body exceeds max-bytes limit of " (princ-to-string max-bytes))
                  (list (cons ':category ':body-too-large) (cons ':max-bytes max-bytes))))
          (t ($http-stream-read-all-acc! s buf-size max-bytes (cons got chunks) (+ total got-n)))))))

(defun stream-read-all! (s &key (max-bytes $http-default-max-body-bytes))
  "Read body stream S to its logical end, tail-recursively and in
O(total-length) time, returning one fresh Array<Char>. Errors rather than
growing without bound once more than MAX-BYTES (default 10 MiB) has been
read -- the ticket's 'no unbounded body/header buffering' requirement."
  ($http-stream-read-all-acc! s 8192 max-bytes () 0))

(defun collect-bytes (s &key (max-bytes $http-default-max-body-bytes))
  "Collect body stream S into one Array<Char>, bounded by :MAX-BYTES."
  (stream-read-all! s :max-bytes max-bytes))

(defun collect-string (s &key (max-bytes $http-default-max-body-bytes) (lossy nil))
  "Collect body stream S and decode it as UTF-8 into a String, bounded by
:MAX-BYTES. Only UTF-8 (the TEXT module's own boundary, lib/30-text.lisp)
is supported for decoding; a declared non-UTF-8 charset is not
transcoded. :LOSSY (default NIL) selects TEXT:UTF8->STRING-LOSSY over the
strict TEXT:UTF8->STRING."
  (let ((bytes (collect-bytes s :max-bytes max-bytes)))
    (if lossy (text:utf8->string-lossy bytes) (text:utf8->string bytes))))

(defun collect-json (s &key (max-bytes $http-default-max-body-bytes))
  "Collect body stream S and parse it as JSON (JSON:PARSE, lib/35-json.lisp),
bounded by :MAX-BYTES. Decodes the body as UTF-8 lossily before parsing."
  (json:parse (collect-string s :max-bytes max-bytes :lossy t)))

;;; ==== request/response accessors (both request and response are plain
;;; alists, per the ticket's own "documented Lamedh structs or alists") =====

(defun response-status (r) (cdr (assoc ':status r)))
(defun response-reason (r) (cdr (assoc ':reason r)))
(defun response-version (r) (cdr (assoc ':version r)))
(defun response-headers (r) (cdr (assoc ':headers r)))
(defun response-header (r name) (mime:headers-get (response-headers r) name))
(defun response-body (r) (cdr (assoc ':body r)))

(defun request-method (r) (cdr (assoc ':method r)))
(defun request-target (r) (cdr (assoc ':target r)))
(defun request-path (r) (cdr (assoc ':path r)))
(defun request-query (r) (cdr (assoc ':query r)))
(defun request-headers (r) (cdr (assoc ':headers r)))
(defun request-header (r name) (mime:headers-get (request-headers r) name))
(defun request-body (r) (cdr (assoc ':body r)))
(defun request-version (r) (cdr (assoc ':version r)))
(defun request-peer-addr (r) (cdr (assoc ':peer-addr r)))
(defun request-url (r) (cdr (assoc ':url r)))

;;; ============================================================================
;;; CLIENT
;;; ============================================================================

(defun $http-check-scheme! (parsed url)
  (let ((sch (url:scheme parsed)))
    (cond
      ((null sch)
       (error (concat "HTTP: URL has no scheme: " (prin1-to-string url))
              (list (cons ':category ':bad-url) (cons ':url url))))
      ((string-ci= sch "http") nil)
      ((and (string-ci= sch "https") (tls:available-p)) nil)
      ((string-ci= sch "https")
       (error "HTTP: https:// requires the `net-tls` cargo feature, which this build of lamedh was not compiled with (issue #365); rebuild with --features net-tls, or use plaintext http://"
              (list (cons ':category ':https-unsupported) (cons ':url url))))
      (t (error (concat "HTTP: unsupported URL scheme " (prin1-to-string sch) " (only http:// and https:// are supported)")
                (list (cons ':category ':unsupported-scheme) (cons ':url url) (cons ':scheme sch)))))))

(defun $http-default-port (parsed)
  "80, or 443 for an https:// URL -- both the implicit connect port and the
threshold for omitting an explicit port from the Host header."
  (if (string-ci= (url:scheme parsed) "https") 443 80))

(defun $http-target-for (parsed)
  (let* ((p (url:path parsed))
         (path (if (string-empty-p p) "/" p))
         (q (url:query parsed)))
    (if q (concat path "?" q) path)))

(defun $http-host-header-value (parsed)
  (let ((h (url:host parsed)) (p (url:port parsed)))
    (if (or (null p) (= p ($http-default-port parsed))) h (concat h ":" (princ-to-string p)))))

(defun $http-body-length (body)
  (cond
    ((stringp body) (array-length* (text:string->utf8 body)))
    ((arrayp body) (array-length* body))
    (t (error (concat "HTTP: unsupported request body type " (prin1-to-string body))
              (list (cons ':category ':bad-body))))))

(defun $http-prepare-headers (hdrs parsed body)
  (let* ((h1 (if (mime:headers-get hdrs "Host") hdrs (cons (cons "Host" ($http-host-header-value parsed)) hdrs)))
         (h2 (cond
               ((null body) h1)
               ((ports:port-p body)
                (if (or (mime:headers-get h1 "Content-Length") (mime:headers-get h1 "Transfer-Encoding"))
                    h1
                    (mime:headers-add h1 "Transfer-Encoding" "chunked")))
               (t (if (or (mime:headers-get h1 "Content-Length") (mime:headers-get h1 "Transfer-Encoding"))
                      h1
                      (mime:headers-add h1 "Content-Length" (princ-to-string ($http-body-length body))))))))
    (mime:headers-set h2 "Connection" "close")))

(defun $http-write-chunked-body! (dest src)
  (let* ((chunk (ports:read-bytes! src 8192)) (n (array-length* chunk)))
    (if (= n 0)
        (ports:write-string! dest "0\r\n\r\n")
        (progn
          (ports:write-string! dest (concat ($http-number->hex-lower n) "\r\n"))
          (ports:write-bytes! dest chunk)
          (ports:write-string! dest "\r\n")
          ($http-write-chunked-body! dest src)))))

(defun $http-write-body! (port body)
  (cond
    ((null body) nil)
    ((stringp body) (ports:write-bytes! port (text:string->utf8 body)))
    ((ports:port-p body) ($http-write-chunked-body! port body))
    ((arrayp body) (ports:write-bytes! port body))
    (t (error (concat "HTTP: unsupported request body type " (prin1-to-string body))
              (list (cons ':category ':bad-body))))))

(defun $http-open-transport! (parsed host port connect-timeout-ms extra-roots)
  "The connected PORTS port to speak HTTP/1.1 over: a plain TCP:CONNECT for
http://, or TLS:CONNECT (verification on, SNI = HOST, ALPN \"http/1.1\",
:EXTRA-ROOTS forwarded from REQUEST) for https:// -- the single place
$HTTP-SEND-REQUEST! opens a connection, so the scheme decides transport
without touching anything downstream (the returned value is a PORTS port
either way)."
  (if (string-ci= (url:scheme parsed) "https")
      (tls:connect host port :connect-timeout-ms connect-timeout-ms :alpn '("http/1.1")
                    :extra-roots extra-roots)
      (tcp:connect host port connect-timeout-ms)))

(defun $http-send-request! (parsed method hdrs body connect-timeout-ms extra-roots)
  (let ((host (url:host parsed)) (port (if (url:port parsed) (url:port parsed) ($http-default-port parsed))))
    (if (null host)
        (error "HTTP: URL has no host" (list (cons ':category ':bad-url)))
        (let ((tcp-port ($http-open-transport! parsed host port connect-timeout-ms extra-roots)))
          (ports:write-string! tcp-port (concat method " " ($http-target-for parsed) " HTTP/1.1\r\n"))
          ($http-write-headers! tcp-port hdrs)
          (ports:write-string! tcp-port "\r\n")
          (ports:flush! tcp-port)
          ($http-write-body! tcp-port body)
          (ports:flush! tcp-port)
          tcp-port))))

(defun $http-parse-status-line (line)
  (let ((sp1 (string-index-of line " ")))
    (if (null sp1)
        (error (concat "HTTP: malformed status line: " (prin1-to-string line))
               (list (cons ':category ':bad-status-line)))
        (let* ((version (substring line 0 sp1))
               (rest (string-trim-left (substring line (+ sp1 1) (string-length* line))))
               (sp2 (string-index-of rest " "))
               (code-str (if sp2 (substring rest 0 sp2) rest))
               (reason (if sp2 (string-trim-left (substring rest (+ sp2 1) (string-length* rest))) ""))
               (code (parse-integer code-str)))
          (if (null code)
              (error (concat "HTTP: malformed status code in status line: " (prin1-to-string line))
                     (list (cons ':category ':bad-status-line)))
              (list version code reason))))))

(defun $http-read-status-once! (port max-line-bytes)
  (let ((line ($http-read-line! port max-line-bytes)))
    (if (null line)
        (error "HTTP: connection closed before a status line was received" (list (cons ':category ':closed)))
        ($http-parse-status-line line))))

(defun $http-read-status! (port max-line-bytes max-header-count)
  (let* ((parsed ($http-read-status-once! port max-line-bytes)) (code (cadr parsed)))
    (if (and (>= code 100) (< code 200))
        (progn ($http-read-headers! port max-line-bytes max-header-count)
               ($http-read-status! port max-line-bytes max-header-count))
        parsed)))

(defun $http-response-framing (method code hdrs)
  (cond
    ((string-ci= method "HEAD") (cons ':none nil))
    ((or (= code 204) (= code 304) (and (>= code 100) (< code 200))) (cons ':none nil))
    ((let ((te (mime:headers-get hdrs "Transfer-Encoding"))) (and te (contains-p (string-downcase te) "chunked")))
     (cons ':chunked nil))
    ((mime:headers-get hdrs "Content-Length")
     (let ((n (parse-integer (string-trim (mime:headers-get hdrs "Content-Length")))))
       (if (or (null n) (< n 0))
           (error (concat "HTTP: malformed Content-Length header: " (prin1-to-string (mime:headers-get hdrs "Content-Length")))
                  (list (cons ':category ':bad-content-length)))
           (cons ':content-length n))))
    (t (cons ':close nil))))

(defun $http-build-response-stream (tcp-port framing)
  (let ((kind (car framing)))
    (cond
      ((eq kind ':none) ($http-make-stream ':none tcp-port 0 nil t))
      ((eq kind ':content-length) ($http-make-stream ':content-length tcp-port (cdr framing) nil t))
      ((eq kind ':chunked) ($http-make-stream ':chunked tcp-port 0 nil t))
      ((eq kind ':close) ($http-make-stream ':close tcp-port nil nil t))
      (t (error "HTTP: internal error: unknown response framing kind" (list (cons ':category ':internal)))))))

(defun $http-origin (parsed)
  (list (string-downcase (if (url:scheme parsed) (url:scheme parsed) ""))
        (string-downcase (if (url:host parsed) (url:host parsed) ""))
        (if (url:port parsed) (url:port parsed) ($http-default-port parsed))))

(defun $http-same-origin-p (a b) (equal ($http-origin a) ($http-origin b)))

(defun $http-strip-credentials (hdrs)
  (mime:headers-remove (mime:headers-remove (mime:headers-remove hdrs "Authorization") "Cookie") "Proxy-Authorization"))

(defun $http-resolve-location (parsed location)
  "Resolve a Location header value against the current hop's PARSED URL --
see the file header's REDIRECT RESOLUTION note for exactly which forms are
supported."
  (let ((loc-parsed (url:parse location)))
    (cond
      ((url:scheme loc-parsed) loc-parsed)
      ((url:host loc-parsed)
       (list (cons 'scheme (url:scheme parsed)) (cons 'userinfo (url:userinfo loc-parsed))
             (cons 'host (url:host loc-parsed)) (cons 'port (url:port loc-parsed))
             (cons 'path (url:path loc-parsed)) (cons 'query (url:query loc-parsed))
             (cons 'fragment (url:fragment loc-parsed))))
      ((starts-with-p location "/")
       (list (cons 'scheme (url:scheme parsed)) (cons 'userinfo nil)
             (cons 'host (url:host parsed)) (cons 'port (url:port parsed))
             (cons 'path (url:path loc-parsed)) (cons 'query (url:query loc-parsed))
             (cons 'fragment (url:fragment loc-parsed))))
      (t (error (concat "HTTP: unsupported relative redirect target (must be absolute or start with '/'): "
                        (prin1-to-string location))
                (list (cons ':category ':unsupported-redirect) (cons ':location location)))))))

(defun $http-redirect-method-and-body (code method body)
  (cond
    ((= code 303) (cons "GET" nil))
    ((and (or (= code 301) (= code 302)) (string-ci= method "POST")) (cons "GET" nil))
    ((or (= code 307) (= code 308))
     (if (or (null body) (stringp body) (arrayp body))
         (cons method body)
         (error "HTTP: cannot follow a 307/308 redirect that would replay a streamed (Port) request body -- it was already consumed sending the first hop"
                (list (cons ':category ':unreplayable-redirect-body)))))
    (t (cons method body))))

(defun $http-check-deadline! (deadline)
  (if (and deadline (> (monotonic-micros) deadline))
      (error "HTTP: overall request deadline exceeded" (list (cons ':category ':deadline-exceeded)))
      nil))

(defun $http-request-loop (method url hdrs body connect-timeout-ms read-timeout-ms max-redirects
                            follow-redirects max-line-bytes max-header-count hop deadline extra-roots)
  ($http-check-deadline! deadline)
  (let ((parsed (url:parse url)))
    ($http-check-scheme! parsed url)
    (let* ((sent-headers ($http-prepare-headers hdrs parsed body))
           (tcp-port ($http-send-request! parsed method sent-headers body connect-timeout-ms extra-roots)))
      (if read-timeout-ms (tcp:set-read-timeout! tcp-port read-timeout-ms) nil)
      (let* ((status ($http-read-status! tcp-port max-line-bytes max-header-count))
             (version (car status)) (code (cadr status)) (reason (caddr status))
             (resp-headers ($http-read-headers! tcp-port max-line-bytes max-header-count))
             (framing ($http-response-framing method code resp-headers))
             (location (mime:headers-get resp-headers "Location"))
             (is-redirect (and follow-redirects (member code $http-redirect-statuses) location)))
        (if (not is-redirect)
            (list (cons ':status code) (cons ':reason reason) (cons ':version version)
                  (cons ':headers resp-headers) (cons ':body ($http-build-response-stream tcp-port framing))
                  (cons ':request (list (cons ':method method) (cons ':url url)
                                         (cons ':target ($http-target-for parsed))
                                         (cons ':path (url:path parsed)) (cons ':query (url:query parsed))
                                         (cons ':headers sent-headers) (cons ':body body)
                                         (cons ':version "HTTP/1.1") (cons ':peer-addr nil))))
            (if (>= hop max-redirects)
                (error (concat "HTTP: too many redirects (limit " (princ-to-string max-redirects) ")")
                       (list (cons ':category ':too-many-redirects) (cons ':url url) (cons ':location location)))
                (let ((discard ($http-build-response-stream tcp-port framing)))
                  (stream-close! discard)
                  (let* ((next-parsed ($http-resolve-location parsed location)))
                    ($http-check-scheme! next-parsed location)
                    (let* ((next-url (url:build next-parsed))
                           (adj ($http-redirect-method-and-body code method body))
                           (next-method (car adj)) (next-body (cdr adj))
                           (cross-origin (not ($http-same-origin-p parsed next-parsed)))
                           (next-headers (if cross-origin ($http-strip-credentials hdrs) hdrs)))
                      ($http-request-loop next-method next-url next-headers next-body connect-timeout-ms
                                           read-timeout-ms max-redirects follow-redirects max-line-bytes
                                           max-header-count (+ hop 1) deadline extra-roots))))))))))

(defun request (method url &key (headers ()) (body nil) (connect-timeout-ms nil) (read-timeout-ms nil)
                (overall-timeout-ms nil) (max-redirects $http-default-max-redirects) (follow-redirects t)
                (max-line-bytes $http-default-max-line-bytes) (max-header-count $http-default-max-header-count)
                (extra-roots ()))
  "Perform an HTTP/1.1 request: METHOD (a string, e.g. \"GET\") against URL
(http:// always; https:// when the `net-tls` cargo feature is compiled in
-- see the file header). :HEADERS is a list of (name . value) conses
(repeated names preserved, mirroring MIME's representation); :BODY is NIL,
a String, an Array<Char>, or a readable PORTS port (streamed via chunked
Transfer-Encoding). :CONNECT-TIMEOUT-MS bounds TCP:CONNECT; :READ-TIMEOUT-MS
bounds every individual socket read (status line, headers, and each body
read alike); :OVERALL-TIMEOUT-MS is a coarse wall-clock deadline checked
between phases (connect, and each redirect hop) via MONOTONIC-MICROS -- not
a preemptive mid-read cancellation, which :READ-TIMEOUT-MS already covers
at finer grain. :MAX-REDIRECTS (default 5) caps 301/302/303/307/308 hops
when :FOLLOW-REDIRECTS (default T) is on; cross-origin hops strip
Authorization/Cookie/Proxy-Authorization, and a redirect Location naming an
unsupported scheme is the same structured error as passing it to URL
directly -- https:// is never silently crossed to/from http:// on a
redirect. :EXTRA-ROOTS is forwarded verbatim to TLS:CONNECT for an
https:// URL (ignored for http://) -- see TLS:WRAP-CLIENT for its shape;
this is how a caller (or a test harness) trusts a private/throwaway CA. The
returned response's :BODY is an UNREAD body stream (see STREAM-READ!/
STREAM-READ-ALL!/COLLECT-BYTES/COLLECT-STRING/COLLECT-JSON) -- streaming
end to end, per the ticket."
  (let ((deadline (if overall-timeout-ms (+ (monotonic-micros) (* overall-timeout-ms 1000)) nil)))
    ($http-request-loop (string-upcase method) url headers body connect-timeout-ms read-timeout-ms
                         max-redirects follow-redirects max-line-bytes max-header-count 0 deadline
                         extra-roots)))

(defun get (url &key (headers ()) (connect-timeout-ms nil) (read-timeout-ms nil) (overall-timeout-ms nil)
            (max-redirects $http-default-max-redirects) (follow-redirects t) (extra-roots ()))
  "Ergonomic (request \"GET\" url ...); see REQUEST for every keyword."
  (request "GET" url :headers headers :connect-timeout-ms connect-timeout-ms :read-timeout-ms read-timeout-ms
           :overall-timeout-ms overall-timeout-ms :max-redirects max-redirects :follow-redirects follow-redirects
           :extra-roots extra-roots))

(defun post (url &key (headers ()) (body nil) (connect-timeout-ms nil) (read-timeout-ms nil)
             (overall-timeout-ms nil) (max-redirects $http-default-max-redirects) (follow-redirects t)
             (extra-roots ()))
  "Ergonomic (request \"POST\" url :body body ...); see REQUEST for every
keyword."
  (request "POST" url :headers headers :body body :connect-timeout-ms connect-timeout-ms
           :read-timeout-ms read-timeout-ms :overall-timeout-ms overall-timeout-ms
           :max-redirects max-redirects :follow-redirects follow-redirects :extra-roots extra-roots))

;;; ============================================================================
;;; SERVER
;;; ============================================================================

(def $http-status-reasons
  (list (cons 100 "Continue") (cons 101 "Switching Protocols")
        (cons 200 "OK") (cons 201 "Created") (cons 202 "Accepted") (cons 204 "No Content")
        (cons 206 "Partial Content")
        (cons 301 "Moved Permanently") (cons 302 "Found") (cons 303 "See Other")
        (cons 304 "Not Modified") (cons 307 "Temporary Redirect") (cons 308 "Permanent Redirect")
        (cons 400 "Bad Request") (cons 401 "Unauthorized") (cons 403 "Forbidden") (cons 404 "Not Found")
        (cons 405 "Method Not Allowed") (cons 408 "Request Timeout") (cons 409 "Conflict")
        (cons 411 "Length Required") (cons 413 "Payload Too Large") (cons 414 "URI Too Long")
        (cons 415 "Unsupported Media Type") (cons 431 "Request Header Fields Too Large")
        (cons 500 "Internal Server Error") (cons 501 "Not Implemented") (cons 502 "Bad Gateway")
        (cons 503 "Service Unavailable") (cons 504 "Gateway Timeout")))

(defun default-reason (code)
  "The standard reason phrase for STATUS CODE, or \"\" if not in the
built-in table (still a perfectly legal HTTP/1.1 response -- RFC 7230
requires a reason PHRASE be present, not any particular text)."
  (let ((hit (assoc code $http-status-reasons))) (if hit (cdr hit) "")))

(defun respond (status &key (headers ()) (body nil) (reason nil))
  "Build a response alist for a server HANDLER to return: STATUS (an
integer), :HEADERS (a list of (name . value) conses), :BODY (NIL, a
String, an Array<Char>, or a readable PORTS port -- streamed via chunked
Transfer-Encoding unless :HEADERS already sets an explicit Content-Length),
:REASON (defaults to DEFAULT-REASON)."
  (list (cons ':status status) (cons ':reason (if reason reason (default-reason status)))
        (cons ':headers headers) (cons ':body body)))

(defun $http-parse-request-line (line)
  (let ((sp1 (string-index-of line " ")))
    (if (null sp1)
        (error (concat "HTTP: malformed request line: " (prin1-to-string line))
               (list (cons ':category ':bad-request)))
        (let* ((method (substring line 0 sp1))
               (rest (substring line (+ sp1 1) (string-length* line)))
               (sp2 (string-index-of rest " ")))
          (if (null sp2)
              (error (concat "HTTP: malformed request line: " (prin1-to-string line))
                     (list (cons ':category ':bad-request)))
              (list method (substring rest 0 sp2) (substring rest (+ sp2 1) (string-length* rest))))))))

(defun $http-split-target (target)
  (let ((q (string-index-of target "?")))
    (if q
        (cons (substring target 0 q) (substring target (+ q 1) (string-length* target)))
        (cons target nil))))

(defun $http-request-body-framing (hdrs)
  (cond
    ((let ((te (mime:headers-get hdrs "Transfer-Encoding"))) (and te (contains-p (string-downcase te) "chunked")))
     (cons ':chunked nil))
    ((mime:headers-get hdrs "Content-Length")
     (let ((n (parse-integer (string-trim (mime:headers-get hdrs "Content-Length")))))
       (if (or (null n) (< n 0))
           (error (concat "HTTP: malformed Content-Length: " (prin1-to-string (mime:headers-get hdrs "Content-Length")))
                  (list (cons ':category ':bad-content-length)))
           (cons ':content-length n))))
    (t (cons ':none nil))))

(defun $http-build-request-stream (tcp-port framing)
  (let ((kind (car framing)))
    (cond
      ((eq kind ':none) ($http-make-stream ':none tcp-port 0 nil nil))
      ((eq kind ':content-length) ($http-make-stream ':content-length tcp-port (cdr framing) nil nil))
      ((eq kind ':chunked) ($http-make-stream ':chunked tcp-port 0 nil nil))
      (t (error "HTTP: internal error: unexpected request framing kind" (list (cons ':category ':internal)))))))

(defun $http-client-wants-close-p (version hdrs)
  (let ((conn (mime:headers-get hdrs "Connection")))
    (cond
      (conn (string-ci= (string-trim conn) "close"))
      ((string-ci= version "HTTP/1.0") t)
      (t nil))))

(defun $http-drain! (s budget)
  (if (stream-eof-p s)
      nil
      (let* ((got (stream-read! s 8192)) (n (array-length* got)))
        (cond
          ((= n 0) nil)
          ((> n budget)
           (error "HTTP: request body exceeded the drain limit" (list (cons ':category ':body-too-large))))
          (t ($http-drain! s (- budget n)))))))

(defun $http-no-body-status-p (status) (or (= status 204) (= status 304) (and (>= status 100) (< status 200))))

(defun $http-write-port-body-raw! (dest src)
  (let* ((chunk (ports:read-bytes! src 8192)) (n (array-length* chunk)))
    (if (= n 0) nil (progn (ports:write-bytes! dest chunk) ($http-write-port-body-raw! dest src)))))

(defun $http-write-response! (port resp close-after-p)
  (let* ((status (response-status resp))
         (reason0 (response-reason resp))
         (reason (if reason0 reason0 (default-reason status)))
         (hdrs0 (response-headers resp))
         (body (response-body resp))
         (hdrs1 (mime:headers-set hdrs0 "Connection" (if close-after-p "close" "keep-alive")))
         (status-line (concat "HTTP/1.1 " (princ-to-string status) " " reason "\r\n")))
    (cond
      (($http-no-body-status-p status)
       (ports:write-string! port status-line)
       ($http-write-headers! port (mime:headers-remove (mime:headers-remove hdrs1 "Content-Length") "Transfer-Encoding"))
       (ports:write-string! port "\r\n")
       (ports:flush! port))
      ((null body)
       (ports:write-string! port status-line)
       ($http-write-headers! port (mime:headers-set hdrs1 "Content-Length" "0"))
       (ports:write-string! port "\r\n")
       (ports:flush! port))
      ((ports:port-p body)
       (let* ((explicit-cl (mime:headers-get hdrs1 "Content-Length"))
              (hdrs2 (if explicit-cl hdrs1 (mime:headers-set hdrs1 "Transfer-Encoding" "chunked"))))
         (ports:write-string! port status-line)
         ($http-write-headers! port hdrs2)
         (ports:write-string! port "\r\n")
         (if explicit-cl ($http-write-port-body-raw! port body) ($http-write-chunked-body! port body))
         (ports:flush! port)))
      (t
       (let* ((bytes (if (stringp body) (text:string->utf8 body) body))
              (hdrs2 (mime:headers-set hdrs1 "Content-Length" (princ-to-string (array-length* bytes)))))
         (ports:write-string! port status-line)
         ($http-write-headers! port hdrs2)
         (ports:write-string! port "\r\n")
         (ports:write-bytes! port bytes)
         (ports:flush! port))))))

(defun $http-serve-one-request! (tcp-port peer handler max-line-bytes max-header-count max-body-bytes on-error)
  (let ((line ($http-read-line! tcp-port max-line-bytes)))
    (cond
      ((null line) ':closed)
      ((string-empty-p line)
       ($http-serve-one-request! tcp-port peer handler max-line-bytes max-header-count max-body-bytes on-error))
      (t
       (let* ((rl ($http-parse-request-line line))
              (method (car rl)) (target (cadr rl)) (version (caddr rl))
              (hdrs ($http-read-headers! tcp-port max-line-bytes max-header-count))
              (split ($http-split-target target))
              (path (car split)) (query (cdr split))
              (framing ($http-request-body-framing hdrs)))
         (if (and (eq (car framing) ':content-length) (> (cdr framing) max-body-bytes))
             (progn
               ($http-write-response! tcp-port
                                       (respond 413 :headers (list (cons "Content-Type" "text/plain; charset=utf-8"))
                                                :body "Payload Too Large\n")
                                       t)
               ':close-after)
             (let* ((body-stream ($http-build-request-stream tcp-port framing))
                    (req (list (cons ':method method) (cons ':target target) (cons ':path path)
                               (cons ':query query) (cons ':headers hdrs) (cons ':body body-stream)
                               (cons ':version version) (cons ':peer-addr peer)))
                    (close-wanted ($http-client-wants-close-p version hdrs))
                    (outcome
                     (handler-case (cons ':ok (funcall handler req))
                       (error (e)
                         (if on-error
                             (funcall on-error e)
                             (ports:write-string! (ports:stderr)
                                                   (concat "http:serve: handler error: " (error-message e) "\n")))
                         (cons ':failed
                               (respond 500 :headers (list (cons "Content-Type" "text/plain; charset=utf-8"))
                                        :body "Internal Server Error\n")))))
                    (resp (cdr outcome))
                    (handler-failed (eq (car outcome) ':failed)))
               ($http-drain! body-stream max-body-bytes)
               ($http-write-response! tcp-port resp (or close-wanted handler-failed))
               (if (or close-wanted handler-failed) ':close-after ':continue))))))))

(defun $http-serve-requests! (tcp-port peer handler read-timeout-ms max-line-bytes max-header-count
                               max-body-bytes on-error)
  (let ((outcome
         (handler-case
             ($http-serve-one-request! tcp-port peer handler max-line-bytes max-header-count max-body-bytes on-error)
           (error (e) (cons ':fatal e)))))
    (cond
      ((and (consp outcome) (eq (car outcome) ':fatal))
       (if on-error
           (funcall on-error (cdr outcome))
           (ports:write-string! (ports:stderr) (concat "http:serve: " (error-message (cdr outcome)) "\n")))
       nil)
      ((eq outcome ':continue)
       ($http-serve-requests! tcp-port peer handler read-timeout-ms max-line-bytes max-header-count
                               max-body-bytes on-error))
      (t nil))))

(defun serve-one! (listener handler &key (read-timeout-ms $http-default-idle-timeout-ms)
                    (max-line-bytes $http-default-max-line-bytes) (max-header-count $http-default-max-header-count)
                    (max-body-bytes $http-default-max-body-bytes) (on-error nil))
  "Accept exactly one inbound TCP connection from LISTENER (a TCP:LISTEN
listener) and serve every request on it (HANDLER: request alist ->
response alist, see RESPOND) until the client closes, requests
`Connection: close`, or a fatal protocol error forces the connection
closed -- then return. See the file header for the synchronous,
one-connection-at-a-time execution model. :READ-TIMEOUT-MS (default 30s)
bounds every read on this connection, so an idle client eventually gets
disconnected rather than pinning this call forever."
  (let* ((accepted (tcp:accept listener)) (tcp-port (car accepted)) (peer (cdr accepted)))
    (tcp:set-read-timeout! tcp-port read-timeout-ms)
    (unwind-protect
        ($http-serve-requests! tcp-port peer handler read-timeout-ms max-line-bytes max-header-count
                                max-body-bytes on-error)
      (ports:close! tcp-port))))

(defun $http-serve-loop! (listener handler read-timeout-ms max-line-bytes max-header-count max-body-bytes
                           on-error max-requests stop-p served)
  (if (or (and max-requests (>= served max-requests)) (and stop-p (funcall stop-p)))
      served
      (progn
        (serve-one! listener handler :read-timeout-ms read-timeout-ms :max-line-bytes max-line-bytes
                    :max-header-count max-header-count :max-body-bytes max-body-bytes :on-error on-error)
        ($http-serve-loop! listener handler read-timeout-ms max-line-bytes max-header-count max-body-bytes
                            on-error max-requests stop-p (+ served 1)))))

(defun serve (listener handler &key (read-timeout-ms $http-default-idle-timeout-ms)
              (max-line-bytes $http-default-max-line-bytes) (max-header-count $http-default-max-header-count)
              (max-body-bytes $http-default-max-body-bytes) (on-error nil) (max-requests nil) (stop-p nil))
  "Serve inbound connections on LISTENER forever, calling HANDLER once per
HTTP request. One connection is served fully (every keep-alive request on
it, in order) before the next is accepted -- concurrent serving via
share-nothing workers is issue #140's business, explicitly out of scope
here (\"a minimal implementation may serve serially for correctness\" is
the ticket's own text). GRACEFUL SHUTDOWN: :STOP-P, a zero-argument
predicate, is consulted BETWEEN connections (never during one, and cannot
interrupt an in-progress ACCEPT -- see the file header). :MAX-REQUESTS
stops after serving that many connections; primarily for deterministic
tests. :ON-ERROR, if given, is called with the structured error condition
for any handler exception or fatal protocol error (for host-side
diagnostics) -- uncaught handler errors always become a generic 500 to the
peer, never leaking the condition's message or data across the wire."
  ($http-serve-loop! listener handler read-timeout-ms max-line-bytes max-header-count max-body-bytes on-error
                      max-requests stop-p 0))

)

(provide 'http
  '(http:request http:get http:post
    http:response-status http:response-reason http:response-version http:response-headers
    http:response-header http:response-body
    http:request-method http:request-target http:request-path http:request-query
    http:request-headers http:request-header http:request-body http:request-version
    http:request-peer-addr http:request-url
    http:stream-read! http:stream-read-all! http:stream-eof-p http:stream-close!
    http:collect-bytes http:collect-string http:collect-json
    http:serve http:serve-one! http:respond http:default-reason))
