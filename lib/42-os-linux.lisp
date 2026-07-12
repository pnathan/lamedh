;;; OS-LINUX module — typed Linux-specific filesystem facilities (issue
;;; #260, epic #253).
;;;
;;; WHY A SEPARATE MODULE: the epic ruling requires "a portable `os` module
;;; for operations with stable cross-platform semantics [and] a Linux-only
;;; module for typed Linux facilities [with] conditional availability
;;; discoverable through feature/module metadata" -- this file is that
;;; second module, kept distinct from lib/41-os.lisp so requiring 'os never
;;; silently pulls in platform-specific vocabulary. `(require 'os-linux)`
;;; makes the split visible at the call site.
;;;
;;; SCOPE (issue #260): advanced file metadata (STAT, mirroring `stat(2)`'s
;;; typed fields, never a raw C struct) and symlink target resolution
;;; (READLINK). Everything else the ticket lists as Linux-specific --
;;; pipes/descriptor duplication as a standalone primitive (spawned-process
;;; pipes are already covered by OS:SPAWN's :PIPE stdio), openat-style
;;; directory-relative operations, terminal/PTY settings, eventfd/timerfd/
;;; signalfd/epoll, and Unix-domain sockets -- is explicitly deferred (see
;;; the PR description), per the ticket's own sequencing: "eventfd/timerfd/
;;; signalfd or epoll only after the basic resource and polling model is
;;; stable".
;;;
;;; PLATFORM NOTE: the underlying Rust implementation (STD::OS::UNIX,
;;; std-only, no crate) is POSIX-portable across every Unix std supports,
;;; not literally Linux-exclusive -- but this module is deliberately named
;;; and scoped as the ticket's "Linux-only module" rather than folded into
;;; the portable OS module, and signals a structured :UNSUPPORTED-PLATFORM
;;; error (rather than failing to bind) on any non-Unix target (see
;;; src/evaluator/builtins_os.rs).
;;;
;;; CAPABILITIES: both operations reuse the existing READ-FS capability
;;; (filesystem metadata reads), per the ticket's "existing filesystem
;;; read/create/temp grants" instruction rather than inventing a parallel
;;; one.
;;;
;;; REQUIRE-ABLE (issue #256): `(require 'os-linux)` on a `with_prelude()`
;;; environment loads exactly this file. It requires 'modules first.

(require 'modules)

(defmodule os-linux
  (:export stat lstat readlink
           stat-size stat-mode stat-uid stat-gid stat-nlink stat-ino
           stat-dev stat-mtime stat-atime stat-ctime
           stat-directory-p stat-file-p stat-symlink-p))

(with-module os-linux

  (defun stat (path)
    "PATH's metadata, following a trailing symlink (like `stat(2)`), as a
structured alist: ((:SIZE . bytes) (:MODE . permission-bits) (:UID . n)
(:GID . n) (:NLINK . n) (:INO . n) (:DEV . n) (:MTIME . epoch-seconds)
(:ATIME . epoch-seconds) (:CTIME . epoch-seconds) (:IS-DIR . t-or-nil)
(:IS-FILE . t-or-nil) (:IS-SYMLINK . t-or-nil)). Requires READ-FS. Signals
a structured error (:CATEGORY :NOT-FOUND/:PERMISSION-DENIED/...) if PATH
cannot be stat'd."
    (os-linux-stat* path t))

  (defun lstat (path)
    "Like STAT, but does not follow a trailing symlink (like `lstat(2)`) --
if PATH itself is a symlink, the returned metadata describes the symlink,
not its target, and :IS-SYMLINK is T."
    (os-linux-stat* path nil))

  (defun readlink (path)
    "The target PATH points to, as a string, if PATH is a symlink. Requires
READ-FS. Signals a structured :INVALID-ARGUMENT (or :NOT-FOUND) error if
PATH is not a symlink."
    (os-linux-readlink* path))

  ;; ── Accessors ────────────────────────────────────────────────────
  ;; Thin (CDR (ASSOC ...)) wrappers, mirroring OS:EXIT-CODE/OS:EXIT-SIGNAL.

  (defun stat-size (s) (cdr (assoc ':size s)))
  (defun stat-mode (s) (cdr (assoc ':mode s)))
  (defun stat-uid (s) (cdr (assoc ':uid s)))
  (defun stat-gid (s) (cdr (assoc ':gid s)))
  (defun stat-nlink (s) (cdr (assoc ':nlink s)))
  (defun stat-ino (s) (cdr (assoc ':ino s)))
  (defun stat-dev (s) (cdr (assoc ':dev s)))
  (defun stat-mtime (s) (cdr (assoc ':mtime s)))
  (defun stat-atime (s) (cdr (assoc ':atime s)))
  (defun stat-ctime (s) (cdr (assoc ':ctime s)))
  (defun stat-directory-p (s) (cdr (assoc ':is-dir s)))
  (defun stat-file-p (s) (cdr (assoc ':is-file s)))
  (defun stat-symlink-p (s) (cdr (assoc ':is-symlink s)))

  )

(provide 'os-linux
  '(os-linux:stat os-linux:lstat os-linux:readlink
    os-linux:stat-size os-linux:stat-mode os-linux:stat-uid
    os-linux:stat-gid os-linux:stat-nlink os-linux:stat-ino
    os-linux:stat-dev os-linux:stat-mtime os-linux:stat-atime
    os-linux:stat-ctime os-linux:stat-directory-p os-linux:stat-file-p
    os-linux:stat-symlink-p))
