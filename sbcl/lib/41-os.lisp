;;; OS module — portable process environment, time, randomness, and process
;;; spawning/control (issue #260, epic #253).
;;;
;;; WHY A MODULE: like lib/31-ports.lisp and lib/37-net.lisp, this is a
;;; genuinely new facility layered on the kernel, not a completion of an
;;; existing flat name, so per the epic #253 namespace ruling it lives under
;;; a module. Call it qualified (OS:ARGS) or `(import os)` to bind the
;;; unqualified names.
;;;
;;; SCOPE (issue #260): this file is the PORTABLE surface the ticket asks
;;; for -- process identity/environment, time, randomness, and spawning a
;;; child process with typed stdio/wait/kill/terminate. Linux-specific
;;; typed facilities (advanced file metadata, symlink targets) live in the
;;; separate OS-LINUX module (lib/42-os-linux.lisp), per the ticket's
;;; explicit "portable module + Linux-only module" split. Deferred entirely
;;; out of this increment (see the PR description for the full list): a
;;; standalone PIPE primitive not tied to a spawned child, PTY/terminal
;;; settings, eventfd/timerfd/signalfd/epoll, Unix-domain sockets, and
;;; openat-style directory-relative operations -- all explicitly named by
;;; the ticket as later-priority once "the basic resource and polling model
;;; is stable".
;;;
;;; NO RAW SYSCALL NUMBERS, NO BARE HANDLES: a spawned process is an opaque
;;; OS:CHILD handle (compares by identity, closes/reaps deterministically,
;;; has a Drop backstop -- see src/lib.rs's ChildObj); its stdio pipes are
;;; ordinary PORTS ports (issue #255). Signals are sent only by typed name
;;; (:TERM, :KILL, :HUP, ...) via SIGNAL!, never a raw signal number.
;;;
;;; CAPABILITIES (see src/evaluator/builtins_os.rs's module header for the
;;; full model): OS-ENV gates reading identity/environment (ARGS,
;;; EXECUTABLE-PATH, CWD, ENV-GET, ENV-LIST, PID, PPID, HOSTNAME). OS-ENV-WRITE
;;; gates mutating it (CHDIR!, ENV-SET!, ENV-UNSET!). OS-PROCESS gates
;;; spawning (SPAWN); once a child handle is returned, PROCESS-WAIT!/
;;; TRY-WAIT!/KILL!/TERMINATE!/ID/ALIVE-P need no further capability --
;;; using a resource you already hold is "continue" authority (issue #255's
;;; rule, same as PORTS/TCP/UDP). OS-SIGNAL gates SIGNAL! (sending to a PID
;;; you do not hold a handle for). Time/randomness are ungated (pure or
;;; read-only-entropy operations; see the Rust module header for why).
;;;
;;; RELATION TO lib/07-shell.lisp: SHELL/SH run a command through `/bin/sh
;;; -c` and return `(exit-code stdout stderr)` after the whole command
;;; finishes -- a convenient, coarse, capability-gated (SHELL) escape hatch.
;;; OS:SPAWN is the principled typed layer this issue asks for: explicit
;;; argv (no shell interpolation/injection), explicit environment/cwd,
;;; separately configurable stdin/stdout/stderr (:INHERIT/:NULL/:PIPE), and
;;; an owned handle with non-blocking TRY-WAIT!, KILL!, and TERMINATE! --
;;; none of which SHELL exposes. SHELL/SH are unchanged and remain the
;;; simpler choice when a shell command line, not a typed subprocess API, is
;;; what's wanted.
;;;
;;; REQUIRE-ABLE (issue #256): `(require 'os)` on a `with_prelude()`
;;; environment loads exactly this file. It requires 'modules and
;;; 'ports (for PORT-P style predicates read by OS:SPAWN's returned ports)
;;; first.

(require 'modules)
(require 'ports)

(defmodule os
  (:export args executable-path cwd chdir!
           env-get env-list env-set! env-unset!
           pid ppid hostname
           now now-unix monotonic-nanos elapsed-seconds sleep sleep-seconds
           make-prng prng-next random-double
           random-bytes
           spawn process-handle process-stdin process-stdout process-stderr
           process-wait! process-try-wait! process-id process-alive-p
           process-kill! process-terminate! process-p
           exit-code exit-signal exit-success-p
           signal!))

(with-module os

  ;; ── Process identity / environment (read) ──────────────────────────

  (defun args ()
    "The process's raw argv (including argv[0], the path/name it was
invoked as), as a list of strings, via the host's std::env::args(). Requires
OS-ENV. Distinct from *ARGV* (bound by the CLI to the script arguments
that follow the script path, when running a script)."
    (os-args*))

  (defun executable-path ()
    "The absolute path to the currently running executable. Requires
OS-ENV."
    (os-executable-path*))

  (defun cwd ()
    "The process's current working directory, as a string. Requires
OS-ENV."
    (os-cwd*))

  (defun chdir! (path)
    "Change the process's current working directory to PATH. Affects every
subsequent relative-path filesystem operation process-wide. Requires
OS-ENV-WRITE."
    (os-chdir* path))

  (defun env-get (name)
    "The value of environment variable NAME, or NIL if unset. Requires
OS-ENV."
    (os-env-get* name))

  (defun env-list ()
    "Every environment variable as an alist of (name . value) strings,
sorted by name. Requires OS-ENV."
    (os-env-list*))

  (defun env-set! (name value)
    "Set environment variable NAME to VALUE (process-wide). Requires
OS-ENV-WRITE. Not safe to call concurrently with reads of the environment
from other host threads outside this interpreter -- see
src/evaluator/builtins_os.rs's OS-ENV-SET* doc comment."
    (os-env-set* name value))

  (defun env-unset! (name)
    "Remove environment variable NAME (process-wide), a no-op if it was
already unset. Requires OS-ENV-WRITE."
    (os-env-unset* name))

  (defun pid ()
    "This process's OS process ID. Requires OS-ENV."
    (os-pid*))

  (defun ppid ()
    "This process's parent process ID. Requires OS-ENV. Linux-only (reads
/proc/self/stat -- std has no portable getppid()); signals a structured
:UNSUPPORTED-PLATFORM error elsewhere."
    (os-ppid*))

  (defun hostname ()
    "This host's hostname. Requires OS-ENV. Linux-only (reads
/proc/sys/kernel/hostname -- std has no portable gethostname());
signals a structured :UNSUPPORTED-PLATFORM error elsewhere."
    (os-hostname*))

  ;; ── Time ─────────────────────────────────────────────────────────

  (defun now ()
    "Current wall-clock time since the Unix epoch, as (CONS seconds
nanoseconds). No capability required."
    (os-now*))

  (defun now-unix ()
    "Current wall-clock time since the Unix epoch, as a single float number
of seconds. No capability required."
    (let ((pair (now)))
      (+ (car pair) (/ (cdr pair) 1000000000.0))))

  (defun monotonic-nanos ()
    "Nanoseconds elapsed since an arbitrary, process-local, monotonically
increasing reference point (not comparable across processes or with
NOW/NOW-UNIX). Never goes backward. No capability required."
    (os-monotonic-nanos*))

  (defun elapsed-seconds (start-nanos)
    "Seconds elapsed since START-NANOS (a prior MONOTONIC-NANOS reading), as
a float. No capability required."
    (/ (- (monotonic-nanos) start-nanos) 1000000000.0))

  (defun sleep (ms)
    "Block the calling thread for MS milliseconds. std::thread::sleep
always sleeps for at least the requested duration (it retries internally on
spurious wakeups), so no EINTR/short-sleep behavior is observable from
Lisp. No capability required."
    (os-sleep* ms))

  (defun sleep-seconds (secs)
    "Block the calling thread for SECS seconds (a float or integer). See
SLEEP."
    (sleep (round (* secs 1000.0))))

  ;; ── Randomness ───────────────────────────────────────────────────
  ;;
  ;; Deterministic and secure randomness are distinct APIs (issue #260):
  ;; MAKE-PRNG/PRNG-NEXT are a pure, explicitly-seeded, reproducible
  ;; generator (SplitMix64); RANDOM-BYTES reads OS-backed secure entropy.
  ;; Neither touches or replaces the pre-existing global (RANDOM n)
  ;; primitive (src/evaluator/builtins_extra.rs) -- that time-seeded,
  ;; implicitly-stateful convenience function is untouched, kept under its
  ;; existing name, and remains the quick "just give me a random int"
  ;; choice; these are the typed, explicit-seed / explicit-entropy-source
  ;; choices for callers who need one or the other property.

  (defun make-prng (seed)
    "A fresh deterministic PRNG state seeded with SEED (any integer).
Purely functional: PRNG-NEXT takes a state and returns a new state, never
mutating SEED or any prior state in place -- pass the returned new state to
the next call."
    seed)

  (defun prng-next (state)
    "Advance PRNG STATE (from MAKE-PRNG or a prior PRNG-NEXT) one SplitMix64
step. Returns (CONS new-state value), where VALUE is a non-negative integer
in [0, 2^63). Deterministic: the same STATE always yields the same result.
No capability required (touches no host resource)."
    (os-prng-step* state))

  (defun random-double (state)
    "Advance PRNG STATE one step like PRNG-NEXT, returning (CONS new-state
value) where VALUE is a float in [0.0, 1.0)."
    (let ((stepped (prng-next state)))
      (cons (car stepped) (/ (cdr stepped) 9223372036854775808.0))))

  (defun random-bytes (n)
    "N cryptographically secure random bytes (from the OS entropy source,
/dev/urandom on Linux) as a fresh Array<Char>. No capability required (a
read-only entropy source, not application data)."
    (os-random-bytes* n))

  ;; ── Process spawn / control ──────────────────────────────────────
  ;;
  ;; The kernel primitive OS-SPAWN* returns raw (child stdin stdout stderr)
  ;; data; SPAWN wraps it as a small alist -- "structured records/alists in
  ;; and out" (issue #260) -- with PROCESS-HANDLE/STDIN/STDOUT/STDERR
  ;; accessors, mirroring how (shell cmd) already returns a plain
  ;; (exit-code stdout stderr) list (lib/07-shell.lisp).

  (defun $stdio-mode (mode)
    "Normalize MODE (NIL, :INHERIT, :NULL, or :PIPE) to a keyword symbol;
NIL defaults to :INHERIT."
    (if (null mode) ':inherit mode))

  (defun spawn (program &optional argv &key (inherit-env t) env cwd
                stdin stdout stderr)
    "Spawn PROGRAM (a path, not run through a shell -- no shell
interpolation) with ARGV (a list of strings, not including PROGRAM itself).
Requires OS-PROCESS.

:INHERIT-ENV (default T) -- when true, the child inherits this process's
environment, with :ENV's (name . value) pairs applied as overrides on top;
when NIL, the child's environment is exactly :ENV's pairs (nothing
inherited).
:CWD -- the child's working directory (a string), or NIL to inherit this
process's cwd.
:STDIN/:STDOUT/:STDERR -- each NIL/:INHERIT (share this process's stream),
:NULL (discard/no input), or :PIPE (return a PORTS port for it).

Returns an alist: ((:HANDLE . child) (:STDIN . port-or-nil)
(:STDOUT . port-or-nil) (:STDERR . port-or-nil)) -- ports are present only
for streams requested as :PIPE. Signals a structured error (:CATEGORY
:NOT-FOUND/:PERMISSION-DENIED/:POLICY-DENIED/...) if the executable cannot
be spawned."
    (let ((result (os-spawn* program (or argv ())
                              (if inherit-env t nil) (or env ()) cwd
                              ($stdio-mode stdin) ($stdio-mode stdout)
                              ($stdio-mode stderr))))
      (list (cons ':handle (car result))
            (cons ':stdin (cadr result))
            (cons ':stdout (caddr result))
            (cons ':stderr (cadddr result)))))

  (defun process-handle (process)
    "The OS:CHILD handle inside a SPAWN result alist."
    (cdr (assoc ':handle process)))

  (defun process-stdin (process)
    "The stdin PORTS port inside a SPAWN result alist, or NIL if stdin was
not requested as :PIPE."
    (cdr (assoc ':stdin process)))

  (defun process-stdout (process)
    "The stdout PORTS port inside a SPAWN result alist, or NIL if stdout
was not requested as :PIPE."
    (cdr (assoc ':stdout process)))

  (defun process-stderr (process)
    "The stderr PORTS port inside a SPAWN result alist, or NIL if stderr
was not requested as :PIPE."
    (cdr (assoc ':stderr process)))

  (defun process-wait! (handle)
    "Block until the child behind HANDLE (an OS:CHILD, e.g.
(process-handle spawn-result)) exits, then reap it. Returns an exit-status
alist: ((:EXIT-CODE . n-or-nil) (:SIGNAL . n-or-nil) (:SUCCESS . t-or-nil)).
Idempotent: calling this again after the child is already reaped returns
the same cached status rather than erroring. No further capability
required."
    (os-process-wait* handle))

  (defun process-try-wait! (handle)
    "Non-blocking poll of HANDLE: NIL if the child is still running, else
the same exit-status alist PROCESS-WAIT! returns (reaping the child). No
further capability required."
    (os-process-try-wait* handle))

  (defun process-id (handle)
    "HANDLE's OS PID. Retained (not NIL) even after the process has been
reaped, for diagnostics/logging. No further capability required."
    (os-process-id* handle))

  (defun process-alive-p (handle)
    "T unless HANDLE has been reaped (by PROCESS-WAIT!/PROCESS-TRY-WAIT! or
the Drop backstop). No further capability required."
    (os-process-open-p* handle))

  (defun process-kill! (handle)
    "Send SIGKILL to the child behind HANDLE (hard, unignorable kill). Does
NOT reap it -- call PROCESS-WAIT!/PROCESS-TRY-WAIT! afterward, exactly like
POSIX kill(2) + waitpid(2). Signals a :CLOSED error if already reaped. No
further capability required."
    (os-process-kill* handle))

  (defun process-terminate! (handle)
    "Send SIGTERM to the child behind HANDLE (graceful termination request;
the child may ignore or handle it). Does NOT reap it. Signals a :CLOSED
error if already reaped. No further capability required."
    (os-process-terminate* handle))

  (defun process-p (x)
    "T if X is an OS:CHILD handle (as returned by (process-handle
spawn-result))."
    (os-process-p* x))

  (defun exit-code (status)
    "The :EXIT-CODE field of an exit-status alist (PROCESS-WAIT!/
PROCESS-TRY-WAIT!'s return value), or NIL if the process was terminated by
a signal rather than exiting normally."
    (cdr (assoc ':exit-code status)))

  (defun exit-signal (status)
    "The :SIGNAL field of an exit-status alist: the signal number that
terminated the process, or NIL if it exited normally."
    (cdr (assoc ':signal status)))

  (defun exit-success-p (status)
    "T if an exit-status alist represents a normal, zero-exit-code
termination."
    (cdr (assoc ':success status)))

  ;; ── Signals ──────────────────────────────────────────────────────

  (defun signal! (pid signal-name)
    "Send SIGNAL-NAME (a typed name, e.g. :TERM, :KILL, :HUP, :INT, :USR1,
:USR2, :QUIT, :CONT, :STOP, :CHLD, :PIPE, :ALRM -- the SIG prefix is
optional) to PID (an arbitrary integer PID, not necessarily one this
process owns a handle for). Requires OS-SIGNAL. Use PROCESS-KILL!/
PROCESS-TERMINATE! instead when you hold an OS:CHILD handle for the target
-- those need no OS-SIGNAL grant (continue-authority on a resource you
already hold)."
    (os-signal* pid (princ-to-string signal-name)))

  )

(provide 'os
  '(os:args os:executable-path os:cwd os:chdir!
    os:env-get os:env-list os:env-set! os:env-unset!
    os:pid os:ppid os:hostname
    os:now os:now-unix os:monotonic-nanos os:elapsed-seconds os:sleep
    os:sleep-seconds
    os:make-prng os:prng-next os:random-double
    os:random-bytes
    os:spawn os:process-handle os:process-stdin os:process-stdout
    os:process-stderr os:process-wait! os:process-try-wait! os:process-id
    os:process-alive-p os:process-kill! os:process-terminate! os:process-p
    os:exit-code os:exit-signal os:exit-success-p
    os:signal!))
