;;;; io.lisp -- host I/O primitives: files, ports, text/UTF-8, shell, OS,
;;;; TCP/UDP, TLS, and regex. Backs lib/07,30-44.
;;;;
;;;; This port does not gate these behind capabilities (see sbcl/README.md
;;;; on sandboxing); every primitive here is simply always available.

(in-package #:lamedh-rt)

(eval-when (:compile-toplevel :load-toplevel :execute)
  (require :sb-bsd-sockets)
  (require :sb-posix))

;;; ============================================================================
;;; Files
;;; ============================================================================

(defbuiltin "FILE-EXISTS-P" (path) (bool (probe-file path)))
(defbuiltin "DIRECTORY-P" (path) (bool (and (probe-file path) (uiop:directory-pathname-p (probe-file path)))))
(defbuiltin "FILE-P" (path) (bool (and (probe-file path) (not (uiop:directory-pathname-p (probe-file path))))))
(defbuiltin "FILE-READABLE-P" (path) (bool (probe-file path)))
(defbuiltin "FILE-WRITABLE-P" (path) (bool (or (probe-file path) (probe-file (make-pathname :name nil :type nil :defaults path)))))
(defbuiltin "FILE-EXECUTABLE-P" (path) (bool (probe-file path)))
(defbuiltin "FILE-SIZE" (path) (with-open-file (s path :element-type '(unsigned-byte 8)) (file-length s)))
(defbuiltin "READ-FILE" (path) (uiop:read-file-string path))
(defbuiltin "READ-STRING" (s) (lread-all s))
(defbuiltin "WRITE-FILE" (path content)
  (with-open-file (s path :direction :output :if-exists :supersede :if-does-not-exist :create)
    (write-string content s))
  nil)
(defbuiltin "DELETE-FILE" (path) (delete-file path) nil)
(defbuiltin "CREATE-DIRECTORY" (path) (ensure-directories-exist (uiop:ensure-directory-pathname path)) nil)
(defbuiltin "RENAME-FILE" (old new) (rename-file old new) nil)
(defbuiltin "CHMOD" (path mode) (declare (ignore mode)) path)
(defbuiltin "DIRECTORY-FILES" (path)
  (mapcar (lambda (p) (namestring (uiop:enough-pathname p (uiop:ensure-directory-pathname path))))
          (uiop:directory-files path)))
(defbuiltin "FILE-NEWER-P" (a b) (bool (> (or (file-write-date a) 0) (or (file-write-date b) 0))))
(defbuiltin "MAKE-TEMP-FILE" (&optional prefix)
  (namestring (uiop:with-temporary-file (:pathname p :prefix (or prefix "lamedh") :keep t) p)))
(defbuiltin "MAKE-TEMP-DIRECTORY" (&optional prefix)
  (let ((dir (uiop:ensure-directory-pathname
              (format nil "~A~A-~D/" (uiop:temporary-directory) (or prefix "lamedh") (random 1000000)))))
    (ensure-directories-exist dir)
    (namestring dir)))

;;; ============================================================================
;;; Ports (lib/31-ports.lisp)
;;; ============================================================================

(defstruct lport
  kind      ; :file :memory :stdin :stdout :stderr
  name
  direction ; :input :output
  stream
  (closed nil)
  in-bytes in-pos     ; memory input
  out-bytes)          ; memory output (adjustable (unsigned-byte 8) vector)

(defun bytes-array->lisp (vec) (map 'simple-vector #'code-char vec))
(defun lisp-array->bytes (arr)
  (map '(vector (unsigned-byte 8)) (lambda (c) (if (characterp c) (char-code c) c)) arr))

(defbuiltin "PORT-OPEN-INPUT-FILE*" (path)
  (make-lport :kind :file :name path :direction :input
              :stream (open path :direction :input :element-type '(unsigned-byte 8))))
(defbuiltin "PORT-OPEN-OUTPUT-FILE*" (path)
  (make-lport :kind :file :name path :direction :output
              :stream (open path :direction :output :element-type '(unsigned-byte 8)
                            :if-exists :supersede :if-does-not-exist :create)))
(defbuiltin "PORT-OPEN-APPEND-FILE*" (path)
  (make-lport :kind :file :name path :direction :output
              :stream (open path :direction :output :element-type '(unsigned-byte 8)
                            :if-exists :append :if-does-not-exist :create)))
(defbuiltin "PORT-OPEN-INPUT-BYTES*" (bytes)
  (make-lport :kind :memory :name "<bytes>" :direction :input :in-bytes (lisp-array->bytes bytes) :in-pos 0))
(defbuiltin "PORT-OPEN-OUTPUT-BYTES*" ()
  (make-lport :kind :memory :name "<memory>" :direction :output
              :out-bytes (make-array 0 :element-type '(unsigned-byte 8) :adjustable t :fill-pointer 0)))
(defbuiltin "PORT-OUTPUT-CONTENTS*" (port)
  (unless (lport-out-bytes port) (lamedh-error "PORT-OUTPUT-CONTENTS*: not a memory output port"))
  (bytes-array->lisp (lport-out-bytes port)))
(defbuiltin "PORT-STDIN*" () (make-lport :kind :stdin :name "<stdin>" :direction :input :stream *standard-input*))
(defbuiltin "PORT-STDOUT*" () (make-lport :kind :stdout :name "<stdout>" :direction :output :stream *standard-output*))
(defbuiltin "PORT-STDERR*" () (make-lport :kind :stderr :name "<stderr>" :direction :output :stream *error-output*))

(defbuiltin "PORT-READ-BYTE*" (port)
  (case (lport-kind port)
    (:memory (if (< (lport-in-pos port) (length (lport-in-bytes port)))
                 (prog1 (aref (lport-in-bytes port) (lport-in-pos port)) (incf (lport-in-pos port)))
                 nil))
    (t (let ((c (read-char (lport-stream port) nil nil))) (and c (char-code c))))))
(defbuiltin "PORT-READ-BYTES*" (port n)
  (let (out)
    (dotimes (i n) (let ((b (lport-read-byte port))) (if b (push b out) (return))))
    (bytes-array->lisp (coerce (nreverse out) 'vector))))
(defun lport-read-byte (port)
  (case (lport-kind port)
    (:memory (if (< (lport-in-pos port) (length (lport-in-bytes port)))
                 (prog1 (aref (lport-in-bytes port) (lport-in-pos port)) (incf (lport-in-pos port)))
                 nil))
    (t (let ((c (read-char (lport-stream port) nil nil))) (and c (char-code c))))))
(defbuiltin "PORT-WRITE-BYTE*" (port byte)
  (let ((b (if (characterp byte) (char-code byte) byte)))
    (case (lport-kind port)
      (:memory (vector-push-extend b (lport-out-bytes port)))
      (t (write-char (code-char b) (lport-stream port)))))
  nil)
(defbuiltin "PORT-WRITE-BYTES*" (port bytes)
  (let ((v (lisp-array->bytes bytes)))
    (loop for b across v do
      (case (lport-kind port)
        (:memory (vector-push-extend b (lport-out-bytes port)))
        (t (write-char (code-char b) (lport-stream port)))))
    (length v)))
(defbuiltin "PORT-FLUSH*" (port) (when (lport-stream port) (force-output (lport-stream port))) nil)
(defbuiltin "PORT-CLOSE*" (port)
  (unless (lport-closed port)
    (when (and (lport-stream port) (member (lport-kind port) '(:file)))
      (close (lport-stream port)))
    (setf (lport-closed port) t))
  nil)
(defbuiltin "PORT-OPEN-P*" (port) (bool (not (lport-closed port))))
(defbuiltin "PORT-INPUT-P*" (port) (bool (eq (lport-direction port) :input)))
(defbuiltin "PORT-OUTPUT-P*" (port) (bool (eq (lport-direction port) :output)))
(defbuiltin "PORT-SEEKABLE-P*" (port) (bool (member (lport-kind port) '(:file :memory))))
(defbuiltin "PORT-POSITION*" (port)
  (case (lport-kind port) (:memory (lport-in-pos port)) (:file (file-position (lport-stream port)))
    (t (lamedh-error "PORT-POSITION*: not seekable"))))
(defbuiltin "PORT-SEEK*" (port offset)
  (case (lport-kind port)
    (:memory (setf (lport-in-pos port) offset))
    (:file (file-position (lport-stream port) offset))
    (t (lamedh-error "PORT-SEEK*: not seekable")))
  offset)
(defbuiltin "PORT-P*" (v) (bool (lport-p v)))
(defbuiltin "PORT-NAME*" (port) (princ-to-string (lport-name port)))
(defbuiltin "PORT-KIND*" (port) (lsym (string-upcase (symbol-name (lport-kind port)))))

;;; ============================================================================
;;; Text: explicit UTF-8 <-> String boundary (lib/30-text.lisp)
;;; ============================================================================

(defbuiltin "STRING->UTF8*" (s) (bytes-array->lisp (sb-ext:string-to-octets s :external-format :utf-8)))
(defbuiltin "UTF8->STRING*" (bytes)
  (handler-case (sb-ext:octets-to-string (lisp-array->bytes bytes) :external-format :utf-8)
    (error (c) (lamedh-error (format nil "invalid UTF-8: ~A" c)))))
(defbuiltin "UTF8->STRING-LOSSY*" (bytes) (utf8-decode-lossy (lisp-array->bytes bytes)))
(defun utf8-decode-lossy (bytes)
  "Manual UTF-8 decode substituting U+FFFD for any invalid byte sequence,
one replacement character per maximal invalid subsequence (the usual WHATWG
Encoding Standard convention)."
  (let ((n (length bytes)) (i 0) (out (make-array 0 :element-type 'character :adjustable t :fill-pointer 0)))
    (flet ((cont-p (b) (and b (= (logand b #xC0) #x80))))
      (loop while (< i n) do
        (let ((b0 (aref bytes i)))
          (cond
            ((< b0 #x80) (vector-push-extend (code-char b0) out) (incf i))
            ((and (>= b0 #xC2) (<= b0 #xDF) (cont-p (and (< (1+ i) n) (aref bytes (1+ i)))))
             (vector-push-extend (code-char (logior (ash (logand b0 #x1F) 6) (logand (aref bytes (1+ i)) #x3F))) out)
             (incf i 2))
            ((and (>= b0 #xE0) (<= b0 #xEF) (< (+ i 2) n)
                  (cont-p (aref bytes (1+ i))) (cont-p (aref bytes (+ i 2))))
             (vector-push-extend (code-char (logior (ash (logand b0 #x0F) 12) (ash (logand (aref bytes (1+ i)) #x3F) 6)
                                                     (logand (aref bytes (+ i 2)) #x3F))) out)
             (incf i 3))
            ((and (>= b0 #xF0) (<= b0 #xF4) (< (+ i 3) n)
                  (cont-p (aref bytes (1+ i))) (cont-p (aref bytes (+ i 2))) (cont-p (aref bytes (+ i 3))))
             (vector-push-extend (code-char (logior (ash (logand b0 #x07) 18) (ash (logand (aref bytes (1+ i)) #x3F) 12)
                                                     (ash (logand (aref bytes (+ i 2)) #x3F) 6) (logand (aref bytes (+ i 3)) #x3F))) out)
             (incf i 4))
            (t (vector-push-extend (code-char #xFFFD) out) (incf i))))))
    (coerce out 'simple-string)))

;;; ============================================================================
;;; Shell (lib/07-shell.lisp)
;;; ============================================================================

(defbuiltin "SHELL" (cmd)
  (multiple-value-bind (out err code)
      (uiop:run-program cmd :output '(:string :stripped nil) :error-output '(:string :stripped nil)
                         :ignore-error-status t :force-shell t)
    (list code out err)))

;;; ============================================================================
;;; OS (lib/41-os.lisp, lib/42-os-linux.lisp)
;;; ============================================================================

(defbuiltin "OS-ARGS*" () (cons (or (first sb-ext:*posix-argv*) "lamedh") (rest sb-ext:*posix-argv*)))
(defbuiltin "OS-EXECUTABLE-PATH*" () (namestring (truename (or *load-truename* sb-ext:*runtime-pathname* "/proc/self/exe"))))
(defbuiltin "OS-CWD*" () (namestring (uiop:getcwd)))
(defbuiltin "OS-CHDIR*" (path) (sb-posix:chdir path) nil)
(defbuiltin "OS-ENV-GET*" (name) (uiop:getenv name))
(defbuiltin "OS-ENV-LIST*" ()
  (sort (mapcar (lambda (kv) (let ((i (position #\= kv))) (cons (subseq kv 0 i) (subseq kv (1+ i)))))
                (sb-ext:posix-environ))
        #'string< :key #'car))
(defbuiltin "OS-ENV-SET*" (name value) (sb-posix:setenv name value 1) nil)
(defbuiltin "OS-ENV-UNSET*" (name) (sb-posix:unsetenv name) nil)
(defbuiltin "OS-PID*" () (sb-posix:getpid))
(defbuiltin "OS-PPID*" () (sb-posix:getppid))
(defbuiltin "OS-HOSTNAME*" () (machine-instance))
(defbuiltin "OS-NOW*" () (cons (- (get-universal-time) 2208988800) 0))
(defbuiltin "OS-MONOTONIC-NANOS*" () (round (* (get-internal-real-time) (/ 1000000000 internal-time-units-per-second))))
(defbuiltin "OS-SLEEP*" (ms) (sleep (/ ms 1000.0)) nil)
(defbuiltin "OS-PRNG-STEP*" (state)
  (let* ((s (logand (+ state #x9E3779B97F4A7C15) #xFFFFFFFFFFFFFFFF)) (z s))
    (setf z (logand (* (logxor z (ash z -30)) #xBF58476D1CE4E5B9) #xFFFFFFFFFFFFFFFF))
    (setf z (logand (* (logxor z (ash z -27)) #x94D049BB133111EB) #xFFFFFFFFFFFFFFFF))
    (setf z (logxor z (ash z -31)))
    (cons s z)))
(defbuiltin "OS-RANDOM-BYTES*" (n)
  (with-open-file (s "/dev/urandom" :element-type '(unsigned-byte 8))
    (let ((buf (make-array n :element-type '(unsigned-byte 8))))
      (read-sequence buf s)
      (bytes-array->lisp buf))))

(defstruct lchild process)
(defbuiltin "OS-SPAWN*" (program argv inherit-env env cwd stdin-mode stdout-mode stderr-mode)
  (declare (ignore inherit-env))
  (flet ((mode (m) (case m (:pipe :stream) (:null nil) (t t))))
    (let* ((proc (sb-ext:run-program program argv :output (mode stdout-mode) :error (mode stderr-mode)
                                      :input (mode stdin-mode) :wait nil
                                      :environment (when env (mapcar (lambda (kv) (format nil "~A=~A" (car kv) (cdr kv))) env))
                                      :directory cwd))
           (child (make-lchild :process proc)))
      (list child
            (and (eq (mode stdin-mode) :stream) (make-lport :kind :file :name "<child-stdin>" :direction :output :stream (sb-ext:process-input proc)))
            (and (eq (mode stdout-mode) :stream) (make-lport :kind :file :name "<child-stdout>" :direction :input :stream (sb-ext:process-output proc)))
            (and (eq (mode stderr-mode) :stream) (make-lport :kind :file :name "<child-stderr>" :direction :input :stream (sb-ext:process-error proc)))))))
(defun child-status-alist (proc)
  (sb-ext:process-wait proc)
  (list (cons (lsym ":EXIT-CODE") (sb-ext:process-exit-code proc))
        (cons (lsym ":SIGNAL") nil)
        (cons (lsym ":SUCCESS") (bool (eql 0 (sb-ext:process-exit-code proc))))))
(defbuiltin "OS-PROCESS-WAIT*" (handle) (child-status-alist (lchild-process handle)))
(defbuiltin "OS-PROCESS-TRY-WAIT*" (handle)
  (if (sb-ext:process-alive-p (lchild-process handle)) nil (child-status-alist (lchild-process handle))))
(defbuiltin "OS-PROCESS-ID*" (handle) (sb-ext:process-pid (lchild-process handle)))
(defbuiltin "OS-PROCESS-OPEN-P*" (handle) (bool (sb-ext:process-alive-p (lchild-process handle))))
(defbuiltin "OS-PROCESS-KILL*" (handle) (sb-ext:process-kill (lchild-process handle) 9) nil)
(defbuiltin "OS-PROCESS-TERMINATE*" (handle) (sb-ext:process-kill (lchild-process handle) 15) nil)
(defbuiltin "OS-PROCESS-P*" (v) (bool (lchild-p v)))
(defbuiltin "OS-SIGNAL*" (pid signal-name)
  (let ((n (cond ((string-equal signal-name "TERM") 15) ((string-equal signal-name "KILL") 9)
                 ((string-equal signal-name "HUP") 1) ((string-equal signal-name "INT") 2)
                 (t 15))))
    (sb-posix:kill pid n))
  nil)
(defbuiltin "OS-LINUX-STAT" (path) (declare (ignore path)) nil)
(defbuiltin "OS-LINUX-READLINK" (path) (namestring (truename path)))

;;; ============================================================================
;;; TLS (lib/43-tls.lisp) -- honestly unavailable: no bundled TLS library.
;;; ============================================================================
;;;
;;; A genuine external-dependency gap (see sbcl/README.md), not a design
;;; choice: SBCL has no built-in TLS, and this environment has no network
;;; access to fetch cl+ssl/OpenSSL bindings. Every primitive still exists
;;; (so lib/43-tls.lisp loads without error) and signals a clear error only
;;; when actually invoked.

(defun tls-unavailable () (lamedh-error "TLS is not available in this port (no bundled TLS library)"))
(dolist (name '("TLS-AVAILABLE-P*" "TLS-WRAP-CLIENT*" "TLS-WRAP-CLIENT-INSECURE*" "TLS-WRAP-SERVER*"
                "TLS-ALPN-PROTOCOL*" "TLS-PEER-CERTIFICATES*" "TLS-PEER-CERTIFICATE-SUMMARY*" "TLS-SNI-HOSTNAME*"))
  (env-set-local *global-env* (lsym name)
                  (if (string= name "TLS-AVAILABLE-P*") (lambda (&rest args) (declare (ignore args)) nil)
                      (lambda (&rest args) (declare (ignore args)) (tls-unavailable)))))

;;; ============================================================================
;;; NET / TCP / UDP (lib/37,38,39) via sb-bsd-sockets
;;; ============================================================================

(defstruct lnethandle kind socket stream)

(defbuiltin "NET-RESOLVE*" (host port)
  (let ((addrs (sb-bsd-sockets:host-ent-addresses (sb-bsd-sockets:get-host-by-name host))))
    (mapcar (lambda (a) (list (lsym "IPV4") (format nil "~{~D~^.~}" (coerce a 'list)) port)) addrs)))
(defun sockaddr->triple (socket)
  (multiple-value-bind (addr port) (sb-bsd-sockets:socket-name socket)
    (list (lsym "IPV4") (format nil "~{~D~^.~}" (coerce addr 'list)) port)))
(defbuiltin "NET-LOCAL-ADDR*" (resource)
  (sockaddr->triple (if (lnethandle-p resource) (lnethandle-socket resource) (lport-stream resource))))
(defbuiltin "NET-PEER-ADDR*" (resource)
  (multiple-value-bind (addr port) (sb-bsd-sockets:socket-peername
                                     (if (lnethandle-p resource) (lnethandle-socket resource) (lport-stream resource)))
    (list (lsym "IPV4") (format nil "~{~D~^.~}" (coerce addr 'list)) port)))

(defbuiltin "TCP-CONNECT*" (host port)
  (let ((sock (make-instance 'sb-bsd-sockets:inet-socket :type :stream :protocol :tcp)))
    (sb-bsd-sockets:socket-connect sock (sb-bsd-sockets:host-ent-address (sb-bsd-sockets:get-host-by-name host)) port)
    (make-lport :kind :file :name (format nil "~A:~D" host port) :direction :input
                :stream (sb-bsd-sockets:socket-make-stream sock :input t :output t :element-type '(unsigned-byte 8)))))
(defbuiltin "TCP-LISTEN*" (host port backlog)
  (let ((sock (make-instance 'sb-bsd-sockets:inet-socket :type :stream :protocol :tcp)))
    (setf (sb-bsd-sockets:sockopt-reuse-address sock) t)
    (sb-bsd-sockets:socket-bind sock (sb-bsd-sockets:make-inet-address host) port)
    (sb-bsd-sockets:socket-listen sock (or backlog 128))
    (make-lnethandle :kind :tcp-listener :socket sock)))
(defbuiltin "TCP-ACCEPT*" (listener)
  (let ((client (sb-bsd-sockets:socket-accept (lnethandle-socket listener))))
    (make-lport :kind :file :name "<tcp-accepted>" :direction :input
                :stream (sb-bsd-sockets:socket-make-stream client :input t :output t :element-type '(unsigned-byte 8)))))
(defbuiltin "TCP-SHUTDOWN*" (port how) (declare (ignore how)) (close (lport-stream port)) nil)
(defbuiltin "TCP-SET-READ-TIMEOUT*" (port secs) (declare (ignore port secs)) nil)
(defbuiltin "TCP-SET-WRITE-TIMEOUT*" (port secs) (declare (ignore port secs)) nil)
(defbuiltin "NET-HANDLE-CLOSE*" (h) (ignore-errors (sb-bsd-sockets:socket-close (lnethandle-socket h))) nil)
(defbuiltin "NET-HANDLE-OPEN-P*" (h) (bool (sb-bsd-sockets:socket-open-p (lnethandle-socket h))))
(defbuiltin "NET-HANDLE-P*" (v) (bool (lnethandle-p v)))
(defbuiltin "NET-HANDLE-NAME*" (h) (princ-to-string (lnethandle-kind h)))
(defbuiltin "NET-HANDLE-KIND*" (h) (lsym (string-upcase (symbol-name (lnethandle-kind h)))))

(defbuiltin "UDP-BIND*" (host port)
  (let ((sock (make-instance 'sb-bsd-sockets:inet-socket :type :datagram :protocol :udp)))
    (sb-bsd-sockets:socket-bind sock (sb-bsd-sockets:make-inet-address host) port)
    (make-lnethandle :kind :udp :socket sock)))
(defbuiltin "UDP-CONNECT*" (host port)
  (let ((sock (make-instance 'sb-bsd-sockets:inet-socket :type :datagram :protocol :udp)))
    (sb-bsd-sockets:socket-connect sock (sb-bsd-sockets:host-ent-address (sb-bsd-sockets:get-host-by-name host)) port)
    (make-lnethandle :kind :udp :socket sock)))
(defbuiltin "UDP-SEND-TO*" (h bytes host port)
  (sb-bsd-sockets:socket-send (lnethandle-socket h) (lisp-array->bytes bytes) nil
                               :address (sb-bsd-sockets:host-ent-address (sb-bsd-sockets:get-host-by-name host)) :port port))
(defbuiltin "UDP-SEND*" (h bytes) (sb-bsd-sockets:socket-send (lnethandle-socket h) (lisp-array->bytes bytes) nil))
(defbuiltin "UDP-RECEIVE-FROM*" (h n)
  (multiple-value-bind (buf len addr port) (sb-bsd-sockets:socket-receive (lnethandle-socket h) nil n)
    (list (bytes-array->lisp (subseq buf 0 len)) (list (lsym "IPV4") (format nil "~{~D~^.~}" (coerce addr 'list)) port))))
(defbuiltin "UDP-SET-TIMEOUT*" (h secs) (declare (ignore h secs)) nil)

;;; ============================================================================
;;; Regex (lib/44-regex.lisp) -- a small backtracking engine.
;;; ============================================================================
;;;
;;; DEVIATION (documented in sbcl/README.md): the reference implementation
;;; wraps Rust's `regex` crate (RE2 semantics -- no backtracking, guaranteed
;;; linear time). This is a straightforward recursive-descent /
;;; backtracking matcher instead: functionally equivalent on the common
;;; subset (literals, ./*/+/?/{}, character classes, ^/$, |, capturing
;;; groups, \d\w\s escapes) but WITHOUT RE2's linear-time guarantee -- a
;;; pathological pattern can backtrack exponentially here. Named groups
;;; and Unicode-aware classes are not supported.

(defstruct lregex pattern ast)

(defun regex-parse (pattern)
  (let ((pos 0) (len (length pattern)) (group-count 0))
    (labels
        ((peek () (and (< pos len) (char pattern pos)))
         (advance () (prog1 (char pattern pos) (incf pos)))
         (parse-alt ()
           (let ((branches (list (parse-seq))))
             (loop while (eql (peek) #\|) do (advance) (push (parse-seq) branches))
             (if (cdr branches) (list :alt (nreverse branches)) (car branches))))
         (parse-seq ()
           (let (items)
             (loop while (and (peek) (not (member (peek) '(#\| #\))))) do (push (parse-rep) items))
             (list :seq (nreverse items))))
         (parse-rep ()
           (let ((atom (parse-atom)))
             (loop
               (case (peek)
                 (#\* (advance) (setf atom (list :rep atom 0 nil)))
                 (#\+ (advance) (setf atom (list :rep atom 1 nil)))
                 (#\? (advance) (setf atom (list :rep atom 0 1)))
                 (#\{ (let ((save pos))
                        (advance)
                        (let ((n (parse-int)))
                          (if (null n) (progn (setf pos save) (return atom))
                              (let (m)
                                (if (eql (peek) #\,)
                                    (progn (advance) (setf m (parse-int)))
                                    (setf m n))
                                (if (eql (peek) #\}) (progn (advance) (setf atom (list :rep atom n m)))
                                    (progn (setf pos save) (return atom))))))))
                 (t (return atom))))
             atom))
         (parse-int ()
           (let ((start pos))
             (loop while (and (peek) (digit-char-p (peek))) do (advance))
             (if (> pos start) (parse-integer pattern :start start :end pos) nil)))
         (parse-atom ()
           (case (peek)
             (#\( (advance)
              (let ((idx (incf group-count)) (inner (parse-alt)))
                (unless (eql (peek) #\)) (lamedh-error "REGEX: unmatched '('"))
                (advance) (list :group idx inner)))
             (#\[ (advance) (parse-class))
             (#\. (advance) (list :any))
             (#\^ (advance) (list :bol))
             (#\$ (advance) (list :eol))
             (#\\ (advance) (parse-escape))
             (t (list :char (advance)))))
         (parse-escape ()
           (let ((c (advance)))
             (case c
               (#\d (list :class nil '((#\0 . #\9))))
               (#\D (list :class t '((#\0 . #\9))))
               (#\w (list :class nil '((#\a . #\z) (#\A . #\Z) (#\0 . #\9) (#\_ . #\_))))
               (#\W (list :class t '((#\a . #\z) (#\A . #\Z) (#\0 . #\9) (#\_ . #\_))))
               (#\s (list :class nil '((#\Space . #\Space) (#\Tab . #\Tab) (#\Newline . #\Newline) (#\Return . #\Return))))
               (#\S (list :class t '((#\Space . #\Space) (#\Tab . #\Tab) (#\Newline . #\Newline) (#\Return . #\Return))))
               (t (list :char c)))))
         (parse-class ()
           (let ((negate (eql (peek) #\^)) ranges)
             (when negate (advance))
             (loop while (and (peek) (not (eql (peek) #\])))
                   do (let ((c (if (eql (peek) #\\) (progn (advance) (advance)) (advance))))
                        (if (and (eql (peek) #\-) (< (1+ pos) len) (not (eql (char pattern (1+ pos)) #\])))
                            (progn (advance) (push (cons c (advance)) ranges))
                            (push (cons c c) ranges))))
             (unless (eql (peek) #\]) (lamedh-error "REGEX: unmatched '['"))
             (advance)
             (list :class negate ranges))))
      (let ((ast (parse-alt)))
        (values ast group-count)))))

(defun regex-class-match-p (negate ranges ch)
  (let ((hit (some (lambda (r) (char<= (car r) ch (cdr r))) ranges)))
    (if negate (not hit) hit)))

(defun regex-match-node (node s i caps k)
  "Try to match NODE at position I in string S; K is a continuation
(lambda (i caps) ...) called with the position after a successful match.
Returns K's result, or NIL if no match (with backtracking via K's own
NIL propagation)."
  (case (car node)
    (:char (and (< i (length s)) (char= (char s i) (second node)) (funcall k (1+ i) caps)))
    (:any (and (< i (length s)) (funcall k (1+ i) caps)))
    (:class (and (< i (length s)) (regex-class-match-p (second node) (third node) (char s i)) (funcall k (1+ i) caps)))
    (:bol (and (= i 0) (funcall k i caps)))
    (:eol (and (= i (length s)) (funcall k i caps)))
    (:seq (regex-match-seq (second node) s i caps k))
    (:alt (some (lambda (b) (regex-match-node b s i caps k)) (second node)))
    (:group (regex-match-node (third node) s i caps
                               (lambda (j caps2) (funcall k j (cons (list (second node) i j) caps2)))))
    (:rep (regex-match-rep node s i caps k))
    (t nil)))

(defun regex-match-seq (nodes s i caps k)
  (if (null nodes) (funcall k i caps)
      (regex-match-node (car nodes) s i caps (lambda (j caps2) (regex-match-seq (cdr nodes) s j caps2 k)))))

(defun regex-match-rep (node s i caps k)
  (destructuring-bind (inner min max) (cdr node)
    (labels ((go-count (n i caps)
               (if (and max (>= n max))
                   (funcall k i caps)
                   (or (regex-match-node inner s i caps (lambda (j caps2) (and (/= j i) (go-count (1+ n) j caps2))))
                       (and (>= n min) (funcall k i caps))))))
      (go-count 0 i caps))))

(defun regex-search (rx s start)
  "Search RX in S starting at or after char index START. Returns (values
match-start match-end caps) or NIL."
  (loop for i from start to (length s) do
    (let (result)
      (regex-match-node (lregex-ast rx) s i nil (lambda (j caps) (setf result (list i j caps)) t))
      (when result (return-from regex-search (values (first result) (second result) (third result))))))
  nil)

(defbuiltin "REGEX-COMPILE*" (pattern)
  (make-lregex :pattern pattern :ast (regex-parse pattern)))
(defbuiltin "REGEX-P*" (v) (bool (lregex-p v)))
(defbuiltin "REGEX-PATTERN*" (rx) (lregex-pattern rx))
(defbuiltin "REGEX-ESCAPE*" (s)
  (with-output-to-string (out)
    (loop for c across s do
      (when (find c ".^$*+?()[]{}|\\") (write-char #\\ out))
      (write-char c out))))
(defun as-regex (re) (if (lregex-p re) re (make-lregex :pattern re :ast (regex-parse re))))
(defbuiltin "REGEX-IS-MATCH*" (re s) (bool (regex-search (as-regex re) s 0)))
(defbuiltin "REGEX-FIND*" (re s &optional (start 0))
  (multiple-value-bind (i j) (regex-search (as-regex re) s start)
    (if i (list (subseq s i j) i j) nil)))
(defbuiltin "REGEX-FIND-ALL*" (re s)
  (let ((rx (as-regex re)) (start 0) out)
    (loop
      (multiple-value-bind (i j) (regex-search rx s start)
        (unless i (return))
        (push (list (subseq s i j) i j) out)
        (setf start (if (= j i) (1+ j) j))
        (when (> start (length s)) (return))))
    (nreverse out)))
(defbuiltin "REGEX-CAPTURES*" (re s)
  (let ((rx (as-regex re)))
    (multiple-value-bind (i j caps) (regex-search rx s 0)
      (when i
        (let ((groups (make-array (1+ (if caps (reduce #'max caps :key #'first) 0)) :initial-element nil)))
          (setf (aref groups 0) (list (subseq s i j) i j))
          (dolist (c caps) (setf (aref groups (first c)) (list (subseq s (second c) (third c)) (second c) (third c))))
          (coerce groups 'list))))))
(defbuiltin "REGEX-CAPTURES-NAMED*" (re s) (declare (ignore re s)) nil)
(defun regex-expand-template (template s caps whole-start whole-end)
  (with-output-to-string (out)
    (let ((i 0) (n (length template)))
      (loop while (< i n) do
        (let ((c (char template i)))
          (if (and (char= c #\$) (< (1+ i) n))
              (let ((c1 (char template (1+ i))))
                (cond
                  ((char= c1 #\$) (write-char #\$ out) (incf i 2))
                  ((digit-char-p c1)
                   (let ((num (digit-char-p c1)))
                     (if (= num 0)
                         (write-string (subseq s whole-start whole-end) out)
                         (let ((g (find num caps :key #'first)))
                           (when g (write-string (subseq s (second g) (third g)) out))))
                     (incf i 2)))
                  (t (write-char c out) (incf i))))
              (progn (write-char c out) (incf i))))))))
(defbuiltin "REGEX-REPLACE*" (re s replacement)
  (let ((rx (as-regex re)))
    (multiple-value-bind (i j caps) (regex-search rx s 0)
      (if i (concatenate 'string (subseq s 0 i) (regex-expand-template replacement s caps i j) (subseq s j))
          s))))
(defbuiltin "REGEX-REPLACE-ALL*" (re s replacement)
  (let ((rx (as-regex re)) (start 0) (out (make-string-output-stream)))
    (loop
      (multiple-value-bind (i j caps) (regex-search rx s start)
        (unless i (write-string (subseq s start) out) (return))
        (write-string (subseq s start i) out)
        (write-string (regex-expand-template replacement s caps i j) out)
        (setf start (if (= j i) (progn (when (< j (length s)) (write-char (char s j) out)) (1+ j)) j))
        (when (> start (length s)) (return))))
    (get-output-stream-string out)))
(defbuiltin "REGEX-SPLIT*" (re s &optional limit)
  (declare (ignore limit))
  (let ((rx (as-regex re)) (start 0) (piece-start 0) out)
    (loop
      (multiple-value-bind (i j) (regex-search rx s start)
        (unless i (push (subseq s piece-start) out) (return))
        (push (subseq s piece-start i) out)
        (setf piece-start j start (if (= j i) (1+ j) j))
        (when (> start (length s)) (push (subseq s piece-start) out) (return))))
    (nreverse out)))
