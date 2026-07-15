;;; 44-regex.lisp — Regular expression module
;;;
;;; Wraps Rust's `regex` crate (RE2 semantics).  Compiled regex objects
;;; are first-class values; every function that takes a regex also
;;; accepts a pattern string (compiled per call — hoist REGEX:COMPILE
;;; out of loops for performance).
;;;
;;; Limitations (RE2):
;;;   - No backreferences (\1, \2, etc.)
;;;   - No lookahead/lookbehind ((?=...), (?<=...), etc.)
;;;   - In exchange: guaranteed linear-time matching, Unicode-aware.
;;;
;;; No capability required — regex is pure computation with guaranteed
;;; linear time (no ReDoS risk even from untrusted patterns).
;;;
;;; Match representation: every match is a plain list (TEXT START END)
;;; where START and END are character indices (end-exclusive),
;;; compatible with SUBSTRING.

(require 'modules)

(defmodule regex
  (:export compile regex-p pattern escape
           match-p find find-all groups named-groups
           replace replace-all split))

(with-module regex

  (defun compile (pattern)
    "Compile PATTERN (a string) into a reusable compiled-regex object.
Signals a descriptive error on invalid syntax."
    (regex-compile* pattern))

  (defun regex-p (x)
    "T if X is a compiled regex object, NIL otherwise."
    (regex-p* x))

  (defun pattern (re)
    "Return the source pattern string of compiled regex RE."
    (regex-pattern* re))

  (defun escape (s)
    "Escape every regex metacharacter in S so the result matches S literally."
    (regex-escape* s))

  (defun match-p (re s)
    "T if RE matches anywhere in S (search semantics — anchor with ^...$ for
a full-string match)."
    (regex-is-match* re s))

  (defun find (re s &optional start)
    "First match of RE in S at or after character index START (default 0).
Returns (TEXT START END) or NIL."
    (if start
        (regex-find* re s start)
        (regex-find* re s)))

  (defun find-all (re s)
    "All non-overlapping matches of RE in S, left to right.
Returns a list of (TEXT START END) triples, or NIL if none."
    (regex-find-all* re s))

  (defun groups (re s)
    "First match of RE in S with capture groups.
Returns NIL if no match, else a list where element 0 is the whole-match
triple and element I is group I's (TEXT START END) triple — or NIL for a
group that did not participate."
    (regex-captures* re s))

  (defun named-groups (re s)
    "First match of RE in S with named capture groups.
Returns NIL if no match, else an alist of (NAME-STRING . (TEXT START END)).
Unparticipated named groups have NIL as the cdr."
    (regex-captures-named* re s))

  (defun replace (re s replacement)
    "Return a new string with the first match of RE in S replaced.
REPLACEMENT is a template string: $1/$2 and ${name} expand to captures,
$$ is a literal $."
    (regex-replace* re s replacement))

  (defun replace-all (re s replacement)
    "Return a new string with every match of RE in S replaced.
REPLACEMENT uses the same template syntax as REGEX:REPLACE."
    (regex-replace-all* re s replacement))

  (defun split (re s &optional limit)
    "Split S on matches of RE.  With LIMIT, return at most LIMIT pieces
\(the last containing the unsplit remainder\).  Adjacent delimiters yield
empty strings."
    (if limit
        (regex-split* re s limit)
        (regex-split* re s)))

) ; end with-module
