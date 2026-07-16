# Lamedh Reference Manual

Auto-generated from Lisp documentation database.

---

## Categories

- INTROSPECTION - Inspecting registered definitions and compiled code
- REGEX - Regular expressions (RE2 semantics; lib/44-regex.lisp)
- FLAGS - Global condition/signal flags
- ENVIRONMENTS - First-class environment objects
- MODULES - REQUIRE/PROVIDE load-once library loading (issue #256)
- CAPABILITIES - Capability feature flags and shell access
- FILESYSTEM - File system I/O (requires READ-FS / CREATE-FS / TEMP-FS capability)
- ARRAYS - Mutable random-access arrays (Lisp 1.5 Appendix A)
- BITWISE - Bitwise operations
- HASH-TABLES - Hash tables
- PLISTS - Property lists
- META - Metaprogramming
- ERRORS - Error handling
- IO - Input/Output
- SPECIAL-FORMS - Special forms and macros
- OS-LINUX - Typed Linux file metadata and symlinks (OS-LINUX module, lib/42-os-linux.lisp, issue #260); READ-FS capability
- OS - Process identity/environment, time, randomness, and process spawn/control (OS module, lib/41-os.lisp, issue #260); OS-ENV/OS-ENV-WRITE/OS-PROCESS/OS-SIGNAL capabilities
- HTTP - HTTP/1.1 client and server (http:// always, https:// with the net-tls cargo feature -- issue #365)
- TLS - TLS client/server wrap of a connected TCP port (TLS module, lib/43-tls.lisp, issue #365); off-by-default net-tls cargo feature, rustls/ring
- UDP - UDP bind/send-to/receive-from datagram sockets (UDP module, lib/39-udp.lisp, issue #258); NET-CONNECT/NET-LISTEN capabilities
- TCP - TCP connect/bind/listen/accept over binary ports (TCP module, lib/38-tcp.lisp, issue #258); NET-CONNECT/NET-LISTEN capabilities
- NET - Addresses and DNS resolution (NET module, lib/37-net.lisp, issue #258); NET-DNS capability
- MIME - Case-insensitive multi-value headers and Content-Type parse/build (MIME module, lib/36-mime.lisp, issue #257)
- JSON - JSON parse/stringify (JSON module, lib/35-json.lisp, issue #257)
- URL - URL parse/build, percent-encoding, and query-string parse/build (URL module, lib/34-url.lisp, issue #257)
- HEX - Hexadecimal encode/decode over Array<Char> bytes (HEX module, lib/33-hex.lisp, issue #257)
- BASE64 - Base64 encode/decode over Array<Char> bytes (BASE64 module, lib/32-base64.lisp, issue #257)
- PORTS - Synchronous binary I/O ports (PORTS module, lib/31-ports.lisp)
- TEXT - Explicit String <-> UTF-8 Array<Char> boundary (TEXT module, lib/30-text.lisp)
- STRINGS - String operations
- LISTS - List manipulation
- PREDICATES - Type and value predicates
- ARITHMETIC - Numeric operations

---

# INTROSPECTION Functions

Inspecting registered definitions and compiled code

---

### DESCRIBE

**Type:** `FUNCTION`

**Syntax:** `(describe 'sym)`

Print a brief summary of what a symbol (or value) is: its kind, parameters/arity, value, any typed (JIT) signature and compiled status, and its docstring.

**Arguments:**
- `SYM` - A (usually quoted) symbol, or any value

**Returns:** T (the summary is printed to stdout)

**Examples:**
```lisp
(DESCRIBE (QUOTE +))  ; => T
(DESCRIBE (QUOTE CAR))  ; => T
```

**See also:** SEE-SOURCE, DISASSEMBLE, DOCUMENTATION, HELP

---

### SEE-SOURCE

**Type:** `FUNCTION`

**Syntax:** `(see-source 'sym) or (see-source 'sym t)`

Reconstruct the source form the evaluator registered for an operative (lambda, fexpr, macro, vau). With no second argument it returns the form; with a non-NIL second argument it prints the form as an indented tree and returns T.

**Arguments:**
- `SYM` - A (usually quoted) symbol bound to an operative, or the operative value itself
- `TREE` - Optional: when non-NIL, render an indented tree to stdout

**Returns:** The reconstructed source form, or T in tree mode

**Examples:**
```lisp
(SEE-SOURCE (QUOTE CUBE))  ; => (LAMBDA (X) (* X (* X X)))
```

**See also:** DESCRIBE, DISASSEMBLE, MACROEXPAND

---

### DISASSEMBLE

**Type:** `FUNCTION`

**Syntax:** `(disassemble 'sym)`

Print the typed-core pseudo-assembly of a jotted (defun-typed) function: the typed IR lowered to a flat register/label instruction listing. Reports clearly when the symbol has no typed edition.

**Arguments:**
- `SYM` - A quoted symbol naming a typed (defun-typed) function

**Returns:** T (the listing is printed to stdout)

**Examples:**
```lisp
(DISASSEMBLE (QUOTE FACT))  ; => T
```

**See also:** DESCRIBE, SEE-SOURCE, DEFUN-TYPED

---

### DOCUMENTATION

**Type:** `FUNCTION`

**Syntax:** `(documentation symbol)`

Returns the docstring for a symbol.

**See also:** GETP, HELP

---

### SIGNATURE

**Type:** `FUNCTION`

**Syntax:** `(signature 'sym)`

Loud type inference (companion to DEFUN*'s silent fallback): the inferred type signature of a typed function as a readable sexpr, e.g. (INT64 INT64 -> INT64). NIL for an untyped function — a plain lambda, a DEFUN*/DEFUN-TYPED whose registry entry has been shadowed by a later plain redefinition, or a name that was never typed at all.

**Arguments:**
- `SYM` - A quoted symbol naming a function

**Returns:** A signature sexpr, or NIL

**Examples:**
```lisp
(PROGN (DEFUN* ADD (X INT64) (Y INT64) (+ X Y)) (SIGNATURE (QUOTE ADD)))  ; => (INT64 INT64 -> INT64)
```

**See also:** COMPILED-P, WHY-NOT-TYPED, DEFUN*, EXPLAIN-COMPILE

---

### COMPILED-P

**Type:** `FUNCTION`

**Syntax:** `(compiled-p 'sym)`

Loud type inference: the execution tier a typed function will actually run on. NATIVE when a Cranelift native edition exists (the jit feature only), CLOSURE when only the portable closure edition does (always true for a typed function when the jit feature is disabled, and possible even with it enabled if native codegen fell back), or NIL for a plain interpreted function / unknown name.

**Arguments:**
- `SYM` - A quoted symbol naming a function

**Returns:** NATIVE, CLOSURE, or NIL

**Examples:**
```lisp
(PROGN (DEFUN* ADD (X INT64) (Y INT64) (+ X Y)) (COMPILED-P (QUOTE ADD)))  ; => NATIVE
```

**See also:** SIGNATURE, WHY-NOT-TYPED, DEFUN*, DISASSEMBLE

---

### WHY-NOT-TYPED

**Type:** `FUNCTION`

**Syntax:** `(why-not-typed 'sym)`

Loud type inference: for a DEFUN* that fell back to an ordinary lambda, the concrete inference-failure reason recorded at the fallback site — e.g. which expression or operand defeated typing — not just a generic "inference failed". NIL if the function is currently typed, or was never a DEFUN* candidate. The reason is cleared automatically the next time DEFUN* (re)defines the same name and succeeds.

**Arguments:**
- `SYM` - A quoted symbol naming a function

**Returns:** A reason string, or NIL

**Examples:**
```lisp
(PROGN (DEFUN* MK (A B) (CONS A B)) (WHY-NOT-TYPED (QUOTE MK)))  ; => "call to unknown function `CONS`"
```

**See also:** SIGNATURE, COMPILED-P, DEFUN*, EXPLAIN-COMPILE

---

# REGEX Functions

Regular expressions (RE2 semantics; lib/44-regex.lisp)

---

### REGEX:COMPILE

**Type:** `FUNCTION`

**Syntax:** `(regex:compile pattern)`

Compiles PATTERN (a string) into a reusable compiled-regex object; signals a descriptive error on invalid syntax. Hoist out of loops: functions that take a regex also accept a raw pattern string, but that recompiles on every call.

**Examples:**
```lisp
(REGEX:REGEX-P (REGEX:COMPILE "a+"))  ; => T
```

**See also:** REGEX:REGEX-P, REGEX:PATTERN, REGEX:MATCH-P

---

### REGEX:REGEX-P

**Type:** `FUNCTION`

**Syntax:** `(regex:regex-p x)`

Returns T if X is a compiled regex object (from REGEX:COMPILE), NIL otherwise.

**Examples:**
```lisp
(REGEX:REGEX-P (REGEX:COMPILE "a+"))  ; => T
(REGEX:REGEX-P "a+")  ; => ()
```

**See also:** REGEX:COMPILE, REGEX:PATTERN

---

### REGEX:PATTERN

**Type:** `FUNCTION`

**Syntax:** `(regex:pattern re)`

Returns the source pattern string of a compiled regex RE.

**Examples:**
```lisp
(REGEX:PATTERN (REGEX:COMPILE "a+"))  ; => "a+"
```

**See also:** REGEX:COMPILE

---

### REGEX:ESCAPE

**Type:** `FUNCTION`

**Syntax:** `(regex:escape s)`

Returns a copy of S with every regex metacharacter backslash-escaped, so the result matches S literally when used as a pattern.

**Examples:**
```lisp
(REGEX:MATCH-P (REGEX:ESCAPE "a.b") "a.b")  ; => T
```

**See also:** REGEX:COMPILE, REGEX:MATCH-P

---

### REGEX:MATCH-P

**Type:** `FUNCTION`

**Syntax:** `(regex:match-p re s)`

Returns T if RE matches anywhere in S (search semantics), NIL otherwise. Anchor with ^...$ for a full-string match. RE may be a compiled regex or a pattern string.

**Examples:**
```lisp
(REGEX:MATCH-P "^a+$" "aaa")  ; => T
(REGEX:MATCH-P "^a+$" "aab")  ; => ()
```

**See also:** REGEX:FIND, REGEX:FIND-ALL

---

### REGEX:FIND

**Type:** `FUNCTION`

**Syntax:** `(regex:find re s &optional start)`

Returns the first match of RE in S at or after character index START (default 0) as a (TEXT START END) triple, or NIL if there is none.

**Examples:**
```lisp
(REGEX:FIND "b" "abcb" 2)  ; => ("b" 3 4)
(REGEX:FIND "z" "abc")  ; => ()
```

**See also:** REGEX:FIND-ALL, REGEX:MATCH-P, REGEX:GROUPS

---

### REGEX:FIND-ALL

**Type:** `FUNCTION`

**Syntax:** `(regex:find-all re s)`

Returns a list of every non-overlapping match of RE in S, left to right, each a (TEXT START END) triple; NIL if there are none.

**Examples:**
```lisp
(REGEX:FIND-ALL "a." "axby az")  ; => (("ax" 0 2) ("az" 5 7))
```

**See also:** REGEX:FIND, REGEX:SPLIT

---

### REGEX:GROUPS

**Type:** `FUNCTION`

**Syntax:** `(regex:groups re s)`

First match of RE in S with capture groups: NIL if no match, else a list whose element 0 is the whole-match (TEXT START END) triple and whose element I is capture group I's triple — or NIL for a group that did not participate.

**Examples:**
```lisp
(REGEX:GROUPS "(a)(b)" "ab")  ; => (("ab" 0 2) ("a" 0 1) ("b" 1 2))
```

**See also:** REGEX:NAMED-GROUPS, REGEX:FIND

---

### REGEX:NAMED-GROUPS

**Type:** `FUNCTION`

**Syntax:** `(regex:named-groups re s)`

First match of RE in S with named capture groups: NIL if no match, else an alist of (NAME-STRING . (TEXT START END)); a named group that did not participate has NIL as its cdr. Name groups with (?P<name>...) or (?<name>...).

**Examples:**
```lisp
(REGEX:NAMED-GROUPS "(?P<x>a)" "a")  ; => (("x" "a" 0 1))
```

**See also:** REGEX:GROUPS

---

### REGEX:REPLACE

**Type:** `FUNCTION`

**Syntax:** `(regex:replace re s replacement)`

Returns a new string with the first match of RE in S replaced by REPLACEMENT, a template string in which $1/$2 and ${name} expand to captures and $$ is a literal dollar sign. Returns S unchanged if RE does not match.

**Examples:**
```lisp
(REGEX:REPLACE "a" "aaa" "X")  ; => "Xaa"
```

**See also:** REGEX:REPLACE-ALL

---

### REGEX:REPLACE-ALL

**Type:** `FUNCTION`

**Syntax:** `(regex:replace-all re s replacement)`

Returns a new string with every match of RE in S replaced by REPLACEMENT (same template syntax as REGEX:REPLACE).

**Examples:**
```lisp
(REGEX:REPLACE-ALL "\\d+" "a1b22" "#")  ; => "a#b#"
```

**See also:** REGEX:REPLACE

---

### REGEX:SPLIT

**Type:** `FUNCTION`

**Syntax:** `(regex:split re s &optional limit)`

Splits S on matches of RE, returning a list of the pieces between matches. With LIMIT, returns at most LIMIT pieces, the last holding the unsplit remainder. Adjacent delimiters yield empty strings.

**Examples:**
```lisp
(REGEX:SPLIT "," "a,b,c")  ; => ("a" "b" "c")
```

**See also:** REGEX:FIND-ALL

---

# FLAGS Functions

Global condition/signal flags

---

### SET-FLAG

**Type:** `FUNCTION`

**Syntax:** `(set-flag name)`

Sets the global condition flag named name (a symbol or string) to true. Condition flags are global boolean signals used to communicate exceptional conditions such as arithmetic overflow. The built-in flag "OVERFLOW" is set by some arithmetic operations when overflow is detected. Custom flags can be set and tested by application code.

**Examples:**
```lisp
(SET-FLAG (QUOTE DONE))  ; => T
(FLAG-SET-P (QUOTE DONE))  ; => T
```

**See also:** CLEAR-FLAG, FLAG-SET-P, CLEAR-ALL-FLAGS

---

### CLEAR-FLAG

**Type:** `FUNCTION`

**Syntax:** `(clear-flag name)`

Clears the global condition flag named name (a symbol or string), setting it to false. Has no effect if the flag was not set. See SET-FLAG for an overview of condition flags.

**Examples:**
```lisp
(SET-FLAG (QUOTE X))  ; => T
(CLEAR-FLAG (QUOTE X))  ; => T
(FLAG-SET-P (QUOTE X))  ; => ()
```

**See also:** SET-FLAG, FLAG-SET-P, CLEAR-ALL-FLAGS

---

### FLAG-SET-P

**Type:** `FUNCTION`

**Syntax:** `(flag-set-p name)`

Returns T if the global condition flag named name is currently set; returns NIL otherwise. The name may be a symbol or a string. Flags default to unset (false) until explicitly set with SET-FLAG.

**Examples:**
```lisp
(FLAG-SET-P (QUOTE OVERFLOW))  ; => ()
(SET-FLAG (QUOTE OVERFLOW))  ; => T
(FLAG-SET-P (QUOTE OVERFLOW))  ; => T
```

**See also:** SET-FLAG, CLEAR-FLAG, CLEAR-ALL-FLAGS

---

### CLEAR-ALL-FLAGS

**Type:** `FUNCTION`

**Syntax:** `(clear-all-flags)`

Clears all global condition flags at once. Takes no arguments. Useful at the start of a test suite or computation to ensure a clean flag state.

**See also:** CLEAR-FLAG, SET-FLAG, FLAG-SET-P

---

# ENVIRONMENTS Functions

First-class environment objects

---

### THE-ENVIRONMENT

**Type:** `FUNCTION`

**Syntax:** `(the-environment)`

Returns the current lexical environment as a first-class Environment object. This is a live reference — any bindings established after the call are visible through the returned object. The environment can be passed to EVAL, EVLIS, EVCON, or MAKE-ENVIRONMENT as a parent to evaluate code in a specific scope. Primarily used by VAU operatives (via their env-param) and by metaprogramming utilities.

**See also:** MAKE-ENVIRONMENT, CURRENT-ENVIRONMENT, EVAL, EVLIS, VAU

---

### MAKE-ENVIRONMENT

**Type:** `FUNCTION`

**Syntax:** `(make-environment) or (make-environment parent-env)`

Creates a new first-class environment. With no arguments creates a fresh root environment pre-populated with all builtins (equivalent to a clean Lamedh session before loading the standard library). With one argument — an Environment object — creates a child environment that inherits all bindings from parent-env while new definitions are isolated to the child. Useful for sandboxing, module systems, and eval-in-context patterns.

**Examples:**
```lisp
(LET ((E (MAKE-ENVIRONMENT (THE-ENVIRONMENT)))) (EVAL (QUOTE (DEF X 42)) E) (EVAL (QUOTE X) E))  ; => 42
```

**See also:** THE-ENVIRONMENT, CURRENT-ENVIRONMENT, EVAL

---

### CURRENT-ENVIRONMENT

**Type:** `FUNCTION`

**Syntax:** `(current-environment)`

Returns a snapshot of all currently visible bindings as a hash table (symbol → value). Unlike THE-ENVIRONMENT, this returns a new, frozen hash table rather than a live environment object. Useful for inspection, debugging, and serialisation. The keys are symbols; the values are the current binding values at call time.

**See also:** THE-ENVIRONMENT, MAKE-ENVIRONMENT, KEYS

---

# MODULES Functions

REQUIRE/PROVIDE load-once library loading (issue #256)

---

### REQUIRE

**Type:** `FUNCTION`

**Syntax:** `(require 'name)`

Loads module NAME (a symbol or string) at most once in this environment; returns NAME's canonical (uppercase) symbol. A second REQUIRE of an already-loaded module is a no-op -- it never re-evaluates the source. NAME resolves through a per-environment registry, in order: (1) sources a host registered directly (Rust: env.register_module); (2) sources embedded in the binary (the numbered optional library files -- SHELL, TESTING, CONDENSATION, TEXT, ...); (3) -- only under the READ-FS capability -- files under host-configured disk search paths. A REQUIRE for a module already mid-load (directly or transitively) is a hard cycle error naming the full chain. A module whose source signals an error, or which finishes without calling (PROVIDE 'NAME), is NOT marked loaded -- whatever top-level definitions it already ran are not rolled back. See docs/manual/10-modules.md section 10.7 for the full story, and lib/06-require.lisp for the implementation.

**Examples:**
```lisp
(REQUIRE (QUOTE SHELL))  ; => SHELL
(REQUIRE (QUOTE SHELL))  ; => SHELL
```

**See also:** PROVIDE, REQUIRE-RELOAD, LOADED-MODULES, MODULE-STATE, MODULE-INFO, DEFMODULE

---

### PROVIDE

**Type:** `FUNCTION`

**Syntax:** `(provide 'name) or (provide 'name exports)`

Called from within a module's own source (as loaded by REQUIRE) to mark NAME complete; conventionally the module's last top-level form. REQUIRE signals an error if a module's source finishes evaluating without a matching PROVIDE. The optional EXPORTS argument is a list of symbol names this module claims to define -- metadata only, not enforcement (Lamedh has no reader-level privacy or namespaces); REQUIRE warns if a declared export ends up unbound, and warns (or, with *REQUIRE-STRICT-EXPORTS* bound to T, errors) if a declared export was already claimed by a different module.

**Examples:**
```lisp
(PROVIDE (QUOTE MY-APP))  ; => MY-APP
```

**See also:** REQUIRE, REQUIRE-RELOAD

---

### REQUIRE-RELOAD

**Type:** `FUNCTION`

**Syntax:** `(require-reload 'name)`

Development/debugging operation: forces NAME to be re-resolved and re-evaluated via REQUIRE's normal procedure even though it is already loaded. Ordinary REQUIRE never does this implicitly -- use REQUIRE-RELOAD when iterating on a registered or disk module's source without restarting the interpreter. Errors if NAME is currently mid-load.

**See also:** REQUIRE, PROVIDE

---

### LOADED-MODULES

**Type:** `FUNCTION`

**Syntax:** `(loaded-modules)`

Returns all module names currently REQUIRE-loaded in this environment, in no particular order.

**See also:** REQUIRE, MODULE-STATE, MODULE-INFO

---

### MODULE-STATE

**Type:** `FUNCTION`

**Syntax:** `(module-state 'name)`

Returns 'REQUIRE-LOADED, 'REQUIRE-LOADING, 'REQUIRE-UNLOADED, or NIL if NAME has never been REQUIREd, PROVIDEd, or registered in this environment.

**See also:** REQUIRE, LOADED-MODULES, MODULE-INFO

---

### MODULE-INFO

**Type:** `FUNCTION`

**Syntax:** `(module-info 'name)`

Returns an alist of diagnostic metadata REQUIRE tracks for NAME: STATE, SOURCE (an origin string such as "registered", "embedded", or "disk:<path>"), DEPS (names REQUIREd while NAME itself was loading), EXPORTS, and ERROR (the last load failure's message, or NIL).

**See also:** REQUIRE, MODULE-STATE, LOADED-MODULES

---

# CAPABILITIES Functions

Capability feature flags and shell access

---

### FEATURE-ENABLED-P

**Type:** `FUNCTION`

**Syntax:** `(feature-enabled-p name)`

Returns T if the capability (feature) named name is currently enabled; returns NIL otherwise. Capability names are case-insensitive. Available capabilities: SHELL (subprocess execution), READ-FS (filesystem reads), CREATE-FS (filesystem mutation), TEMP-FS (temp file creation), IO (stdin reads). All capabilities are OFF by default in every environment; they must be granted by the host via Rust API or the --capability CLI flag.

**Examples:**
```lisp
(FEATURE-ENABLED-P (QUOTE SHELL))  ; => ()
(FEATURE-ENABLED-P "READ-FS")  ; => ()
```

**See also:** FEATURES, SHELL, READ-FILE, WRITE-FILE

---

### FEATURES

**Type:** `FUNCTION`

**Syntax:** `(features)`

Returns a sorted list of strings naming all currently-enabled capabilities. An empty list means no capabilities have been granted. Lisp code cannot grant capabilities to itself; this function is read-only introspection.

**Examples:**
```lisp
(FEATURES)  ; => ()
```

**See also:** FEATURE-ENABLED-P, SHELL, READ-FILE

---

### SHELL

**Type:** `FUNCTION`

**Syntax:** `(shell command) or (shell program arg...)`

Runs a shell command and returns a list (exit-code stdout stderr) as three values. With a single string argument the command is passed to "sh -c"; with multiple arguments the first is the program and the rest are arguments passed directly (no shell expansion). Requires the SHELL capability to be enabled.

The return value is always a proper three-element list:
  (0)   exit code as an integer (-1 if the process exited without a code)
  (1)   stdout as a string
  (2)   stderr as a string

Use the helpers in lib/07-shell.lisp (SHELL-EXIT-CODE, SHELL-STDOUT, SHELL-STDERR, SHELL-OK-P, SH) for more ergonomic access to these values.

Grant the capability: --capability SHELL on the CLI, or (env.enable_feature "SHELL") from Rust host code.

**Examples:**
```lisp
(SHELL "echo hello")  ; => (0 "hello\n" "")
(SHELL "ls" "/tmp")  ; => (0 "..." "")
```

**See also:** FEATURE-ENABLED-P, FEATURES

---

# FILESYSTEM Functions

File system I/O (requires READ-FS / CREATE-FS / TEMP-FS capability)

---

### READ-FILE

**Type:** `FUNCTION`

**Syntax:** `(read-file path)`

Reads the entire contents of the file at path as a UTF-8 string. Signals an error if the file does not exist, cannot be read, or is not valid UTF-8. Requires the READ-FS capability.

**Examples:**
```lisp
(READ-FILE "/etc/hostname")  ; => "myhost\n"
```

**See also:** WRITE-FILE, READ-FILE-BYTE, READ-FILE-SECTION, FILE-EXISTS-P, FEATURE-ENABLED-P

---

### READ-FILE-BYTE

**Type:** `FUNCTION`

**Syntax:** `(read-file-byte path offset)`

Reads a single byte at byte offset from the file at path. Returns the byte value as an integer (0–255), or NIL if offset is at or past the end of the file. Requires the READ-FS capability. Useful for binary file inspection; for text use READ-FILE or READ-FILE-SECTION.

**Examples:**
```lisp
(READ-FILE-BYTE "/bin/true" 0)  ; => 127
```

**See also:** READ-FILE, READ-FILE-SECTION, FEATURE-ENABLED-P

---

### READ-FILE-SECTION

**Type:** `FUNCTION`

**Syntax:** `(read-file-section path offset len)`

Reads up to len bytes starting at byte offset from the file at path and decodes them as UTF-8 STRICTLY: invalid bytes signal an error (naming the offending byte offset) rather than being silently coerced. Returns a shorter string if fewer than len bytes are available. Use READ-FILE-SECTION-LOSSY for replacement-character decoding, or READ-FILE-SECTION-BYTES for the raw bytes. Requires the READ-FS capability.

**See also:** READ-FILE-SECTION-LOSSY, READ-FILE-SECTION-BYTES, READ-FILE, READ-FILE-BYTE, WRITE-FILE, FEATURE-ENABLED-P

---

### READ-FILE-SECTION-LOSSY

**Type:** `FUNCTION`

**Syntax:** `(read-file-section-lossy path offset len)`

Like READ-FILE-SECTION but decodes UTF-8 lossily: invalid bytes become the U+FFFD replacement character instead of signaling an error. The explicit opt-in to lossy decoding, mirroring TEXT:UTF8->STRING-LOSSY. Requires the READ-FS capability.

**See also:** READ-FILE-SECTION, READ-FILE-SECTION-BYTES, FEATURE-ENABLED-P

---

### READ-FILE-SECTION-BYTES

**Type:** `FUNCTION`

**Syntax:** `(read-file-section-bytes path offset len)`

Reads up to len bytes starting at byte offset from the file at path and returns them as an Array of bytes (Array<Char>), with no text decoding. Cross the text boundary yourself with TEXT:UTF8->STRING / TEXT:UTF8->STRING-LOSSY, or feed the bytes to a codec (BASE64, HEX, JSON, ...). Returns a shorter array if fewer than len bytes are available. Requires the READ-FS capability.

**See also:** READ-FILE-SECTION, READ-FILE-SECTION-LOSSY, FEATURE-ENABLED-P

---

### WRITE-FILE

**Type:** `FUNCTION`

**Syntax:** `(write-file path content)`

Writes the string content to the file at path, replacing any existing content. Creates the file if it does not exist. Returns T on success; signals an error on failure. Requires the CREATE-FS capability. For appending or streaming writes, use the SHELL capability with shell tools.

**Examples:**
```lisp
(WRITE-FILE "/tmp/hello.txt" "hello world\n")  ; => T
```

**See also:** READ-FILE, MAKE-TEMP-FILE, FEATURE-ENABLED-P

---

### READ-SEXPR-FILE

**Type:** `FUNCTION`

**Syntax:** `(read-sexpr-file path)`

Reads path's full text (requires READ-FS) and parses it into a list of every top-level s-expression it contains, via READ-STRING. The inverse of WRITE-SEXPR-FILE (issue #150, lib/18-format.lisp).

**See also:** WRITE-SEXPR-FILE, READ-FILE, READ-STRING

---

### WRITE-SEXPR-FILE

**Type:** `FUNCTION`

**Syntax:** `(write-sexpr-file path forms)`

Writes forms (a list of s-expressions) to path (requires CREATE-FS), one per line in readable (PRIN1) form; the inverse of READ-SEXPR-FILE (issue #150, lib/18-format.lisp).

**See also:** READ-SEXPR-FILE, WRITE-FILE, PRIN1-TO-STRING

---

### FILE-EXISTS-P

**Type:** `FUNCTION`

**Syntax:** `(file-exists-p path)`

Returns T if something (file, directory, symlink, etc.) exists at path; returns NIL otherwise. Requires the READ-FS capability.

**See also:** FILE-P, DIRECTORY-P, FILE-READABLE-P, FEATURE-ENABLED-P

---

### DIRECTORY-P

**Type:** `FUNCTION`

**Syntax:** `(directory-p path)`

Returns T if path names an existing directory; returns NIL otherwise. Requires the READ-FS capability.

**See also:** FILE-P, FILE-EXISTS-P, DIRECTORY-FILES, FEATURE-ENABLED-P

---

### FILE-P

**Type:** `FUNCTION`

**Syntax:** `(file-p path)`

Returns T if path names an existing regular file (not a directory or special file); returns NIL otherwise. Requires the READ-FS capability.

**See also:** DIRECTORY-P, FILE-EXISTS-P, FILE-READABLE-P, FEATURE-ENABLED-P

---

### FILE-READABLE-P

**Type:** `FUNCTION`

**Syntax:** `(file-readable-p path)`

Returns T if the file at path can be opened for reading by the current process; returns NIL otherwise. Requires the READ-FS capability. Implemented by attempting to open the file.

**See also:** FILE-WRITABLE-P, FILE-EXECUTABLE-P, FILE-EXISTS-P, FEATURE-ENABLED-P

---

### FILE-WRITABLE-P

**Type:** `FUNCTION`

**Syntax:** `(file-writable-p path)`

Returns T if the file at path exists and is not marked read-only; returns NIL otherwise. Requires the READ-FS capability. Checks the filesystem metadata permissions; does not attempt to open the file.

**See also:** FILE-READABLE-P, FILE-EXECUTABLE-P, FEATURE-ENABLED-P

---

### FILE-EXECUTABLE-P

**Type:** `FUNCTION`

**Syntax:** `(file-executable-p path)`

Returns T if the file at path has at least one executable permission bit set (Unix execute bit); returns NIL otherwise or on non-Unix platforms. Requires the READ-FS capability.

**See also:** FILE-READABLE-P, FILE-WRITABLE-P, FEATURE-ENABLED-P

---

### FILE-SIZE

**Type:** `FUNCTION`

**Syntax:** `(file-size path)`

Returns the size of the file at path in bytes as an integer. Signals an error if the file does not exist or cannot be accessed. Requires the READ-FS capability.

**Examples:**
```lisp
(FILE-SIZE "/etc/hostname")  ; => 8
```

**See also:** FILE-EXISTS-P, READ-FILE, FEATURE-ENABLED-P

---

### DIRECTORY-FILES

**Type:** `FUNCTION`

**Syntax:** `(directory-files path)`

Returns a sorted list of filename strings (not full paths) for all entries in the directory at path. Includes both files and subdirectories; does not recurse. Signals an error if path is not a readable directory. Requires the READ-FS capability.

**Examples:**
```lisp
(DIRECTORY-FILES "/tmp")  ; => ("file1.txt" "subdir")
```

**See also:** DIRECTORY-P, FILE-EXISTS-P, FEATURE-ENABLED-P

---

### FILE-NEWER-P

**Type:** `FUNCTION`

**Syntax:** `(file-newer-p path1 path2)`

Returns T if path1's modification time is strictly later than path2's modification time; returns NIL otherwise. Both files must exist. Requires the READ-FS capability. Useful for incremental build-like logic.

**See also:** FILE-EXISTS-P, FILE-SIZE, FEATURE-ENABLED-P

---

### CHMOD

**Type:** `FUNCTION`

**Syntax:** `(chmod path mode)`

Changes the permissions of the file at path to mode. Mode may be an integer (the raw Unix mode value) or an octal string like "755". Returns T on success; signals an error on failure. Only supported on Unix; signals an error on Windows. Requires the CREATE-FS capability.

**Examples:**
```lisp
(CHMOD "/tmp/myscript.sh" "755")  ; => T
```

**See also:** FILE-EXECUTABLE-P, WRITE-FILE, FEATURE-ENABLED-P

---

### CREATE-DIRECTORY

**Type:** `FUNCTION`

**Syntax:** `(create-directory path)`

Creates the directory at path and all intermediate directories as needed (like mkdir -p). Returns T on success; signals an error on failure. Requires the CREATE-FS capability.

**Examples:**
```lisp
(CREATE-DIRECTORY "/tmp/new/subdir")  ; => T
```

**See also:** DIRECTORY-P, DELETE-FILE, FEATURE-ENABLED-P

---

### DELETE-FILE

**Type:** `FUNCTION`

**Syntax:** `(delete-file path)`

Deletes the regular file at path. Signals an error if the file does not exist or is a directory. Returns T on success. Requires the CREATE-FS capability. To remove directories, use shell tools via SHELL.

**Examples:**
```lisp
(DELETE-FILE "/tmp/old.txt")  ; => T
```

**See also:** RENAME-FILE, WRITE-FILE, FILE-EXISTS-P, FEATURE-ENABLED-P

---

### RENAME-FILE

**Type:** `FUNCTION`

**Syntax:** `(rename-file from to)`

Renames (or moves) the file or directory at from to to. On the same filesystem this is atomic; across filesystems it may copy-then-delete. Returns T on success; signals an error on failure. Requires the CREATE-FS capability.

**Examples:**
```lisp
(RENAME-FILE "/tmp/old.txt" "/tmp/new.txt")  ; => T
```

**See also:** DELETE-FILE, WRITE-FILE, FEATURE-ENABLED-P

---

### MAKE-TEMP-FILE

**Type:** `FUNCTION`

**Syntax:** `(make-temp-file) or (make-temp-file prefix)`

Creates a new empty temporary file and returns its path as a string. The optional prefix string is prepended to the filename. The file is created atomically in the system temp directory. The caller is responsible for deleting the file when done. Requires the TEMP-FS capability.

**Examples:**
```lisp
(MAKE-TEMP-FILE "myapp-")  ; => "/tmp/myapp-abc123"
```

**See also:** MAKE-TEMP-DIRECTORY, WRITE-FILE, DELETE-FILE, FEATURE-ENABLED-P

---

### MAKE-TEMP-DIRECTORY

**Type:** `FUNCTION`

**Syntax:** `(make-temp-directory) or (make-temp-directory prefix)`

Creates a new empty temporary directory and returns its path as a string. The optional prefix string is prepended to the directory name. The directory is created in the system temp directory. The caller is responsible for deleting the directory and its contents when done. Requires the TEMP-FS capability.

**Examples:**
```lisp
(MAKE-TEMP-DIRECTORY "work-")  ; => "/tmp/work-abc123"
```

**See also:** MAKE-TEMP-FILE, CREATE-DIRECTORY, DELETE-FILE, FEATURE-ENABLED-P

---

# ARRAYS Functions

Mutable random-access arrays (Lisp 1.5 Appendix A)

---

### ARRAY

**Type:** `FUNCTION`

**Syntax:** `(array n)`

Creates and returns a new mutable array of n elements, all initialised to NIL. Lisp 1.5 Appendix A name; MAKE-ARRAY is the longer alias. Arrays are random-access containers with O(1) indexed get/set. Use FETCH/STORE to access elements, ARRAY-LENGTH* to query the size.

**Examples:**
```lisp
(LET ((A (ARRAY 3))) (STORE A 0 (QUOTE X)) (FETCH A 0))  ; => X
```

**See also:** MAKE-ARRAY, FETCH, STORE, ARRAY-LENGTH*, ARRAYP

---

### MAKE-ARRAY

**Type:** `FUNCTION`

**Syntax:** `(make-array n)`

Alias for ARRAY. Creates a mutable array of n NIL-initialised elements. See ARRAY for full documentation.

**Examples:**
```lisp
(MAKE-ARRAY 5)  ; => "an array of 5 NILs"
```

**See also:** ARRAY, FETCH, STORE, ARRAY-LENGTH*, ARRAYP

---

### FETCH

**Type:** `FUNCTION`

**Syntax:** `(fetch array index)`

Returns the element of array at 0-based integer index. Signals an error if index is out of bounds. Lisp 1.5 Appendix A name; ARRAY-FETCH* is the longer alias, AREF the Common-Lisp-style one.

**Examples:**
```lisp
(LET ((A (ARRAY 3))) (STORE A 1 (QUOTE HELLO)) (FETCH A 1))  ; => HELLO
```

**See also:** ARRAY-FETCH*, AREF, STORE, ARRAY, ARRAY-LENGTH*

---

### ARRAY-FETCH*

**Type:** `FUNCTION`

**Syntax:** `(array-fetch* array index)`

Alias for FETCH. Returns the element of array at 0-based index. See FETCH for full documentation.

**See also:** FETCH, STORE, ARRAY-STORE*, ARRAY-LENGTH*

---

### STORE

**Type:** `FUNCTION`

**Syntax:** `(store array index value)`

Destructively sets the element of array at 0-based index to value. Returns the stored value. Signals an error if index is out of bounds. Lisp 1.5 Appendix A name; ARRAY-STORE* is the longer alias, ASET the Common-Lisp-style one. Mutation is in-place: all references to the same array see the change, including inside a defun-typed body (issue #216). Two scoped exceptions: an array nested inside another array or a struct does not write back through the outer object (only top-level flat arrays of scalars do); and passing the same array as two distinct arguments to one defun-typed call is last-writer-wins in argument order, not simultaneous true aliasing.

**Examples:**
```lisp
(LET ((A (ARRAY 3))) (STORE A 0 99) (FETCH A 0))  ; => 99
```

**See also:** ARRAY-STORE*, ASET, FETCH, ARRAY, ARRAY-LENGTH*

---

### ARRAY-STORE*

**Type:** `FUNCTION`

**Syntax:** `(array-store* array index value)`

Alias for STORE. Destructively sets the element at index. See STORE for full documentation.

**See also:** STORE, FETCH, ARRAY-FETCH*, ARRAY-LENGTH*

---

### ARRAY-LENGTH*

**Type:** `FUNCTION`

**Syntax:** `(array-length* array)`

Returns the number of elements in array as an integer. The valid index range is 0 to (array-length* array) - 1.

**Examples:**
```lisp
(ARRAY-LENGTH* (ARRAY 5))  ; => 5
(ARRAY-LENGTH* (ARRAY 0))  ; => 0
```

**See also:** ARRAY, FETCH, STORE, ARRAYP

---

### ARRAYP

**Type:** `FUNCTION`

**Syntax:** `(arrayp x)`

Returns T if x is an array (created with ARRAY or MAKE-ARRAY); returns NIL otherwise. DEFSTRUCT instances are also arrays internally.

**Examples:**
```lisp
(ARRAYP (ARRAY 3))  ; => T
(ARRAYP (QUOTE (1 2 3)))  ; => ()
```

**See also:** ARRAY, ARRAY-LENGTH*, EXTENSION-P

---

### TYPED-ARRAY

**Type:** `FUNCTION`

**Syntax:** `(typed-array n elem-type)`

Creates a flat, zero-initialised typed array of n elements whose element type is 'int64 or 'float64. Unlike a plain ARRAY (a vector of boxed LispVals), a typed array is a raw u64 buffer laid out exactly like the typed JIT's own array buffers, so passing it to a typed function whose parameter's element type matches crosses the native membrane by pointer with no copy in or out — the callee's in-place STORE/ASET mutations are visible to the caller. FETCH/STORE/AREF/ASET/ARRAY-LENGTH* work on it the same as on a plain array. n is capped at 16M elements; elem-type must be the symbol 'int64 or 'float64.

**Arguments:**
- `N` - A non-negative element count
- `ELEM-TYPE` - The symbol 'int64 or 'float64

**Returns:** A new typed array

**Examples:**
```lisp
(LET ((A (TYPED-ARRAY 3 (QUOTE INT64)))) (STORE A 0 7) (FETCH A 0))  ; => 7
```

**See also:** TYPED-ARRAY-P, ARRAY, FETCH, STORE, ARRAY-LENGTH*

---

### TYPED-ARRAY-P

**Type:** `FUNCTION`

**Syntax:** `(typed-array-p x)`

Returns T if x is a typed array (created with TYPED-ARRAY); NIL otherwise. Note ARRAYP is also T for a typed array, since it is an array; TYPED-ARRAY-P is the narrow test for the flat-buffer representation specifically.

**Examples:**
```lisp
(TYPED-ARRAY-P (TYPED-ARRAY 3 (QUOTE FLOAT64)))  ; => T
(TYPED-ARRAY-P (ARRAY 3))  ; => ()
```

**See also:** TYPED-ARRAY, ARRAYP

---

# BITWISE Functions

Bitwise operations

---

### LOGAND

**Type:** `FUNCTION`

**Syntax:** `(logand integer...)`

Bitwise AND of all arguments.

**Examples:**
```lisp
(LOGAND 5 3)  ; => 1
```

**See also:** LOGOR, LOGXOR, LOGNOT

---

### LOGXOR

**Type:** `FUNCTION`

**Syntax:** `(logxor integer...)`

Bitwise XOR of all arguments.

**Examples:**
```lisp
(LOGXOR 5 3)  ; => 6
```

**See also:** LOGOR, LOGAND, LOGNOT

---

### LOGNOT

**Type:** `FUNCTION`

**Syntax:** `(lognot integer)`

Bitwise complement (NOT).

**Examples:**
```lisp
(LOGNOT 0)  ; => -1
```

**See also:** LOGOR, LOGAND, LOGXOR

---

### LEFTSHIFT

**Type:** `FUNCTION`

**Syntax:** `(leftshift n count)`

Shifts bits left (positive count) or right (negative count).

**Examples:**
```lisp
(LEFTSHIFT 1 3)  ; => 8
(LEFTSHIFT 8 -2)  ; => 2
```

**See also:** ASH, LOGOR, LOGAND

---

### ASH

**Type:** `FUNCTION`

**Syntax:** `(ash n count)`

Arithmetic shift. Shifts n left by count bits (right when count is negative).
Left shifts of 64 or more bits return 0 and set the OVERFLOW flag.
Right shifts of 64 or more bits return 0 or -1 (sign extension).
Both n and count must be integers.

**Examples:**
```lisp
(ASH 1 4)  ; => 16
(ASH 16 -2)  ; => 4
(ASH -1 -1)  ; => -1
```

**See also:** LEFTSHIFT, ROT, LOGOR, LOGAND, LOGNOT, LOGXOR

---

### ROT

**Type:** `FUNCTION`

**Syntax:** `(rot n count)`

Rotate bits of n left by count positions (64-bit rotation).
count is reduced modulo 64, so (rot n 64) equals (rot n 0).
Both n and count must be integers.

**Examples:**
```lisp
(ROT 1 1)  ; => 2
(ROT 1 63)  ; => "most-significant bit set"
```

**See also:** ASH, LOGOR, LOGAND, LOGNOT

---

# HASH-TABLES Functions

Hash tables

---

### MAKE-HASH-TABLE

**Type:** `FUNCTION`

**Syntax:** `(make-hash-table)`

Creates and returns a new empty hash table.

**See also:** GETHASH, SET-BANG, SETHASH, KEYS

---

### GETHASH

**Type:** `FUNCTION`

**Syntax:** `(gethash hash-table key)`

Retrieves the value associated with key in hash-table. Returns NIL if the key is not present. Keys are compared by structural equality (like EQUAL). Use GET for property list lookup.

**Examples:**
```lisp
(LET ((H (MAKE-HASH-TABLE))) (SET-BANG H (QUOTE X) 42) (GETHASH H (QUOTE X)))  ; => 42
```

**See also:** SET-BANG, SETHASH, KEYS, DELETE-KEY, DELETE-KEY-BANG, MAKE-HASH-TABLE, GET

---

### SET-BANG

**Type:** `FUNCTION`

**Syntax:** `(set-bang hash-table key value)`

Sets the value for key in hash-table. SETHASH is accepted as a compatibility alias.

**See also:** GETHASH, SETHASH, REMHASH, MAKE-HASH-TABLE

---

### SETHASH

**Type:** `FUNCTION`

**Syntax:** `(sethash hash-table key value)`

Compatibility alias for SET-BANG. Sets the value for key in hash-table and returns T.

**Examples:**
```lisp
(LET ((H (MAKE-HASH-TABLE))) (SETHASH H (QUOTE X) 42) (GETHASH H (QUOTE X)))  ; => 42
```

**See also:** SET-BANG, GETHASH, DELETE-KEY, MAKE-HASH-TABLE

---

### KEYS

**Type:** `FUNCTION`

**Syntax:** `(keys hash-table)`

Returns a list of all keys in hash-table.

**See also:** GETHASH, SET-BANG, MAKE-HASH-TABLE

---

### DELETE-KEY

**Type:** `FUNCTION`

**Syntax:** `(delete-key hash-table key)`

Compatibility alias for DELETE-KEY-BANG. Destructively removes key and its associated value from hash-table. Returns T regardless of whether the key was present.

**Examples:**
```lisp
(LET ((H (MAKE-HASH-TABLE))) (SET-BANG H (QUOTE X) 1) (DELETE-KEY H (QUOTE X)) (GETHASH H (QUOTE X)))  ; => ()
```

**See also:** DELETE-KEY-BANG, SET-BANG, GETHASH, KEYS, MAKE-HASH-TABLE

---

### DELETE-KEY-BANG

**Type:** `FUNCTION`

**Syntax:** `(delete-key-bang hash-table key)`

Destructively removes key and its associated value from hash-table. Returns T regardless of whether the key was present. The bang suffix signals mutation in place.

**Examples:**
```lisp
(LET ((H (MAKE-HASH-TABLE))) (SET-BANG H (QUOTE X) 1) (DELETE-KEY-BANG H (QUOTE X)) (GETHASH H (QUOTE X)))  ; => ()
```

**See also:** DELETE-KEY, SET-BANG, GETHASH, KEYS, MAKE-HASH-TABLE

---

# PLISTS Functions

Property lists

---

### GETP

**Type:** `FUNCTION`

**Syntax:** `(getp symbol indicator)`

Retrieves a property from a symbol's property list.

**See also:** PUTP, REMPROP, PLIST

---

### PUTP

**Type:** `FUNCTION`

**Syntax:** `(putp symbol indicator value)`

Sets a property on a symbol's property list.

**See also:** GETP, REMPROP, PLIST

---

### PLIST

**Type:** `FUNCTION`

**Syntax:** `(plist symbol)`

Returns the entire property list of a symbol.

**See also:** GETP, PUTP

---

### REMPROP

**Type:** `FUNCTION`

**Syntax:** `(remprop symbol indicator)`

Removes the property named indicator from symbol's property list. Returns T if the property was present and removed; returns NIL if it was not found. The indicator may be a symbol or string.

**Examples:**
```lisp
(PUTP (QUOTE X) (QUOTE COLOR) (QUOTE RED))  ; => RED
(REMPROP (QUOTE X) (QUOTE COLOR))  ; => T
```

**See also:** PUTP, GETP, PLIST, DEFLIST

---

### DOCUMENTATION

**Type:** `FUNCTION`

**Syntax:** `(documentation symbol)`

Returns the docstring for a symbol.

**See also:** GETP, HELP

---

### GET

**Type:** `FUNCTION`

**Syntax:** `(get symbol indicator)`

Retrieves a property from symbol's property list. Classic Lisp 1.5 name for GETP. Returns NIL if the indicator is not found.

**Examples:**
```lisp
(GET (QUOTE FOO) (QUOTE DOCSTRING))  ; => ()
```

**See also:** GETP, PUTP, PLIST, REMPROP

---

### DEFLIST

**Type:** `FUNCTION`

**Syntax:** `(deflist pairs indicator)`

Bulk property setter: for each pair (symbol value) in pairs, sets the property named indicator on symbol to value. A compact Lisp 1.5 idiom for initializing a property across many symbols at once.

**Examples:**
```lisp
(DEFLIST (QUOTE ((X 1) (Y 2) (Z 3))) (QUOTE INDEX))  ; => T
```

**See also:** PUTP, GETP, PLIST, REMPROP

---

# META Functions

Metaprogramming

---

### EVAL

**Type:** `FUNCTION`

**Syntax:** `(eval expression)`

Evaluates an expression.

**Examples:**
```lisp
(EVAL (QUOTE (+ 1 2)))  ; => 3
```

**See also:** APPLY, FUNCALL, QUOTE

---

### APPLY

**Type:** `FUNCTION`

**Syntax:** `(apply function args)`

Applies function to a list of arguments.

**Examples:**
```lisp
(APPLY (QUOTE +) (QUOTE (1 2 3)))  ; => 6
```

**See also:** EVAL, FUNCALL, MAPCAR

---

### FUNCALL

**Type:** `FUNCTION`

**Syntax:** `(funcall function arg...)`

Calls function with the given arguments.

**Examples:**
```lisp
(FUNCALL (QUOTE +) 1 2 3)  ; => 6
```

**See also:** APPLY, EVAL

---

### HELP

**Type:** `FUNCTION`

**Syntax:** `(help) or (help 'symbol) or (help :categories)`

Interactive help system. Use (help) for overview, (help 'symbol) for specific help.

**See also:** DOCUMENTATION, APROPOS

---

### DOCUMENTATION

**Type:** `FUNCTION`

**Syntax:** `(documentation symbol)`

Returns the docstring for a symbol.

**See also:** GETP, HELP

---

### EVLIS

**Type:** `FUNCTION`

**Syntax:** `(evlis list) or (evlis list environment)`

Evaluates each element of list in order and returns a new list of results. With one argument, uses the current environment. With two arguments, evaluates in the given environment object. This is the classic Lisp 1.5 primitive for evaluating argument lists; it is exposed for metaprogramming — most code uses MAPCAR or ordinary function calls instead.

**Examples:**
```lisp
(EVLIS (QUOTE ((+ 1 2) (* 3 4))))  ; => (3 12)
```

**See also:** EVAL, EVCON, APPLY, MAPCAR, THE-ENVIRONMENT

---

### EVCON

**Type:** `FUNCTION`

**Syntax:** `(evcon clauses) or (evcon clauses environment)`

Classic Lisp 1.5 evaluator for COND-style clauses. Walks the list of (test value) pairs, evaluates each test in turn, and returns the evaluated value of the first clause whose test is non-NIL. Returns NIL if no test passes. With two arguments, evaluates in the given environment object. Exposed for metaprogramming; prefer COND in ordinary code.

**Examples:**
```lisp
(EVCON (QUOTE (((= 1 2) "no") ((= 1 1) "yes"))))  ; => "yes"
```

**See also:** COND, EVAL, EVLIS, THE-ENVIRONMENT

---

### OPTIMIZE

**Type:** `FUNCTION`

**Syntax:** `(optimize form)`

Runs the source-level optimizer on form and returns the optimized Lisp expression without evaluating it. The optimizer performs constant folding, dead binding elimination, and other algebraic simplifications. The result is a structurally equivalent but potentially faster form. Used by the REPL and compiler pipeline; also useful for inspecting optimizer output during development.

**Examples:**
```lisp
(OPTIMIZE (QUOTE (+ 1 2)))  ; => 3
(OPTIMIZE (QUOTE (LET ((X 1)) X)))  ; => 1
```

**See also:** EVAL, MACROEXPAND, DEFUN-TYPED-OPT

---

### DEFUN-TYPED-OPT

**Type:** `VAU`

**Syntax:** `(defun-typed-opt (name return-type) ((arg type) ...) body...)`

Optimizer-to-compiler bridge for typed functions. Receives a DEFUN-TYPED-shaped definition as source, runs the Lisp/vau source optimizer over it, then evaluates the optimized DEFUN-TYPED form so the normal HM checker and native compiler install the typed edition. Use this when you want explicit source optimization before typed compilation without making every DEFUN-TYPED globally auto-optimized.

**Examples:**
```lisp
(DEFUN-TYPED-OPT (INC INT64) ((X INT64)) (+ X 0))  ; => INC
```

**See also:** OPTIMIZE, DEFUN-TYPED, CHECK-TYPE, DISASSEMBLE

---

### MACROEXPAND

**Type:** `FUNCTION`

**Syntax:** `(macroexpand form)`

Expands a macro call one level. If form is a list whose car names a macro,
returns the fully expanded form. If form is not a macro call, returns it unchanged.
Useful for debugging macro definitions.

**Examples:**
```lisp
(DEFMACRO INC (X) (QUASIQUOTE (+ (UNQUOTE X) 1)))  ; => INC
(MACROEXPAND (QUOTE (INC 5)))  ; => (+ 5 1)
```

**See also:** DEFMACRO, MACROP, EVLIS

---

# ERRORS Functions

Error handling

---

### ERROR

**Type:** `FUNCTION`

**Syntax:** `(error message)`

Raises an error with the given message.

**See also:** ERRORSET

---

### ERRORSET

**Type:** `FUNCTION`

**Syntax:** `(errorset form)`

Evaluates form, catching errors. Returns (result) on success, NIL on error.

**Examples:**
```lisp
(ERRORSET (QUOTE (+ 1 2)))  ; => (3)
(ERRORSET (QUOTE (/ 1 0)))  ; => ()
```

**See also:** ERROR

---

### MAKE-ERROR

**Type:** `FUNCTION`

**Syntax:** `(make-error message) or (make-error message data)`

Creates an error condition value with the given message string and optional data (any Lisp value). Error values are first-class: they can be stored, passed around, and inspected without being signalled. Use ERROR to signal an error that terminates the current computation. Use HANDLER-CASE or ERRORSET to catch signalled errors.

**Examples:**
```lisp
(LET ((E (MAKE-ERROR "oops"))) (ERROR-MESSAGE E))  ; => "oops"
(LET ((E (MAKE-ERROR "oops" (QUOTE (1 2))))) (ERROR-DATA E))  ; => (1 2)
```

**See also:** ERROR, ERROR-P, ERROR-MESSAGE, ERROR-DATA, ERRORSET

---

### ERROR-P

**Type:** `FUNCTION`

**Syntax:** `(error-p x)`

Returns T if x is an error condition value (created with MAKE-ERROR or captured by ERRORSET). Returns NIL for any other value including ordinary NIL. Useful for dispatching on values that might be errors.

**Examples:**
```lisp
(ERROR-P (MAKE-ERROR "oops"))  ; => T
(ERROR-P 42)  ; => ()
```

**See also:** MAKE-ERROR, ERROR-MESSAGE, ERROR-DATA, ERRORSET

---

### ERROR-MESSAGE

**Type:** `FUNCTION`

**Syntax:** `(error-message error-val)`

Extracts the message string from an error condition value. Signals an error if the argument is not an error value. Use ERROR-P to test first.

**Examples:**
```lisp
(ERROR-MESSAGE (MAKE-ERROR "bad thing"))  ; => "bad thing"
```

**See also:** ERROR-P, ERROR-DATA, MAKE-ERROR

---

### ERROR-DATA

**Type:** `FUNCTION`

**Syntax:** `(error-data error-val)`

Extracts the associated data from an error condition value. Returns NIL if no data was attached (i.e. MAKE-ERROR was called with only a message). Signals an error if the argument is not an error value.

**Examples:**
```lisp
(ERROR-DATA (MAKE-ERROR "x" (QUOTE (A B C))))  ; => (A B C)
(ERROR-DATA (MAKE-ERROR "x"))  ; => ()
```

**See also:** ERROR-P, ERROR-MESSAGE, MAKE-ERROR

---

# IO Functions

Input/Output

---

### PRINT

**Type:** `FUNCTION`

**Syntax:** `(print object...)`

Prints objects to standard output.

**Returns:** NIL

**See also:** PRIN1, PRINC, TERPRI

---

### PRIN1

**Type:** `FUNCTION`

**Syntax:** `(prin1 object)`

Prints object in readable form (strings with quotes).

**Returns:** The object printed

**Examples:**
```lisp
(PRIN1 HELLO)  ; => HELLO
```

**See also:** PRINC, PRINT

---

### PRINC

**Type:** `FUNCTION`

**Syntax:** `(princ object)`

Prints object without escaping (strings without quotes).

**Returns:** The object printed

**See also:** PRIN1, PRINT

---

### TERPRI

**Type:** `FUNCTION`

**Syntax:** `(terpri)`

Prints a newline character.

**Returns:** NIL

**See also:** PRINT, PRINC

---

### READ

**Type:** `FUNCTION`

**Syntax:** `(read)`

Reads one S-expression from standard input.

**Returns:** Parsed S-expression

**See also:** EVAL, LOAD-FILE

---

### LOAD-FILE

**Type:** `FUNCTION`

**Syntax:** `(load-file filename)`

Loads and evaluates a Lisp source file. A loaded source file may include another file with a top-level (include "path.lisp") directive; relative include paths resolve from the file that contains the include.

**Arguments:**
- `FILENAME` - String path to file

**Returns:** T on success

**See also:** READ, EVAL

---

### SPACES

**Type:** `FUNCTION`

**Syntax:** `(spaces n)`

Prints n space characters to standard output without a trailing newline. Lisp 1.5 I/O primitive for column-aligned output.

**Examples:**
```lisp
(SPACES 3)  ; => "   "
```

**See also:** TERPRI, PRINT, PRINC

---

### FORMAT

**Type:** `FUNCTION`

**Syntax:** `(format dest ctrl &rest args)`

CL-style format string rendering (issue #150, lib/18-format.lisp). DEST nil returns the formatted string; t prints it to stdout and returns nil; a PORTS port writes the UTF-8 bytes to it and returns nil. Directives: ~a ~s ~d ~f ~x ~o ~b ~c ~% ~& ~~ ~{...~} ~^ -- an unrecognized directive, or a supported one with an unsupported numeric/colon/at-sign prefix, is a hard error rather than a silent pass-through. See docs/cl-divergences.md and lib/18-format.lisp's header for exact semantics.

**Arguments:**
- `DEST` - NIL (string), T (stdout), or a PORTS port
- `CTRL` - The control string
- `ARGS` - Zero or more arguments consumed by the control string's directives

**Returns:** The formatted string (DEST nil) or NIL (DEST t or a port)

**Examples:**
```lisp
(FORMAT () "~a + ~a = ~a" 2 3 5)  ; => "2 + 3 = 5"
(FORMAT () "~,4f" 3.14159)  ; => "3.1416"
(FORMAT () "~{~a~^, ~}" (1 2 3))  ; => "1, 2, 3"
```

**See also:** PRIN1-TO-STRING, PRINC-TO-STRING, PORTS:WRITE-STRING!

---

### READ-LINE

**Type:** `FUNCTION`

**Syntax:** `(read-line &optional port)`

Reads one line of text (bytes up to but excluding a trailing newline, decoded as UTF-8 lossy) from PORT, or from the process's standard input if PORT is not given (which requires the IO capability). Returns NIL only at true EOF. Thin sugar over PORTS:READ-LINE! (lib/18-format.lisp), lazily requiring the PORTS module on first use.

**Arguments:**
- `PORT` - Optional PORTS port; defaults to (ports:stdin)

**Returns:** A STRING, or NIL at true EOF

**See also:** PORTS:READ-LINE!, PORTS:STDIN, WITH-OUTPUT-TO-STRING

---

### WITH-OUTPUT-TO-STRING

**Type:** `MACRO`

**Syntax:** `(with-output-to-string (var) body...)`

Binds VAR to a fresh in-memory output port for BODY's dynamic extent (write to it with ports:write-string!, ports:write-byte!/write-bytes!, or format with VAR as the destination) and returns everything written to it, decoded as UTF-8 (lossy), as a STRING. The port is always closed afterward; if BODY signals an error, that error propagates (no string is returned) and the port is still closed. Lazily requires the PORTS module on first use.

**Arguments:**
- `BINDING` - A one-element list (var)
- `BODY` - Forms writing to VAR

**Returns:** The captured STRING

**Examples:**
```lisp
(WITH-OUTPUT-TO-STRING (S) (PORTS:WRITE-STRING! S "hi"))  ; => "hi"
```

**See also:** READ-LINE, PORTS:OPEN-OUTPUT-BYTES, PORTS:OUTPUT-CONTENTS

---

# SPECIAL-FORMS Functions

Special forms and macros

---

### QUOTE

**Type:** `SPECIAL-FORM`

**Syntax:** `(quote expression) or 'expression`

Prevents evaluation and returns expression as data.

**Examples:**
```lisp
(QUOTE (+ 1 2))  ; => (+ 1 2)
(QUOTE FOO)  ; => FOO
```

**See also:** QUASIQUOTE, EVAL

---

### IF

**Type:** `SPECIAL-FORM`

**Syntax:** `(if condition then-form else-form)`

Evaluates condition; if non-NIL, evaluates then-form, otherwise else-form.

**Examples:**
```lisp
(IF T "yes" "no")  ; => "yes"
(IF () "yes" "no")  ; => "no"
```

**See also:** COND, AND, OR

---

### COND

**Type:** `SPECIAL-FORM`

**Syntax:** `(cond (test form...)...)`

Multi-way conditional. Evaluates tests until one is true, then evaluates its forms.

**Examples:**
```lisp
(COND ((= 1 2) "a") (T "b"))  ; => "b"
```

**See also:** IF, AND, OR

---

### AND

**Type:** `SPECIAL-FORM`

**Syntax:** `(and form...)`

Short-circuit AND. Returns first NIL or last value.

**Examples:**
```lisp
(AND T T T)  ; => T
(AND T () T)  ; => ()
(AND 1 2 3)  ; => 3
```

**See also:** OR, NOT, IF

---

### OR

**Type:** `SPECIAL-FORM`

**Syntax:** `(or form...)`

Short-circuit OR. Returns first non-NIL value or NIL.

**Examples:**
```lisp
(OR () () T)  ; => T
(OR 1 2 3)  ; => 1
```

**See also:** AND, NOT, IF

---

### DEF

**Type:** `SPECIAL-FORM`

**Syntax:** `(def symbol value &optional docstring)`

Binds symbol to value in the current environment.

**Examples:**
```lisp
(DEF X 42)  ; => X
```

**See also:** SETQ, LET, DEFUN

---

### SETQ

**Type:** `SPECIAL-FORM`

**Syntax:** `(setq symbol value)`

Assigns a new value to an existing variable.

**See also:** DEF, LET

---

### LET

**Type:** `SPECIAL-FORM`

**Syntax:** `(let ((var val)...) body...)`

Creates local variable bindings for the duration of body.

**Examples:**
```lisp
(LET ((X 1) (Y 2)) (+ X Y))  ; => 3
```

**See also:** DEF, LAMBDA, PROG

---

### LAMBDA

**Type:** `SPECIAL-FORM`

**Syntax:** `(lambda (params...) body...)`

Creates an anonymous function (closure).

**Examples:**
```lisp
((LAMBDA (X) (* X X)) 5)  ; => 25
```

**See also:** DEFUN, FUNCTION, APPLY

---

### DEFUN

**Type:** `MACRO`

**Syntax:** `(defun name (params...) &optional docstring body...)`

Defines a named function with optional docstring.

**See also:** LAMBDA, DEF, DEFMACRO

---

### DEFUN*

**Type:** `VAU`

**Syntax:** `(defun* name [docstring] params... [return-type] body...)`

Recommended default function definition form. Tries HM type inference automatically and compiles a native typed edition when the body is a fully-inferable typed island; otherwise falls back transparently to an ordinary lambda. Params may be classic ((a b)), flat bare (a b), or typed ((x int64)); an optional bare type keyword after the params pins the return type, and any unspecified type is inferred. Emits a note on stderr when types were inferred and compiled.

**Examples:**
```lisp
(DEFUN* SQ (X) (* X X))  ; => SQ
(DEFUN* ADD (X INT64) (Y INT64) (+ X Y))  ; => ADD
```

**See also:** DEFUN, DEFUN-TYPED, DEFUN-TYPED-OPT, CHECK-TYPE, LAMBDA, SIGNATURE, COMPILED-P, WHY-NOT-TYPED

---

### DEFMACRO

**Type:** `SPECIAL-FORM`

**Syntax:** `(defmacro name (params...) body...)`

Defines a macro that transforms code before evaluation.

**See also:** DEFUN, DEFEXPR, MACROEXPAND

---

### PROGN

**Type:** `SPECIAL-FORM`

**Syntax:** `(progn form...)`

Evaluates forms in sequence, returns last value.

**Examples:**
```lisp
(PROGN (+ 1 2) (* 3 4))  ; => 12
```

**See also:** PROG, LET

---

### PROG

**Type:** `SPECIAL-FORM`

**Syntax:** `(prog (vars...) statements...)`

Imperative block with local variables and labels for GO/RETURN.

**See also:** GO, RETURN, PROGN, LET

---

### DEFEXPR

**Type:** `SPECIAL-FORM`

**Syntax:** `(defexpr name (param...) [docstring] body)`

Defines a named FEXPR ("functional expression") — a function-like object that receives its arguments UNEVALUATED as raw list structure instead of as computed values.

A fexpr is the classic Lisp 1.5 mechanism for user-defined special forms. When a fexpr is called the evaluator does NOT evaluate the operands before passing them in; the body of the fexpr receives the literal source forms and may choose to evaluate them (with EVAL), ignore them, or inspect/transform them.

With a single parameter the entire unevaluated operand list is bound to that parameter as a Lisp list:
  (defexpr my-and (args) (cond ((null args) t) ((null (cdr args)) (eval (car args))) ...))
  (my-and (< x 5) (> x 0))  ; args = ((< x 5) (> x 0)) -- not evaluated yet

With multiple parameters each unevaluated operand is bound to the corresponding parameter individually.

Fexprs are powerful but compose poorly: because the evaluator cannot see past a fexpr call, optimisations and macro-expanders that need to walk the code tree are blocked.  Modern usage (post-1970s) generally prefers DEFMACRO for compile-time code transformation and LAMBDA for runtime abstraction.  Use fexprs when you genuinely need access to both the unevaluated source and the current environment at call time — for example, to implement a custom binding form or a quoting operator.

See also VAU/$VAU for the Kernel-language operative, which makes the caller's environment explicit.

**Examples:**
```lisp
(DEFEXPR MY-QUOTE (X) (CAR X))  ; => (MY-QUOTE FOO)
(DEFEXPR VERBOSE-IF (TEST THEN ELSE) (IF (EVAL TEST) (EVAL THEN) (EVAL ELSE)))  ; => (VERBOSE-IF (> 3 2) (PRINT "yes") (PRINT "no"))
```

**See also:** VAU, DEFMACRO, LAMBDA, FUNCALL, EVAL

---

### VAU

**Type:** `SPECIAL-FORM`

**Syntax:** `(vau (operands-param env-param) body...)`

Creates an anonymous VAU operative (also written $VAU following Kernel convention).  A vau operative is similar to a fexpr — it receives arguments UNEVALUATED — but it also receives the CALLER'S ENVIRONMENT as an explicit first-class value, giving the operative complete reflective access.

The parameter list must contain exactly two symbols:
  operands-param — bound to the unevaluated operand list (a Lisp list of the literal source forms)
  env-param      — bound to the caller's environment as a first-class Environment object

Inside the body you can call (eval form env-param) to evaluate any form in the caller's scope, inspect bindings via environment operations, or build derived control structures.

VAU operatives originate in John Shutt's Kernel language (dissertation, 2010).  The key insight is that the combination of (1) receiving operands unevaluated and (2) having the caller's environment as an explicit object is strictly more general than either macros or fexprs alone.  From VAU you can *derive* both LAMBDA (wrap in an evaluating shell) and DEFMACRO (evaluate operands, produce code, evaluate result in caller's env).  This makes VAU the minimal kernel for a reflective Lisp.

Unlike DEFEXPR fexprs, vau operatives do not capture a dynamic parent environment for argument evaluation — the caller's environment is passed explicitly, making the data flow transparent to analysis tools.

In Lamedh the $VAU alias is also recognised (the dollar sign is idiomatic Kernel notation for operatives that receive unevaluated operands).

**Examples:**
```lisp
(DEF $MY-IF ($VAU (TEST THEN ELSE) E (IF (EVAL TEST E) (EVAL THEN E) (EVAL ELSE E))))  ; => ($MY-IF (> 3 2) (QUOTE YES) (QUOTE NO))
(DEF $SEQ ($VAU (FORMS) E (IF (NULL FORMS) () (IF (NULL (CDR FORMS)) (EVAL (CAR FORMS) E) (PROGN (EVAL (CAR FORMS) E) (EVAL (CONS (QUOTE $SEQ) (CDR FORMS)) E))))))  ; => ($SEQ (PRINT "a") (PRINT "b"))
```

**See also:** DEFEXPR, DEFMACRO, LAMBDA, EVAL, THE-ENVIRONMENT, MAKE-ENVIRONMENT

---

### MACRO

**Type:** `SPECIAL-FORM`

**Syntax:** `(macro (params...) body...)`

Anonymous macro constructor: evaluates to a macro VALUE (the macro counterpart of LAMBDA). Because operator dispatch resolves the head symbol through the lexical environment, a name locally bound to a macro value is used as an operator in that scope. Backs MACROLET.

**Examples:**
```lisp
(LET ((SQ (MACRO (X) (LIST (QUOTE *) X X)))) (SQ 6))  ; => 36
```

**See also:** LAMBDA, FEXPR, VAU, DEFMACRO, MACROLET

---

### FEXPR

**Type:** `SPECIAL-FORM`

**Syntax:** `(fexpr (params...) body...)`

Anonymous fexpr constructor: evaluates to a fexpr VALUE whose operands reach the body unevaluated (the fexpr counterpart of LAMBDA). Backs FEXPRLET.

**Examples:**
```lisp
(LET ((Q (FEXPR (A) (CAR A)))) (Q (+ 1 2)))  ; => (+ 1 2)
```

**See also:** LAMBDA, MACRO, VAU, DEFEXPR, FEXPRLET

---

### FLET

**Type:** `MACRO`

**Syntax:** `(flet ((name (params...) body...) ...) body...)`

Locally bind named functions (non-recursive) for the extent of the body. Parallel LET semantics: clauses do not see one another. A local binding shadows a global operator of the same name only within the body.

**Examples:**
```lisp
(FLET ((SQ (X) (* X X))) (SQ 7))  ; => 49
```

**See also:** LET, LAMBDA, MACROLET, FEXPRLET, VAULET

---

### MACROLET

**Type:** `MACRO`

**Syntax:** `(macrolet ((name (params...) body...) ...) body...)`

Locally bind macros for the extent of the body. Each clause is expanded at call sites like a DEFMACRO definition. Parallel LET semantics: clauses do not see one another.

**Examples:**
```lisp
(MACROLET ((TWICE (E) (LIST (QUOTE PROGN) E E))) (TWICE 1))  ; => 1
```

**See also:** MACRO, DEFMACRO, FLET, FEXPRLET, VAULET

---

### FEXPRLET

**Type:** `MACRO`

**Syntax:** `(fexprlet ((name (params...) body...) ...) body...)`

Locally bind fexprs (unevaluated-argument operatives) for the extent of the body. Operands reach the body unevaluated, as with DEFEXPR. Parallel LET semantics.

**Examples:**
```lisp
(FEXPRLET ((Q (A) (CAR A))) (Q (+ 1 2)))  ; => (+ 1 2)
```

**See also:** FEXPR, DEFEXPR, FLET, MACROLET, VAULET

---

### VAULET

**Type:** `MACRO`

**Syntax:** `(vaulet ((name (operands env) body...) ...) body...)`

Locally bind vau operatives for the extent of the body. Each clause's OPERANDS receives the unevaluated operand list and ENV the caller's environment, as with VAU. Parallel LET semantics.

**See also:** VAU, $VAU, FLET, MACROLET, FEXPRLET

---

# OS-LINUX Functions

Typed Linux file metadata and symlinks (OS-LINUX module, lib/42-os-linux.lisp, issue #260); READ-FS capability

---

### OS-LINUX:STAT

**Type:** `FUNCTION`

**Syntax:** `(os-linux:stat path)`

path's metadata, following a trailing symlink (like stat(2)), as a typed alist: :size :mode :uid :gid :nlink :ino :dev :mtime :atime :ctime :is-dir :is-file :is-symlink -- never a raw C struct. Requires READ-FS.

**See also:** OS-LINUX:LSTAT, OS-LINUX:STAT-SIZE, OS-LINUX:READLINK

---

### OS-LINUX:LSTAT

**Type:** `FUNCTION`

**Syntax:** `(os-linux:lstat path)`

Like os-linux:stat, but does not follow a trailing symlink (like lstat(2)) -- if path itself is a symlink, describes the symlink, and :is-symlink is T.

**See also:** OS-LINUX:STAT, OS-LINUX:READLINK

---

### OS-LINUX:READLINK

**Type:** `FUNCTION`

**Syntax:** `(os-linux:readlink path)`

The target path points to, as a string, if path is a symlink. Requires READ-FS. Signals a structured :INVALID-ARGUMENT error if path is not a symlink.

**See also:** OS-LINUX:LSTAT

---

# OS Functions

Process identity/environment, time, randomness, and process spawn/control (OS module, lib/41-os.lisp, issue #260); OS-ENV/OS-ENV-WRITE/OS-PROCESS/OS-SIGNAL capabilities

---

### OS:ARGS

**Type:** `FUNCTION`

**Syntax:** `(os:args)`

The process's raw argv (including argv[0]) as a list of strings. Requires OS-ENV. Distinct from *ARGV*, which the CLI binds to only the arguments after a script path.

**See also:** OS:EXECUTABLE-PATH

---

### OS:EXECUTABLE-PATH

**Type:** `FUNCTION`

**Syntax:** `(os:executable-path)`

The absolute path to the currently running executable. Requires OS-ENV.

**See also:** OS:ARGS

---

### OS:CWD

**Type:** `FUNCTION`

**Syntax:** `(os:cwd)`

The process's current working directory, as a string. Requires OS-ENV.

**See also:** OS:CHDIR!

---

### OS:CHDIR!

**Type:** `FUNCTION`

**Syntax:** `(os:chdir! path)`

Changes the process's current working directory to path (process-wide; affects every subsequent relative-path filesystem operation). Requires OS-ENV-WRITE.

**See also:** OS:CWD

---

### OS:ENV-GET

**Type:** `FUNCTION`

**Syntax:** `(os:env-get name)`

The value of environment variable name, or NIL if unset. Requires OS-ENV.

**See also:** OS:ENV-LIST, OS:ENV-SET!, OS:ENV-UNSET!

---

### OS:ENV-LIST

**Type:** `FUNCTION`

**Syntax:** `(os:env-list)`

Every environment variable as an alist of (name . value) strings, sorted by name. Requires OS-ENV.

**See also:** OS:ENV-GET

---

### OS:ENV-SET!

**Type:** `FUNCTION`

**Syntax:** `(os:env-set! name value)`

Sets environment variable name to value (process-wide). Requires OS-ENV-WRITE.

**See also:** OS:ENV-GET, OS:ENV-UNSET!

---

### OS:ENV-UNSET!

**Type:** `FUNCTION`

**Syntax:** `(os:env-unset! name)`

Removes environment variable name (process-wide); a no-op if already unset. Requires OS-ENV-WRITE.

**See also:** OS:ENV-SET!

---

### OS:PID

**Type:** `FUNCTION`

**Syntax:** `(os:pid)`

This process's OS process ID. Requires OS-ENV.

**See also:** OS:PPID, OS:PROCESS-ID

---

### OS:PPID

**Type:** `FUNCTION`

**Syntax:** `(os:ppid)`

This process's parent process ID. Requires OS-ENV. Linux-only (std has no portable getppid()); signals a structured :UNSUPPORTED-PLATFORM error elsewhere.

**See also:** OS:PID

---

### OS:HOSTNAME

**Type:** `FUNCTION`

**Syntax:** `(os:hostname)`

This host's hostname. Requires OS-ENV. Linux-only; signals a structured :UNSUPPORTED-PLATFORM error elsewhere.

**See also:** OS:PID

---

### OS:NOW

**Type:** `FUNCTION`

**Syntax:** `(os:now)`

Current wall-clock time since the Unix epoch, as (CONS seconds nanoseconds). No capability required.

**See also:** OS:NOW-UNIX, OS:MONOTONIC-NANOS

---

### OS:NOW-UNIX

**Type:** `FUNCTION`

**Syntax:** `(os:now-unix)`

Current wall-clock time since the Unix epoch as a single float number of seconds. No capability required.

**See also:** OS:NOW

---

### OS:MONOTONIC-NANOS

**Type:** `FUNCTION`

**Syntax:** `(os:monotonic-nanos)`

Nanoseconds elapsed since an arbitrary, process-local, never-goes-backward reference point -- not comparable across processes or with os:now. No capability required.

**See also:** OS:ELAPSED-SECONDS, OS:NOW

---

### OS:ELAPSED-SECONDS

**Type:** `FUNCTION`

**Syntax:** `(os:elapsed-seconds start-nanos)`

Seconds elapsed since start-nanos (a prior os:monotonic-nanos reading), as a float. No capability required.

**See also:** OS:MONOTONIC-NANOS

---

### OS:SLEEP

**Type:** `FUNCTION`

**Syntax:** `(os:sleep ms)`

Blocks the calling thread for ms milliseconds. Always sleeps for at least the requested duration; no EINTR/short-sleep behavior is observable. No capability required.

**See also:** OS:SLEEP-SECONDS

---

### OS:SLEEP-SECONDS

**Type:** `FUNCTION`

**Syntax:** `(os:sleep-seconds secs)`

Blocks the calling thread for secs seconds (a float or integer). See os:sleep.

**See also:** OS:SLEEP

---

### OS:MAKE-PRNG

**Type:** `FUNCTION`

**Syntax:** `(os:make-prng seed)`

A fresh deterministic PRNG state seeded with seed (any integer). Distinct from the global time-seeded (random n): explicitly seeded and reproducible. No capability required.

**See also:** OS:PRNG-NEXT, OS:RANDOM-DOUBLE, RANDOM

---

### OS:PRNG-NEXT

**Type:** `FUNCTION`

**Syntax:** `(os:prng-next state)`

Advances PRNG state (from os:make-prng or a prior os:prng-next) one SplitMix64 step. Returns (CONS new-state value), value a non-negative integer in [0, 2^63). Purely functional -- never mutates state in place. Deterministic: the same state always yields the same result.

**Examples:**
```lisp
(CDR (OS:PRNG-NEXT (OS:MAKE-PRNG 42)))  ; => 6839728766377637706
```

**See also:** OS:MAKE-PRNG, OS:RANDOM-DOUBLE

---

### OS:RANDOM-DOUBLE

**Type:** `FUNCTION`

**Syntax:** `(os:random-double state)`

Like os:prng-next, but value is a float in [0.0, 1.0).

**See also:** OS:PRNG-NEXT

---

### OS:RANDOM-BYTES

**Type:** `FUNCTION`

**Syntax:** `(os:random-bytes n)`

N cryptographically secure random bytes from the OS entropy source (/dev/urandom on Linux) as a fresh Array<Char>. No capability required -- a read-only entropy source, not application data. Distinct from os:make-prng/os:prng-next's deterministic, explicitly-seeded generator.

**See also:** OS:MAKE-PRNG

---

### OS:SPAWN

**Type:** `FUNCTION`

**Syntax:** `(os:spawn program &optional argv &key (inherit-env t) env cwd stdin stdout stderr)`

Spawns program (a path, never run through a shell -- argv is passed through literally, no interpolation) with argv (a list of strings). Requires OS-PROCESS. :INHERIT-ENV/:ENV control the child's environment; :CWD its working directory; :STDIN/:STDOUT/:STDERR are each NIL/:INHERIT/:NULL/:PIPE. Returns ((:handle . child) (:stdin . port-or-nil) (:stdout . port-or-nil) (:stderr . port-or-nil)).

**See also:** OS:PROCESS-HANDLE, OS:PROCESS-WAIT!, OS:PROCESS-KILL!, SHELL

---

### OS:PROCESS-HANDLE

**Type:** `FUNCTION`

**Syntax:** `(os:process-handle process)`

The OS:CHILD handle inside an os:spawn result alist. Companions: os:process-stdin, os:process-stdout, os:process-stderr (the stdio ports requested as :pipe, or NIL).

**See also:** OS:SPAWN, OS:PROCESS-WAIT!

---

### OS:PROCESS-WAIT!

**Type:** `FUNCTION`

**Syntax:** `(os:process-wait! handle)`

Blocks until the child behind handle exits, then reaps it. Returns an exit-status alist: ((:exit-code . n-or-nil) (:signal . n-or-nil) (:success . t-or-nil)). Idempotent: a second call after reaping returns the cached status. No further capability required once handle exists (OS-PROCESS covers continued use).

**See also:** OS:PROCESS-TRY-WAIT!, OS:EXIT-CODE, OS:EXIT-SUCCESS-P

---

### OS:PROCESS-TRY-WAIT!

**Type:** `FUNCTION`

**Syntax:** `(os:process-try-wait! handle)`

Non-blocking poll of handle: NIL if still running, else the same exit-status alist os:process-wait! returns (reaping the child).

**See also:** OS:PROCESS-WAIT!

---

### OS:PROCESS-ID

**Type:** `FUNCTION`

**Syntax:** `(os:process-id handle)`

handle's OS PID. Retained (not NIL) even after the process has been reaped, for diagnostics/logging.

**See also:** OS:PID, OS:PROCESS-ALIVE-P

---

### OS:PROCESS-ALIVE-P

**Type:** `FUNCTION`

**Syntax:** `(os:process-alive-p handle)`

T unless handle has been reaped (by os:process-wait!/os:process-try-wait! or the Drop backstop).

**See also:** OS:PROCESS-WAIT!

---

### OS:PROCESS-KILL!

**Type:** `FUNCTION`

**Syntax:** `(os:process-kill! handle)`

Sends SIGKILL to the child behind handle (hard, unignorable). Does NOT reap it -- call os:process-wait!/os:process-try-wait! afterward. Signals a :CLOSED error if already reaped.

**See also:** OS:PROCESS-TERMINATE!, OS:PROCESS-WAIT!

---

### OS:PROCESS-TERMINATE!

**Type:** `FUNCTION`

**Syntax:** `(os:process-terminate! handle)`

Sends SIGTERM to the child behind handle (graceful; the child may ignore or handle it). Does NOT reap it. Signals a :CLOSED error if already reaped.

**See also:** OS:PROCESS-KILL!, OS:SIGNAL!

---

### OS:PROCESS-P

**Type:** `FUNCTION`

**Syntax:** `(os:process-p x)`

T if x is an OS:CHILD handle (as returned by os:process-handle).

**See also:** OS:SPAWN

---

### OS:EXIT-CODE

**Type:** `FUNCTION`

**Syntax:** `(os:exit-code status)`

The :exit-code field of an exit-status alist, or NIL if the process was terminated by a signal. Companions: os:exit-signal, os:exit-success-p.

**See also:** OS:PROCESS-WAIT!, OS:EXIT-SUCCESS-P

---

### OS:SIGNAL!

**Type:** `FUNCTION`

**Syntax:** `(os:signal! pid signal-name)`

Sends signal-name (a typed name, e.g. :term, :kill, :hup, :int, :usr1, :usr2, :quit, :cont, :stop, :chld, :pipe, :alrm -- never a raw number) to pid (an arbitrary integer PID not necessarily owned as a handle). Requires OS-SIGNAL. Prefer os:process-kill!/os:process-terminate! for a handle you already hold -- those need only OS-PROCESS.

**See also:** OS:PROCESS-KILL!, OS:PROCESS-TERMINATE!

---

# HTTP Functions

HTTP/1.1 client and server (http:// always, https:// with the net-tls cargo feature -- issue #365)

---

### HTTP:REQUEST

**Type:** `FUNCTION`

**Syntax:** `(http:request method url &key headers body connect-timeout-ms read-timeout-ms overall-timeout-ms max-redirects follow-redirects max-line-bytes max-header-count extra-roots)`

Performs an HTTP/1.1 request. http:// always; https:// too when the net-tls cargo feature is compiled in (:extra-roots forwards to tls:connect for a private/throwaway CA) -- otherwise https:// is a structured :HTTPS-UNSUPPORTED error naming the net-tls feature (issue #365). Requires NET-CONNECT (via tcp:connect; HTTP adds no capability of its own). :HEADERS is an ordered (name . value) list (repeats preserved); :BODY is NIL, a String, an Array<Char>, or a readable PORTS port (streamed via chunked transfer-encoding). Follows 301/302/303/307/308 redirects by default, hop-capped, stripping credentials cross-origin, never crossing schemes silently. Returns a response alist whose :BODY is an UNREAD framing-aware body stream.

**See also:** HTTP:GET, HTTP:POST, HTTP:RESPONSE-STATUS, HTTP:RESPONSE-BODY, HTTP:COLLECT-STRING

---

### HTTP:GET

**Type:** `FUNCTION`

**Syntax:** `(http:get url &key headers connect-timeout-ms read-timeout-ms overall-timeout-ms max-redirects follow-redirects)`

Ergonomic (http:request "GET" url ...). See http:request for every keyword and the capability/scheme rules.

**See also:** HTTP:REQUEST, HTTP:POST, HTTP:COLLECT-STRING

---

### HTTP:POST

**Type:** `FUNCTION`

**Syntax:** `(http:post url &key body headers connect-timeout-ms read-timeout-ms overall-timeout-ms max-redirects follow-redirects)`

Ergonomic (http:request "POST" url :body body ...). See http:request for every keyword.

**See also:** HTTP:REQUEST, HTTP:GET

---

### HTTP:RESPONSE-STATUS

**Type:** `FUNCTION`

**Syntax:** `(http:response-status response)`

The integer status code of a client response alist. Companions: http:response-reason, http:response-version, http:response-headers, http:response-header (case-insensitive first-match lookup), http:response-body (the unread body stream).

**See also:** HTTP:REQUEST, HTTP:RESPONSE-BODY, MIME:HEADERS-GET

---

### HTTP:RESPONSE-BODY

**Type:** `FUNCTION`

**Syntax:** `(http:response-body response)`

The response's UNREAD body stream: framing-aware (Content-Length exact / chunked / read-to-close / no body for HEAD, 1xx, 204, 304). Read incrementally with http:stream-read!, or collect bounded with http:collect-bytes / http:collect-string / http:collect-json.

**See also:** HTTP:STREAM-READ!, HTTP:COLLECT-BYTES, HTTP:COLLECT-STRING, HTTP:COLLECT-JSON

---

### HTTP:STREAM-READ!

**Type:** `FUNCTION`

**Syntax:** `(http:stream-read! stream n)`

Reads up to N bytes from an HTTP body stream, honoring its message framing -- never reads past this message's body. Returns a fresh Array<Char>, possibly shorter than N, empty exactly at the logical end of the body (mirrors ports:read-bytes!). Companions: http:stream-eof-p, http:stream-read-all!, http:stream-close! (closes the client connection; a no-op for a server request body).

**See also:** HTTP:STREAM-EOF-P, HTTP:STREAM-CLOSE!, HTTP:COLLECT-BYTES, PORTS:READ-BYTES!

---

### HTTP:COLLECT-STRING

**Type:** `FUNCTION`

**Syntax:** `(http:collect-string stream &key max-bytes lossy)`

Collects an HTTP body stream to its end (bounded: default 10 MiB, error past :MAX-BYTES -- never unbounded buffering) and decodes it as UTF-8 into a String (:LOSSY t for replacement characters instead of a strict decode error). Companions: http:collect-bytes (raw Array<Char>), http:collect-json (parses via json:parse).

**See also:** HTTP:COLLECT-BYTES, HTTP:COLLECT-JSON, HTTP:STREAM-READ!, TEXT:UTF8->STRING

---

### HTTP:SERVE

**Type:** `FUNCTION`

**Syntax:** `(http:serve listener handler &key read-timeout-ms max-line-bytes max-header-count max-body-bytes on-error max-requests stop-p)`

Serves HTTP/1.1 on a tcp:listen listener (NET-LISTEN gates the listen; HTTP adds no capability). HANDLER: request alist -> response alist (see http:respond). Serial keep-alive: one connection served fully before the next accept (concurrency is issue #140's scope). Request line/header/body limits enforced (oversize body -> 413 without running the handler); an uncaught handler error becomes a generic 500 that never leaks the condition to the peer (:ON-ERROR receives it host-side). :STOP-P is consulted between connections; :MAX-REQUESTS bounds the connection count. http:serve-one! accepts and serves exactly one connection.

**See also:** HTTP:SERVE-ONE!, HTTP:RESPOND, TCP:LISTEN

---

### HTTP:RESPOND

**Type:** `FUNCTION`

**Syntax:** `(http:respond status &key headers body reason)`

Builds a response alist for an http:serve handler: STATUS integer, :HEADERS an ordered (name . value) list, :BODY NIL/String/Array<Char>/readable PORTS port (a port streams out chunked; Content-Length is set automatically otherwise), :REASON defaulting to http:default-reason. The handler-side request accessors are http:request-method, -target, -path, -query, -headers, -header, -body (a streaming body, Content-Length and chunked framing both), -version, and -peer-addr.

**See also:** HTTP:SERVE, HTTP:DEFAULT-REASON, HTTP:REQUEST-BODY

---

# TLS Functions

TLS client/server wrap of a connected TCP port (TLS module, lib/43-tls.lisp, issue #365); off-by-default net-tls cargo feature, rustls/ring

---

### TLS:AVAILABLE-P

**Type:** `FUNCTION`

**Syntax:** `(tls:available-p)`

T if this build of lamedh was compiled with the net-tls cargo feature (rustls); NIL otherwise. Every tls:* name is bound either way -- with the feature off, every other tls:* operation signals a structured :TLS-UNAVAILABLE error instead of doing any work.

**See also:** TLS:CONNECT, TLS:WRAP-CLIENT

---

### TLS:CONNECT

**Type:** `FUNCTION`

**Syntax:** `(tls:connect host port &key connect-timeout-ms handshake-timeout-ms alpn extra-roots)`

tcp:connect + tls:wrap-client sugar: connects to host:port then TLS-wraps the result, :hostname defaulting to host (used for SNI and certificate verification). Verification is on by default against the default (webpki-roots) root store plus :extra-roots (a list of PEM sources: String paths, READ-FS-gated, or Array<Char> bytes). :handshake-timeout-ms bounds the handshake via the underlying TCP port's read/write timeouts. Returns an ordinary PORTS port.

**See also:** TLS:WRAP-CLIENT, TLS:CONNECT-INSECURE!, TLS:ALPN-PROTOCOL, TLS:PEER-CERTIFICATES

---

### TLS:CONNECT-INSECURE!

**Type:** `FUNCTION`

**Syntax:** `(tls:connect-insecure! host port &key connect-timeout-ms handshake-timeout-ms alpn)`

Like tls:connect, but skips certificate-chain verification entirely -- the only Lisp-facing way to do so. ALWAYS signals a structured :POLICY-DENIED error unless the embedding host has separately opted in via Environment::set_allow_insecure_tls (Rust-only, default false) -- Lisp code alone can never disable verification.

**See also:** TLS:CONNECT, TLS:WRAP-CLIENT-INSECURE!

---

### TLS:WRAP-CLIENT

**Type:** `FUNCTION`

**Syntax:** `(tls:wrap-client port &key hostname alpn extra-roots)`

Wraps port -- an already-connected tcp:connect PORT -- as a TLS client, performing the handshake now (blocking). Consumes port (it becomes CLOSED); returns a new PORTS port. :hostname is required (SNI + certificate verification). See tls:connect for the connect+wrap sugar.

**See also:** TLS:CONNECT, TLS:WRAP-CLIENT-INSECURE!, TLS:WRAP-SERVER

---

### TLS:WRAP-CLIENT-INSECURE!

**Type:** `FUNCTION`

**Syntax:** `(tls:wrap-client-insecure! port &key hostname alpn)`

Like tls:wrap-client, but skips certificate verification -- denied by default (:POLICY-DENIED) unless the host opted in via Environment::set_allow_insecure_tls. See tls:connect-insecure! for the connect+wrap sugar.

**See also:** TLS:WRAP-CLIENT, TLS:CONNECT-INSECURE!

---

### TLS:WRAP-SERVER

**Type:** `FUNCTION`

**Syntax:** `(tls:wrap-server port cert key &key alpn)`

Wraps port -- an already-accepted tcp:accept PORT -- as a TLS server, performing the handshake now (blocking). Consumes port (it becomes CLOSED); returns a new PORTS port. cert/key are each a PEM source (String path, READ-FS-gated, or Array<Char> bytes); cert may be a full chain, leaf first. No client-certificate authentication is requested.

**See also:** TLS:WRAP-CLIENT, TCP:ACCEPT

---

### TLS:ALPN-PROTOCOL

**Type:** `FUNCTION`

**Syntax:** `(tls:alpn-protocol port)`

The ALPN protocol negotiated on TLS port (a String), or NIL if none was negotiated.

**See also:** TLS:CONNECT, TLS:WRAP-SERVER

---

### TLS:PEER-CERTIFICATES

**Type:** `FUNCTION`

**Syntax:** `(tls:peer-certificates port)`

The peer's certificate chain on TLS port, leaf first, as a list of Array<Char> raw DER bytes -- or NIL if none presented. No X.509 parser is part of this dependency ruling, so these are opaque DER bytes, not parsed fields; see tls:peer-certificate-summary for structural (unparsed) data.

**See also:** TLS:PEER-CERTIFICATE-SUMMARY

---

### TLS:PEER-CERTIFICATE-SUMMARY

**Type:** `FUNCTION`

**Syntax:** `(tls:peer-certificate-summary port)`

A structural summary alist for TLS port's peer certificate chain: ((:count . N) (:leaf-der-length . M) (:leaf-der . bytes)), or NIL if none presented. No parsed subject/issuer/expiry fields -- see tls:peer-certificates for the full raw DER chain.

**See also:** TLS:PEER-CERTIFICATES

---

### TLS:SNI-HOSTNAME

**Type:** `FUNCTION`

**Syntax:** `(tls:sni-hostname port)`

The SNI hostname a TLS client offered when connecting to server-side port, or NIL if none was sent (or port is a client-side TLS port).

**See also:** TLS:WRAP-SERVER

---

# UDP Functions

UDP bind/send-to/receive-from datagram sockets (UDP module, lib/39-udp.lisp, issue #258); NET-CONNECT/NET-LISTEN capabilities

---

### UDP:BIND

**Type:** `FUNCTION`

**Syntax:** `(udp:bind host port)`

Binds a UDP socket to host:port (port 0 for an OS-assigned ephemeral port). Requires the NET-LISTEN capability -- a bound socket receives datagrams from any sender, matching "binding for inbound traffic".

**See also:** UDP:CONNECT!, UDP:SEND-TO, UDP:RECEIVE-FROM

---

### UDP:CONNECT!

**Type:** `FUNCTION`

**Syntax:** `(udp:connect! socket host port)`

Sets socket's default peer to host:port so udp:send/udp:receive-from can be used without repeating the address. Requires the NET-CONNECT capability.

**See also:** UDP:SEND, UDP:BIND

---

### UDP:SEND-TO

**Type:** `FUNCTION`

**Syntax:** `(udp:send-to socket host port bytes)`

Sends bytes (an Array<Char>) as one datagram to host:port, returning the number of bytes sent. Requires the NET-CONNECT capability.

**See also:** UDP:SEND, UDP:RECEIVE-FROM

---

### UDP:SEND

**Type:** `FUNCTION`

**Syntax:** `(udp:send socket bytes)`

Sends bytes as one datagram to socket's connected peer (see udp:connect!).

**See also:** UDP:CONNECT!, UDP:SEND-TO

---

### UDP:RECEIVE-FROM

**Type:** `FUNCTION`

**Syntax:** `(udp:receive-from socket maxlen)`

Blocks for one datagram of at most maxlen bytes, returning (list bytes peer-address possibly-truncated-p). Datagram boundaries are preserved. possibly-truncated-p is T exactly when the received length equals maxlen, since plain std::net exposes no MSG_TRUNC indicator.

**See also:** UDP:BIND, UDP:SEND-TO

---

### UDP:CLOSE!

**Type:** `FUNCTION`

**Syntax:** `(udp:close! socket)`

Closes socket. Idempotent; every subsequent send/receive on socket errors immediately with a :CLOSED error.

**See also:** UDP:BIND, UDP:SOCKET-OPEN-P

---

### UDP:SOCKET-P

**Type:** `FUNCTION`

**Syntax:** `(udp:socket-p x)`

T if x is a UDP socket handle (as returned by udp:bind).

**See also:** UDP:BIND, TCP:LISTENER-P

---

### UDP:SOCKET-OPEN-P

**Type:** `FUNCTION`

**Syntax:** `(udp:socket-open-p socket)`

T unless socket has been closed.

**See also:** UDP:CLOSE!

---

### UDP:SET-TIMEOUT!

**Type:** `FUNCTION`

**Syntax:** `(udp:set-timeout! socket ms)`

Sets socket's read and write timeout in milliseconds together; NIL blocks without a timeout (the default). A timed-out receive-from/send/send-to signals a structured :TIMEOUT error.

**See also:** UDP:RECEIVE-FROM, TCP:SET-READ-TIMEOUT!

---

# TCP Functions

TCP connect/bind/listen/accept over binary ports (TCP module, lib/38-tcp.lisp, issue #258); NET-CONNECT/NET-LISTEN capabilities

---

### TCP:CONNECT

**Type:** `FUNCTION`

**Syntax:** `(tcp:connect host port) or (tcp:connect host port timeout-ms)`

Connects to host:port over TCP, returning a duplex binary PORTS port -- every PORTS operation (read-byte!, write-bytes!, close!, ...) works on it unchanged. Requires the NET-CONNECT capability. TIMEOUT-MS, if given, bounds the connect attempt; NIL (default) blocks without a timeout.

**See also:** TCP:LISTEN, TCP:ACCEPT, PORTS:READ-BYTES!, PORTS:WRITE-BYTES!

---

### TCP:LISTEN

**Type:** `FUNCTION`

**Syntax:** `(tcp:listen host port) or (tcp:listen host port backlog)`

Binds and listens on host:port for inbound TCP connections, returning a listener handle. Requires the NET-LISTEN capability. BACKLOG (default 128) is accepted for API completeness but is currently advisory only.

**See also:** TCP:ACCEPT, TCP:CLOSE-LISTENER!, TCP:LISTENER-P

---

### TCP:ACCEPT

**Type:** `FUNCTION`

**Syntax:** `(tcp:accept listener)`

Blocks until an inbound connection arrives on listener, returning (cons port peer-address) -- port is a duplex PORTS port, peer-address a NET:ADDRESS. Rejects use after tcp:close-listener! with a :CLOSED error.

**See also:** TCP:LISTEN, NET:PEER-ADDR

---

### TCP:SHUTDOWN!

**Type:** `FUNCTION`

**Syntax:** `(tcp:shutdown! port how)`

Shuts down port's read half, write half, or both (HOW: :read, :write, or :both) without closing it.

**See also:** TCP:CONNECT, PORTS:CLOSE!

---

### TCP:SET-READ-TIMEOUT!

**Type:** `FUNCTION`

**Syntax:** `(tcp:set-read-timeout! port ms)`

Sets port's read timeout in milliseconds; NIL blocks without a timeout (the default). A timed-out read signals a structured :TIMEOUT error.

**See also:** TCP:SET-WRITE-TIMEOUT!, PORTS:READ-BYTES!

---

### TCP:SET-WRITE-TIMEOUT!

**Type:** `FUNCTION`

**Syntax:** `(tcp:set-write-timeout! port ms)`

Sets port's write timeout in milliseconds; NIL blocks without a timeout (the default).

**See also:** TCP:SET-READ-TIMEOUT!, PORTS:WRITE-BYTES!

---

### TCP:CLOSE-LISTENER!

**Type:** `FUNCTION`

**Syntax:** `(tcp:close-listener! listener)`

Closes listener. Idempotent, like ports:close!. Every subsequent tcp:accept on this listener errors immediately with a :CLOSED error.

**See also:** TCP:LISTEN, TCP:LISTENER-OPEN-P

---

### TCP:LISTENER-P

**Type:** `FUNCTION`

**Syntax:** `(tcp:listener-p x)`

T if x is a TCP listener handle (as returned by tcp:listen).

**See also:** TCP:LISTEN, UDP:SOCKET-P

---

### TCP:LISTENER-OPEN-P

**Type:** `FUNCTION`

**Syntax:** `(tcp:listener-open-p listener)`

T unless listener has been closed.

**See also:** TCP:CLOSE-LISTENER!

---

# NET Functions

Addresses and DNS resolution (NET module, lib/37-net.lisp, issue #258); NET-DNS capability

---

### NET:ADDRESS

**Type:** `RECORD`

**Syntax:** `(net:make-address family ip port)`

A DEFRECORD with fields FAMILY (:ipv4 or :ipv6), IP (a string, never bracketed), and PORT (an integer 0-65535). Accessors: net:address-family, net:address-ip, net:address-port. First-class, printable address data -- the kernel never hands Lisp a raw platform socket-address struct.

**See also:** NET:RESOLVE, NET:ADDRESS->STRING, NET:LOCAL-ADDR, NET:PEER-ADDR

---

### NET:ADDRESS->STRING

**Type:** `FUNCTION`

**Syntax:** `(net:address->string addr)`

Formats addr as "ip:port", bracketing an IPv6 host (e.g. "[::1]:8080").

**Examples:**
```lisp
(NET:ADDRESS->STRING (NET:MAKE-ADDRESS (QUOTE :IPV4) "127.0.0.1" 80))  ; => "127.0.0.1:80"
```

**See also:** NET:ADDRESS, NET:RESOLVE

---

### NET:RESOLVE

**Type:** `FUNCTION`

**Syntax:** `(net:resolve host) or (net:resolve host port)`

Resolves host (and optional service port, default 0) to an ordered list of NET:ADDRESS records via the system resolver. Requires the NET-DNS capability. Signals a structured error (:CATEGORY :DNS) on failure.

**See also:** NET:ADDRESS, NET:LOCAL-ADDR, NET:PEER-ADDR, TCP:CONNECT

---

### NET:LOCAL-ADDR

**Type:** `FUNCTION`

**Syntax:** `(net:local-addr resource)`

The local NET:ADDRESS a connected TCP port or a TCP/UDP network handle is bound to. No capability required.

**See also:** NET:PEER-ADDR, TCP:LISTEN, UDP:BIND

---

### NET:PEER-ADDR

**Type:** `FUNCTION`

**Syntax:** `(net:peer-addr port)`

The remote NET:ADDRESS a connected TCP port is connected to. No capability required.

**See also:** NET:LOCAL-ADDR, TCP:CONNECT, TCP:ACCEPT

---

# MIME Functions

Case-insensitive multi-value headers and Content-Type parse/build (MIME module, lib/36-mime.lisp, issue #257)

---

### MIME:HEADER-NAME=

**Type:** `FUNCTION`

**Syntax:** `(mime:header-name= a b)`

Case-insensitive header-name equality (Unicode default case fold; agrees with ASCII case-insensitive comparison for HTTP header names).

**See also:** MIME:HEADERS-GET

---

### MIME:HEADERS-GET

**Type:** `FUNCTION`

**Syntax:** `(mime:headers-get headers name)`

The value of the first header in headers (a list of (name . value) conses) whose name matches name case-insensitively, or NIL.

**See also:** MIME:HEADERS-GET-ALL, MIME:HEADERS-ADD

---

### MIME:HEADERS-GET-ALL

**Type:** `FUNCTION`

**Syntax:** `(mime:headers-get-all headers name)`

Every value in headers whose name matches name case-insensitively, in original order — the multi-value accessor (e.g. every Set-Cookie value; never collapsed into one).

**See also:** MIME:HEADERS-GET, MIME:HEADERS-ADD

---

### MIME:HEADERS-ADD

**Type:** `FUNCTION`

**Syntax:** `(mime:headers-add headers name value)`

Returns a fresh headers list with (name . value) appended after headers. Never removes or collapses an existing entry of the same name — use for multi-value headers like Set-Cookie.

**See also:** MIME:HEADERS-SET, MIME:HEADERS-GET-ALL

---

### MIME:HEADERS-SET

**Type:** `FUNCTION`

**Syntax:** `(mime:headers-set headers name value)`

Returns a fresh headers list with every existing entry matching name (case-insensitive) removed and (name . value) appended once. Use only for headers that must be singular (e.g. Content-Type).

**See also:** MIME:HEADERS-ADD, MIME:HEADERS-REMOVE

---

### MIME:HEADERS-REMOVE

**Type:** `FUNCTION`

**Syntax:** `(mime:headers-remove headers name)`

Returns a fresh headers list with every entry matching name (case-insensitive) removed.

**See also:** MIME:HEADERS-SET

---

### MIME:HEADERS-NAMES

**Type:** `FUNCTION`

**Syntax:** `(mime:headers-names headers)`

The distinct header names in headers, each spelled the way it was first given, in first-seen order.

**See also:** MIME:HEADERS-GET

---

### MIME:PARSE-CONTENT-TYPE

**Type:** `FUNCTION`

**Syntax:** `(mime:parse-content-type s)`

Parses a Content-Type header value s into an alist (TYPE . type-string) (SUBTYPE . subtype-string) (PARAMETERS . ((name . value)...)), parameters in order with quoted-string values already unescaped.

**Examples:**
```lisp
(CDR (ASSOC (QUOTE TYPE) (MIME:PARSE-CONTENT-TYPE "text/html")))  ; => "text"
```

**See also:** MIME:BUILD-CONTENT-TYPE, MIME:CONTENT-TYPE-PARAMETER

---

### MIME:BUILD-CONTENT-TYPE

**Type:** `FUNCTION`

**Syntax:** `(mime:build-content-type type subtype &optional parameters)`

Builds a Content-Type header value from type, subtype, and an optional PARAMETERS list of (name . value) conses. A parameter value is written as a bare token when possible, else a quoted-string with "\" and '"' escaped.

**See also:** MIME:PARSE-CONTENT-TYPE

---

### MIME:CONTENT-TYPE-PARAMETER

**Type:** `FUNCTION`

**Syntax:** `(mime:content-type-parameter ct name)`

Case-insensitive lookup of parameter name's value in ct (as returned by mime:parse-content-type), or NIL if absent.

**See also:** MIME:PARSE-CONTENT-TYPE

---

# JSON Functions

JSON parse/stringify (JSON module, lib/35-json.lisp, issue #257)

---

### JSON:PARSE

**Type:** `FUNCTION`

**Syntax:** `(json:parse s &key (max-depth 512) (on-integer-overflow ':error))`

Parses JSON text s into a Lamedh value: object -> hash table (String keys, last-key-wins), array -> Array (not a list), string -> String, true -> T, false -> NIL, null -> the keyword :NULL (never NIL). Integer literals in i64 range are exact Numbers; out-of-range literals error unless :on-integer-overflow is :float. Every other number is a Float. Strict: rejects trailing garbage, unescaped control characters, leading zeros, and unpaired \u surrogate escapes, with line/column-located errors. :max-depth bounds nesting so deep input is a clean error, not a stack overflow.

**Examples:**
```lisp
(ARRAY->LIST (JSON:PARSE "[1,2,3]"))  ; => (1 2 3)
(JSON:PARSE "null")  ; => :NULL
```

**See also:** JSON:STRINGIFY, JSON:NULL-P

---

### JSON:STRINGIFY

**Type:** `FUNCTION`

**Syntax:** `(json:stringify v &key (pretty nil) (indent 2))`

Serializes Lamedh value v to a JSON text String — the exact inverse of json:parse's mapping. :pretty (default NIL) produces multi-line, :indent-space-per-level indented output; compact output otherwise. A Float is always written with a "." so it round-trips back as a Float, never an integer. Signals an error for a NaN/infinite Float or a value outside the mapping.

**Examples:**
```lisp
(JSON:STRINGIFY (LIST->ARRAY (LIST 1 2)))  ; => "[1,2]"
```

**See also:** JSON:PARSE

---

### JSON:NULL-P

**Type:** `FUNCTION`

**Syntax:** `(json:null-p v)`

T if v is the JSON null marker :NULL that json:parse produces for a JSON null literal (never NIL, so it is distinguishable from false and from an empty array).

**Examples:**
```lisp
(JSON:NULL-P (JSON:PARSE "null"))  ; => T
(JSON:NULL-P (JSON:PARSE "false"))  ; => ()
```

**See also:** JSON:PARSE

---

# URL Functions

URL parse/build, percent-encoding, and query-string parse/build (URL module, lib/34-url.lisp, issue #257)

---

### URL:ENCODE-PATH-SEGMENT

**Type:** `FUNCTION`

**Syntax:** `(url:encode-path-segment s)`

Percent-encodes s for use as one URL path segment: unreserved characters plus sub-delims and ":"/"@" stay literal; every other byte (including "/") is percent-encoded.

**Examples:**
```lisp
(URL:ENCODE-PATH-SEGMENT "a b")  ; => "a%20b"
```

**See also:** URL:ENCODE-QUERY-COMPONENT, URL:DECODE

---

### URL:ENCODE-QUERY-COMPONENT

**Type:** `FUNCTION`

**Syntax:** `(url:encode-query-component s)`

Percent-encodes s for use as a query-string key or value: only unreserved characters stay literal; everything else (including "&"/"="/"+") is percent-encoded.

**Examples:**
```lisp
(URL:ENCODE-QUERY-COMPONENT "a&b")  ; => "a%26b"
```

**See also:** URL:ENCODE-PATH-SEGMENT, URL:DECODE, URL:BUILD-QUERY

---

### URL:DECODE

**Type:** `FUNCTION`

**Syntax:** `(url:decode s &key (lossy nil))`

Percent-decodes s (produced by either encoder — decoding is context-free) back into the original Unicode STRING. Malformed "%XX" escapes are always errors; invalid UTF-8 after decoding is a strict error unless :lossy is T (U+FFFD substitution).

**Examples:**
```lisp
(URL:DECODE "a%20b")  ; => "a b"
```

**See also:** URL:ENCODE-PATH-SEGMENT, URL:ENCODE-QUERY-COMPONENT, URL:DECODE-PATH-SEGMENT, URL:DECODE-QUERY-COMPONENT

---

### URL:DECODE-PATH-SEGMENT

**Type:** `FUNCTION`

**Syntax:** `(url:decode-path-segment s &key (lossy nil))`

Alias for url:decode: percent-decoding is context-free, so this is identical to url:decode-query-component; provided so url:encode-path-segment has a same-named inverse.

**See also:** URL:DECODE, URL:ENCODE-PATH-SEGMENT

---

### URL:DECODE-QUERY-COMPONENT

**Type:** `FUNCTION`

**Syntax:** `(url:decode-query-component s &key (lossy nil))`

Alias for url:decode; see url:decode-path-segment.

**See also:** URL:DECODE, URL:ENCODE-QUERY-COMPONENT

---

### URL:PARSE-QUERY

**Type:** `FUNCTION`

**Syntax:** `(url:parse-query s)`

Parses query string s (without a leading "?") into a list of (key . value) conses, decoded via url:decode, in the string's original order. Repeated keys are preserved as repeated conses, never collapsed.

**Examples:**
```lisp
(URL:PARSE-QUERY "a=1&b=2")  ; => (("a" . "1") ("b" . "2"))
```

**See also:** URL:BUILD-QUERY, URL:DECODE

---

### URL:BUILD-QUERY

**Type:** `FUNCTION`

**Syntax:** `(url:build-query pairs)`

Builds a query string (without a leading "?") from pairs, a list of (key . value) conses, in the given order — the inverse of url:parse-query. Each key/value is percent-encoded via url:encode-query-component.

**See also:** URL:PARSE-QUERY, URL:ENCODE-QUERY-COMPONENT

---

### URL:PARSE

**Type:** `FUNCTION`

**Syntax:** `(url:parse s)`

Parses URL string s into an alist with keys SCHEME, USERINFO, HOST, PORT, PATH, QUERY, FRAGMENT. All are NIL when absent except PATH (always a string). PATH/QUERY/FRAGMENT/USERINFO are raw — still percent-encoded exactly as they appeared, never auto-decoded. No regular expressions are used.

**See also:** URL:BUILD, URL:SCHEME, URL:HOST, URL:PORT, URL:PATH, URL:QUERY, URL:FRAGMENT, URL:USERINFO

---

### URL:BUILD

**Type:** `FUNCTION`

**Syntax:** `(url:build u)`

Builds a URL string from an alist u shaped like url:parse's result — the inverse of url:parse.

**Examples:**
```lisp
(URL:BUILD (URL:PARSE "https://example.com/a?x=1"))  ; => "https://example.com/a?x=1"
```

**See also:** URL:PARSE

---

### URL:SCHEME

**Type:** `FUNCTION`

**Syntax:** `(url:scheme u)`

The SCHEME field of a url:parse alist, or NIL.

**See also:** URL:PARSE

---

### URL:USERINFO

**Type:** `FUNCTION`

**Syntax:** `(url:userinfo u)`

The USERINFO field of a url:parse alist, or NIL.

**See also:** URL:PARSE

---

### URL:HOST

**Type:** `FUNCTION`

**Syntax:** `(url:host u)`

The HOST field of a url:parse alist (a bracketed IPv6 literal is kept as one unit), or NIL.

**See also:** URL:PARSE

---

### URL:PORT

**Type:** `FUNCTION`

**Syntax:** `(url:port u)`

The PORT field of a url:parse alist (a Number), or NIL.

**See also:** URL:PARSE

---

### URL:PATH

**Type:** `FUNCTION`

**Syntax:** `(url:path u)`

The PATH field of a url:parse alist (always a String, raw/still-encoded, possibly "").

**See also:** URL:PARSE

---

### URL:QUERY

**Type:** `FUNCTION`

**Syntax:** `(url:query u)`

The QUERY field of a url:parse alist (raw text after "?", no leading delimiter), or NIL.

**See also:** URL:PARSE, URL:PARSE-QUERY

---

### URL:FRAGMENT

**Type:** `FUNCTION`

**Syntax:** `(url:fragment u)`

The FRAGMENT field of a url:parse alist (raw text after "#", no leading delimiter), or NIL.

**See also:** URL:PARSE

---

# HEX Functions

Hexadecimal encode/decode over Array<Char> bytes (HEX module, lib/33-hex.lisp, issue #257)

---

### HEX:ENCODE

**Type:** `FUNCTION`

**Syntax:** `(hex:encode bytes &key (case ':lower))`

Encodes bytes (an Array<Char>, elements Char or integer 0-255) as a hexadecimal ASCII String, two digits per byte. :case is :lower (default) or :upper.

**Examples:**
```lisp
(HEX:ENCODE (TEXT:STRING->UTF8 "AB"))  ; => "4142"
```

**See also:** HEX:DECODE, BASE64:ENCODE

---

### HEX:DECODE

**Type:** `FUNCTION`

**Syntax:** `(hex:decode s)`

Decodes s (a hexadecimal ASCII String, case-insensitive) into a fresh Array<Char> of the exact original bytes. Strict: an odd-length input or a non-hex-digit character is a named error.

**Examples:**
```lisp
(ARRAY->LIST (HEX:DECODE "4142"))  ; => (65 66)
```

**See also:** HEX:ENCODE, BASE64:DECODE

---

# BASE64 Functions

Base64 encode/decode over Array<Char> bytes (BASE64 module, lib/32-base64.lisp, issue #257)

---

### BASE64:ENCODE

**Type:** `FUNCTION`

**Syntax:** `(base64:encode bytes &key (alphabet ':standard) (pad t))`

Encodes bytes (an Array<Char>, elements Char or integer 0-255) as a Base64 ASCII String. :alphabet is :standard (RFC 4648 "+/") or :url (RFC 4648 "-_"); :pad (default T) controls trailing "=" padding.

**Examples:**
```lisp
(BASE64:ENCODE (TEXT:STRING->UTF8 "foo"))  ; => "Zm9v"
```

**See also:** BASE64:DECODE, HEX:ENCODE

---

### BASE64:DECODE

**Type:** `FUNCTION`

**Syntax:** `(base64:decode s &key (alphabet ':standard) (pad t))`

Decodes s (a Base64 ASCII String, per :alphabet/:pad) into a fresh Array<Char> of the exact original bytes. Strict: invalid characters, misplaced/wrong-count padding, or a length inconsistent with the padding policy are named errors.

**Examples:**
```lisp
(ARRAY->LIST (BASE64:DECODE "Zm9v"))  ; => (102 111 111)
```

**See also:** BASE64:ENCODE, HEX:DECODE

---

# PORTS Functions

Synchronous binary I/O ports (PORTS module, lib/31-ports.lisp)

---

### PORTS:OPEN-INPUT

**Type:** `FUNCTION`

**Syntax:** `(ports:open-input path)`

Opens path as a binary input port. Requires the READ-FS capability.

**See also:** PORTS:OPEN-OUTPUT, PORTS:OPEN-APPEND, PORTS:WITH-OPEN-PORT

---

### PORTS:OPEN-OUTPUT

**Type:** `FUNCTION`

**Syntax:** `(ports:open-output path)`

Opens path as a binary output port, truncating any existing contents (creating the file if needed). Requires the CREATE-FS capability.

**See also:** PORTS:OPEN-INPUT, PORTS:OPEN-APPEND

---

### PORTS:OPEN-APPEND

**Type:** `FUNCTION`

**Syntax:** `(ports:open-append path)`

Opens path as a binary output port positioned at end-of-file, preserving existing contents. Requires the CREATE-FS capability.

**See also:** PORTS:OPEN-OUTPUT

---

### PORTS:OPEN-INPUT-BYTES

**Type:** `FUNCTION`

**Syntax:** `(ports:open-input-bytes bytes)`

Opens a binary input port reading from a private copy of bytes (an Array<Char>). No capability required.

**Examples:**
```lisp
(PORTS:READ-BYTE! (PORTS:OPEN-INPUT-BYTES (LIST->ARRAY (LIST 65))))  ; => 65
```

**See also:** PORTS:OPEN-OUTPUT-BYTES, PORTS:OUTPUT-CONTENTS

---

### PORTS:OPEN-OUTPUT-BYTES

**Type:** `FUNCTION`

**Syntax:** `(ports:open-output-bytes)`

Opens a binary output port that accumulates written bytes in memory; read them back with ports:output-contents. No capability required; not seekable.

**See also:** PORTS:OPEN-INPUT-BYTES, PORTS:OUTPUT-CONTENTS

---

### PORTS:OUTPUT-CONTENTS

**Type:** `FUNCTION`

**Syntax:** `(ports:output-contents port)`

Returns the bytes written so far to an open-output-bytes port, as a fresh Array<Char>.

**See also:** PORTS:OPEN-OUTPUT-BYTES

---

### PORTS:STDIN

**Type:** `FUNCTION`

**Syntax:** `(ports:stdin)`

The process's standard input as a binary input port. Requires the IO capability.

**See also:** PORTS:STDOUT, PORTS:STDERR

---

### PORTS:STDOUT

**Type:** `FUNCTION`

**Syntax:** `(ports:stdout)`

The process's standard output as a binary output port. No capability required.

**See also:** PORTS:STDIN, PORTS:STDERR

---

### PORTS:STDERR

**Type:** `FUNCTION`

**Syntax:** `(ports:stderr)`

The process's standard error as a binary output port. No capability required.

**See also:** PORTS:STDIN, PORTS:STDOUT

---

### PORTS:READ-BYTE!

**Type:** `FUNCTION`

**Syntax:** `(ports:read-byte! port)`

Reads one byte from port as an integer 0-255, or NIL at EOF.

**See also:** PORTS:READ-BYTES!, PORTS:WRITE-BYTE!

---

### PORTS:READ-BYTES!

**Type:** `FUNCTION`

**Syntax:** `(ports:read-bytes! port n)`

Reads up to n bytes from port into a fresh Array<Char>. May be shorter than n (including empty) at EOF or on a partial read; never NIL.

**See also:** PORTS:READ-BYTE!, PORTS:READ-ALL-BYTES!

---

### PORTS:WRITE-BYTE!

**Type:** `FUNCTION`

**Syntax:** `(ports:write-byte! port byte)`

Writes one byte (a Char or integer 0-255) to port.

**See also:** PORTS:WRITE-BYTES!, PORTS:READ-BYTE!

---

### PORTS:WRITE-BYTES!

**Type:** `FUNCTION`

**Syntax:** `(ports:write-bytes! port bytes)`

Writes bytes (an Array<Char>) to port; returns the number of bytes actually written (may be less than the length of bytes on a partial write).

**See also:** PORTS:WRITE-BYTE!, PORTS:READ-BYTES!

---

### PORTS:FLUSH!

**Type:** `FUNCTION`

**Syntax:** `(ports:flush! port)`

Flushes any buffered writes on port.

**See also:** PORTS:WRITE-BYTES!, PORTS:CLOSE!

---

### PORTS:CLOSE!

**Type:** `FUNCTION`

**Syntax:** `(ports:close! port)`

Closes port. Idempotent: closing an already-closed port is a silent no-op, never an error.

**See also:** PORTS:WITH-OPEN-PORT, PORTS:OPEN-P

---

### PORTS:OPEN-P

**Type:** `FUNCTION`

**Syntax:** `(ports:open-p port)`

T if port has not been closed.

**See also:** PORTS:CLOSE!, PORTS:PORT-P

---

### PORTS:INPUT-P

**Type:** `FUNCTION`

**Syntax:** `(ports:input-p port)`

T if port supports reading.

**See also:** PORTS:OUTPUT-P, PORTS:PORT-P

---

### PORTS:OUTPUT-P

**Type:** `FUNCTION`

**Syntax:** `(ports:output-p port)`

T if port supports writing.

**See also:** PORTS:INPUT-P, PORTS:PORT-P

---

### PORTS:SEEKABLE-P

**Type:** `FUNCTION`

**Syntax:** `(ports:seekable-p port)`

T if port supports ports:position/ports:seek!. Files and byte-array input ports are seekable; byte-array output ports and the standard streams are not.

**See also:** PORTS:POSITION, PORTS:SEEK!

---

### PORTS:POSITION

**Type:** `FUNCTION`

**Syntax:** `(ports:position port)`

The current byte offset in a seekable port. Signals an error on a non-seekable port. Qualified-only: deliberately not bound unqualified by (import ports), because the Prelude's flat (position item lst) list helper would be shadowed.

**See also:** PORTS:SEEK!, PORTS:SEEKABLE-P

---

### PORTS:SEEK!

**Type:** `FUNCTION`

**Syntax:** `(ports:seek! port offset)`

Moves a seekable port to absolute byte offset from the start; returns the new position. Signals an error on a non-seekable port.

**See also:** PORTS:POSITION, PORTS:SEEKABLE-P

---

### PORTS:PORT-P

**Type:** `FUNCTION`

**Syntax:** `(ports:port-p v)`

T if v is a port (open or closed) of any kind.

**See also:** PORTS:OPEN-P, PORTS:INPUT-P, PORTS:OUTPUT-P

---

### PORTS:NAME

**Type:** `FUNCTION`

**Syntax:** `(ports:name port)`

port's diagnostic name (e.g. a file path, or "<stdin>").

**See also:** PORTS:KIND

---

### PORTS:KIND

**Type:** `FUNCTION`

**Syntax:** `(ports:kind port)`

port's diagnostic resource kind, as a symbol: FILE, MEMORY, STDIN, STDOUT, or STDERR (or a host-registered kind for an embedder-wrapped port).

**See also:** PORTS:NAME

---

### PORTS:READ-LINE!

**Type:** `FUNCTION`

**Syntax:** `(ports:read-line! port)`

Reads one line of text from port: bytes up to but excluding a trailing newline, decoded as UTF-8 (lossy). Returns NIL only at true EOF; a final line with no trailing newline is still returned once.

**See also:** PORTS:READ-STRING!, PORTS:WRITE-STRING!

---

### PORTS:READ-STRING!

**Type:** `FUNCTION`

**Syntax:** `(ports:read-string! port n)`

Reads up to n bytes from port and decodes them as UTF-8 (lossy), returning a STRING.

**See also:** PORTS:READ-LINE!, PORTS:WRITE-STRING!

---

### PORTS:WRITE-STRING!

**Type:** `FUNCTION`

**Syntax:** `(ports:write-string! port s)`

Writes string s to port as its exact UTF-8 bytes. Returns the number of bytes written.

**See also:** PORTS:READ-STRING!, PORTS:READ-LINE!

---

### PORTS:READ-ALL-BYTES!

**Type:** `FUNCTION`

**Syntax:** `(ports:read-all-bytes! port)`

Reads port to EOF, returning every remaining byte as a fresh Array<Char>.

**See also:** PORTS:READ-BYTES!

---

### PORTS:WITH-OPEN-PORT

**Type:** `MACRO`

**Syntax:** `(ports:with-open-port (var port-expr) body...)`

Binds var to the value of port-expr (a port) for body's dynamic extent, unconditionally closing it afterward: normal return, an ordinary error, THROW, RETURN-FROM, or GO unwinding all run the close, via UNWIND-PROTECT. Double-close is a no-op, so body may close var itself without error.

**Examples:**
```lisp
(PORTS:WITH-OPEN-PORT (P (PORTS:OPEN-INPUT-BYTES (LIST->ARRAY (LIST 1 2)))) (PORTS:READ-BYTE! P))  ; => 1
```

**See also:** PORTS:CLOSE!, PORTS:OPEN-INPUT, PORTS:OPEN-OUTPUT

---

# TEXT Functions

Explicit String <-> UTF-8 Array<Char> boundary (TEXT module, lib/30-text.lisp)

---

### TEXT:STRING->UTF8

**Type:** `FUNCTION`

**Syntax:** `(text:string->utf8 s)`

Returns the exact UTF-8 bytes of string s as a fresh Array<Char> (an array whose every element is a Char byte 0-255). Never fails: every Lisp STRING is valid Unicode. Call qualified, or (import text) first to use STRING->UTF8 unqualified.

**Examples:**
```lisp
(ARRAY-LENGTH* (TEXT:STRING->UTF8 "hi"))  ; => 2
```

**See also:** TEXT:UTF8->STRING, TEXT:UTF8->STRING-LOSSY

---

### TEXT:UTF8->STRING

**Type:** `FUNCTION`

**Syntax:** `(text:utf8->string bytes)`

Decodes bytes (an Array<Char>) as UTF-8 and returns the resulting STRING. Strict: signals a descriptive error naming the offending byte offset if bytes is not well-formed UTF-8; use UTF8->STRING-LOSSY for replacement-character decoding instead.

**Examples:**
```lisp
(TEXT:UTF8->STRING (TEXT:STRING->UTF8 "hi"))  ; => "hi"
```

**See also:** TEXT:STRING->UTF8, TEXT:UTF8->STRING-LOSSY

---

### TEXT:UTF8->STRING-LOSSY

**Type:** `FUNCTION`

**Syntax:** `(text:utf8->string-lossy bytes)`

Decodes bytes (an Array<Char>) as UTF-8, substituting the Unicode replacement character (U+FFFD) for any invalid byte sequence instead of signalling an error.

**Examples:**
```lisp
(TEXT:UTF8->STRING-LOSSY (TEXT:STRING->UTF8 "hi"))  ; => "hi"
```

**See also:** TEXT:STRING->UTF8, TEXT:UTF8->STRING

---

# STRINGS Functions

String operations

---

### CONCAT

**Type:** `FUNCTION`

**Syntax:** `(concat string...)`

Concatenates all string arguments.

**Examples:**
```lisp
(CONCAT "Hello" " " "World")  ; => "Hello World"
```

**See also:** INDEX, EXPLODE

---

### INDEX

**Type:** `FUNCTION`

**Syntax:** `(index string n)`

Returns the character at position n (0-indexed) as a string.

**Examples:**
```lisp
(INDEX "hello" 0)  ; => "h"
(INDEX "hello" 4)  ; => "o"
```

**See also:** CONCAT, EXPLODE

---

### EXPLODE

**Type:** `FUNCTION`

**Syntax:** `(explode atom)`

Converts an atom to a list of single-character symbols.

**Examples:**
```lisp
(EXPLODE (QUOTE HELLO))  ; => (H E L L O)
```

**See also:** IMPLODE, INTERN

---

### IMPLODE

**Type:** `FUNCTION`

**Syntax:** `(implode char-list)`

Converts a list of character symbols to an interned symbol.

**Examples:**
```lisp
(IMPLODE (QUOTE (H E L L O)))  ; => HELLO
```

**See also:** EXPLODE, INTERN, GENSYM

---

### GENSYM

**Type:** `FUNCTION`

**Syntax:** `(gensym)`

Generates a unique uninterned symbol.

**Returns:** Unique symbol like G0001

**See also:** INTERN, IMPLODE

---

### INTERN

**Type:** `FUNCTION`

**Syntax:** `(intern string)`

Interns a string as a symbol in the global symbol table.

**Examples:**
```lisp
(INTERN "HELLO")  ; => HELLO
```

**See also:** IMPLODE, GENSYM

---

### MAKNAM

**Type:** `FUNCTION`

**Syntax:** `(maknam char-list)`

Converts a list of character symbols or strings to an interned symbol.
Identical to IMPLODE. Lisp 1.5 name for the same operation.

**Examples:**
```lisp
(MAKNAM (QUOTE (F O O)))  ; => FOO
```

**See also:** IMPLODE, EXPLODE, INTERN, GENSYM

---

### STRING-LENGTH*

**Type:** `FUNCTION`

**Syntax:** `(string-length* s)`

Returns the number of Unicode characters in string s (not bytes). This is the kernel primitive; the Lisp layer builds higher-level string operations on top of it.

**Examples:**
```lisp
(STRING-LENGTH* "hello")  ; => 5
(STRING-LENGTH* "")  ; => 0
```

**See also:** SUBSTRING, INDEX, CONCAT

---

### SUBSTRING

**Type:** `FUNCTION`

**Syntax:** `(substring s start) or (substring s start end)`

Returns a substring of s from character index start (inclusive, 0-based) to end (exclusive). End defaults to the length of s. Indices are clamped to valid bounds. Characters are counted by Unicode code point, not bytes.

**Examples:**
```lisp
(SUBSTRING "hello" 1 3)  ; => "el"
(SUBSTRING "hello" 2)  ; => "llo"
```

**See also:** STRING-LENGTH*, INDEX, CONCAT

---

### CHAR-CODE

**Type:** `FUNCTION`

**Syntax:** `(char-code c)`

Returns the integer code point of c, where c is a Char value (from a literal like 'a') or a one-character string. Signals an error on an empty string.

**Examples:**
```lisp
(CHAR-CODE "A")  ; => 65
(CHAR-CODE 'a')  ; => 97
(CHAR-CODE " ")  ; => 32
```

**See also:** CODE-CHAR, MAKE-CHAR, CHARP, STRING-LENGTH*

---

### CODE-CHAR

**Type:** `FUNCTION`

**Syntax:** `(code-char n)`

Returns a one-character string containing the character at code point n. The inverse of CHAR-CODE. Signals an error if n is not a valid code point. (Use MAKE-CHAR to build a Char value instead of a string.)

**Examples:**
```lisp
(CODE-CHAR 65)  ; => "A"
(CODE-CHAR 97)  ; => "a"
```

**See also:** CHAR-CODE, MAKE-CHAR, STRING-LENGTH*

---

### MAKE-CHAR

**Type:** `FUNCTION`

**Syntax:** `(make-char n)`

Returns a Char value for integer code point n (0-255). The Char-producing complement of CODE-CHAR, which returns a one-character string. Inverse of CHAR-CODE on Char inputs.

**Examples:**
```lisp
(MAKE-CHAR 65)  ; => 'A'
(CHARP (MAKE-CHAR 65))  ; => T
```

**See also:** CHARP, CHAR-CODE, CODE-CHAR

---

### STRING->NUMBER

**Type:** `FUNCTION`

**Syntax:** `(string->number s)`

Parses string s as a number. Tries integer first, then float. Returns the parsed number on success, or NIL if the string cannot be parsed as a number. Leading and trailing whitespace is ignored.

**Examples:**
```lisp
(STRING->NUMBER "42")  ; => 42
(STRING->NUMBER "3.14")  ; => 3.14
(STRING->NUMBER "abc")  ; => ()
```

**See also:** NUMBER->STRING, READ

---

### NUMBER->STRING

**Type:** `FUNCTION`

**Syntax:** `(number->string n)`

Converts number n to its decimal string representation. Integers produce digit strings; floats produce Rust's default float formatting.

**Examples:**
```lisp
(NUMBER->STRING 42)  ; => "42"
(NUMBER->STRING 3.14)  ; => "3.14"
```

**See also:** STRING->NUMBER, PRIN1-TO-STRING, CONCAT

---

### PRIN1-TO-STRING

**Type:** `FUNCTION`

**Syntax:** `(prin1-to-string object)`

Returns the readable printed representation of object as a string, exactly as PRIN1 would print it to stdout. Strings are wrapped in double quotes; symbols print uppercased; cons cells print as S-expressions.

**Examples:**
```lisp
(PRIN1-TO-STRING "hello")  ; => "\"hello\""
(PRIN1-TO-STRING (QUOTE (1 2)))  ; => "(1 2)"
```

**See also:** PRINC-TO-STRING, PRIN1, NUMBER->STRING

---

### PRINC-TO-STRING

**Type:** `FUNCTION`

**Syntax:** `(princ-to-string object)`

Returns the human-readable printed representation of object as a string, exactly as PRINC would print it to stdout. Top-level strings are returned without surrounding quotes; everything else uses the same format as PRIN1-TO-STRING.

**Examples:**
```lisp
(PRINC-TO-STRING "hello")  ; => "hello"
(PRINC-TO-STRING 42)  ; => "42"
```

**See also:** PRIN1-TO-STRING, PRINC, NUMBER->STRING

---

### MAKE-STRING

**Type:** `FUNCTION`

**Syntax:** `(make-string n) or (make-string n char)`

Returns a fresh string of length n, every character char (a one-character string or code point; default space). Signals an error if n is negative.

**Examples:**
```lisp
(MAKE-STRING 3)  ; => "   "
(MAKE-STRING 3 "x")  ; => "xxx"
```

**See also:** STRING-REPEAT, STRING-PAD-LEFT, STRING-PAD-RIGHT

---

### STRING-EMPTY-P

**Type:** `FUNCTION`

**Syntax:** `(string-empty-p s)`

True if s has length zero.

**Examples:**
```lisp
(STRING-EMPTY-P "")  ; => T
(STRING-EMPTY-P "a")  ; => ()
```

**See also:** STRING-LENGTH*

---

### STRING-CONCAT

**Type:** `FUNCTION`

**Syntax:** `(string-concat &rest strs)`

Concatenates zero or more strings. A named alias for CONCAT.

**Examples:**
```lisp
(STRING-CONCAT "a" "b" "c")  ; => "abc"
(STRING-CONCAT)  ; => ""
```

**See also:** CONCAT

---

### CHAR-AT

**Type:** `FUNCTION`

**Syntax:** `(char-at s i)`

One-character access: the character at index i in s, as a one-character string. Unlike SUBSTRING, an out-of-range i signals a clear error naming i and s's length instead of clamping.

**Examples:**
```lisp
(CHAR-AT "hello" 0)  ; => "h"
(CHAR-AT "hello" 4)  ; => "o"
```

**See also:** SUBSTRING, STRING-LENGTH*

---

### STRING<

**Type:** `FUNCTION`

**Syntax:** `(string< a b)`

True if string a is lexicographically (by code point) before string b. Case-sensitive. Same ordering as STRING-LESSP, under CL's name for the case-sensitive comparison.

**Examples:**
```lisp
(STRING< "abc" "abd")  ; => T
```

**See also:** STRING>, STRING<=, STRING>=, STRING-LESSP, STRING-CI<

---

### STRING>

**Type:** `FUNCTION`

**Syntax:** `(string> a b)`

True if string a is lexicographically (by code point) after string b. Case-sensitive.

**Examples:**
```lisp
(STRING> "abd" "abc")  ; => T
```

**See also:** STRING<, STRING<=, STRING>=, STRING-CI>

---

### STRING<=

**Type:** `FUNCTION`

**Syntax:** `(string<= a b)`

Non-strict case-sensitive ordering: true unless a comes lexicographically after b.

**Examples:**
```lisp
(STRING<= "abc" "abc")  ; => T
```

**See also:** STRING<, STRING>, STRING>=, STRING-CI<=

---

### STRING>=

**Type:** `FUNCTION`

**Syntax:** `(string>= a b)`

Non-strict case-sensitive ordering: true unless a comes lexicographically before b.

**Examples:**
```lisp
(STRING>= "abc" "abc")  ; => T
```

**See also:** STRING<, STRING>, STRING<=, STRING-CI>=

---

### STRING-NE

**Type:** `FUNCTION`

**Syntax:** `(string-ne a b)`

True if strings a and b do NOT have the same contents. Case-sensitive. Named STRING-NE rather than CL's STRING/=: the reader does not treat `/` as a symbol constituent, so `string/=` cannot be written as one token.

**Examples:**
```lisp
(STRING-NE "a" "b")  ; => T
(STRING-NE "a" "a")  ; => ()
```

**See also:** STRING=, STRING-CI-NE

---

### STRING-CI=

**Type:** `FUNCTION`

**Syntax:** `(string-ci= a b)`

True if a and b have the same contents under Unicode default case folding (via STRING-CASEFOLD*: locale-independent, not ASCII-only). Named with a `-ci` infix rather than CL's STRING-EQUAL, because STRING-LESSP already has case-sensitive semantics here.

**Examples:**
```lisp
(STRING-CI= "ABC" "abc")  ; => T
```

**See also:** STRING=, STRING-CI-NE, STRING-CI<, STRING-CI>

---

### STRING-CI-NE

**Type:** `FUNCTION`

**Syntax:** `(string-ci-ne a b)`

True if a and b do NOT have the same contents under Unicode case folding.

**Examples:**
```lisp
(STRING-CI-NE "ABC" "xyz")  ; => T
```

**See also:** STRING-CI=, STRING-NE

---

### STRING-CI<

**Type:** `FUNCTION`

**Syntax:** `(string-ci< a b)`

True if a is lexicographically before b under Unicode case folding.

**Examples:**
```lisp
(STRING-CI< "abc" "ABD")  ; => T
```

**See also:** STRING-CI>, STRING-CI<=, STRING-CI>=, STRING<

---

### STRING-CI>

**Type:** `FUNCTION`

**Syntax:** `(string-ci> a b)`

True if a is lexicographically after b under Unicode case folding.

**Examples:**
```lisp
(STRING-CI> "ABD" "abc")  ; => T
```

**See also:** STRING-CI<, STRING-CI<=, STRING-CI>=

---

### STRING-CI<=

**Type:** `FUNCTION`

**Syntax:** `(string-ci<= a b)`

Non-strict case-insensitive ordering: true unless a comes after b under Unicode case folding.

**Examples:**
```lisp
(STRING-CI<= "ABC" "abc")  ; => T
```

**See also:** STRING-CI<, STRING-CI>, STRING-CI>=

---

### STRING-CI>=

**Type:** `FUNCTION`

**Syntax:** `(string-ci>= a b)`

Non-strict case-insensitive ordering: true unless a comes before b under Unicode case folding.

**Examples:**
```lisp
(STRING-CI>= "ABC" "abc")  ; => T
```

**See also:** STRING-CI<, STRING-CI>, STRING-CI<=

---

### STRING-LAST-INDEX-OF

**Type:** `FUNCTION`

**Syntax:** `(string-last-index-of s sub)`

Returns the index of the LAST (rightmost) occurrence of non-empty sub in s, or NIL if sub does not occur (or is empty).

**Examples:**
```lisp
(STRING-LAST-INDEX-OF "abcabc" "bc")  ; => 4
```

**See also:** STRING-INDEX-OF, STRING-COUNT

---

### STRING-COUNT

**Type:** `FUNCTION`

**Syntax:** `(string-count s sub)`

Counts non-overlapping occurrences of non-empty sub in s; 0 if sub is empty or does not occur.

**Examples:**
```lisp
(STRING-COUNT "abcabcabc" "abc")  ; => 3
(STRING-COUNT "aaaa" "aa")  ; => 2
```

**See also:** STRING-INDEX-OF, STRING-LAST-INDEX-OF

---

### STRING-REPLACE-FIRST

**Type:** `FUNCTION`

**Syntax:** `(string-replace-first s old new)`

Replaces only the first (non-empty) occurrence of old in s with new.

**Examples:**
```lisp
(STRING-REPLACE-FIRST "aaa" "a" "b")  ; => "baa"
```

**See also:** STRING-REPLACE, STRING-REPLACE-ALL

---

### STRING-REPLACE-ALL

**Type:** `FUNCTION`

**Syntax:** `(string-replace-all s old new)`

Replaces every (non-empty) occurrence of old in s with new. Alias for STRING-REPLACE, named to pair explicitly with STRING-REPLACE-FIRST.

**Examples:**
```lisp
(STRING-REPLACE-ALL "aaa" "a" "b")  ; => "bbb"
```

**See also:** STRING-REPLACE, STRING-REPLACE-FIRST

---

### STRING-TRIM-LEFT

**Type:** `FUNCTION`

**Syntax:** `(string-trim-left s)`

Removes leading whitespace from s.

**Examples:**
```lisp
(STRING-TRIM-LEFT "  hi  ")  ; => "hi  "
```

**See also:** STRING-TRIM-RIGHT, STRING-TRIM

---

### STRING-TRIM-RIGHT

**Type:** `FUNCTION`

**Syntax:** `(string-trim-right s)`

Removes trailing whitespace from s.

**Examples:**
```lisp
(STRING-TRIM-RIGHT "  hi  ")  ; => "  hi"
```

**See also:** STRING-TRIM-LEFT, STRING-TRIM

---

### STRING-CAPITALIZE

**Type:** `FUNCTION`

**Syntax:** `(string-capitalize s)`

Returns s with its first character uppercased (ASCII) and the rest lowercased.

**Examples:**
```lisp
(STRING-CAPITALIZE "hELLO world")  ; => "Hello World"
```

**See also:** STRING-UPCASE, STRING-DOWNCASE

---

### STRING-REVERSE

**Type:** `FUNCTION`

**Syntax:** `(string-reverse s)`

Reverses s. A named entry point onto the generic REVERSE (which already works on strings).

**Examples:**
```lisp
(STRING-REVERSE "hello")  ; => "olleh"
```

**See also:** REVERSE

---

# LISTS Functions

List manipulation

---

### CAR

**Type:** `FUNCTION`

**Syntax:** `(car list)`

Returns the first element of a list (the car of a cons cell).

**Arguments:**
- `LIST` - A cons cell or NIL

**Returns:** First element, or NIL for empty list

**Examples:**
```lisp
(CAR (QUOTE (A B C)))  ; => A
(CAR ())  ; => ()
```

**See also:** CDR, CONS, CADR, CADDR

---

### CDR

**Type:** `FUNCTION`

**Syntax:** `(cdr list)`

Returns the rest of a list (the cdr of a cons cell).

**Arguments:**
- `LIST` - A cons cell or NIL

**Returns:** Rest of list, or NIL

**Examples:**
```lisp
(CDR (QUOTE (A B C)))  ; => (B C)
(CDR (QUOTE (A)))  ; => ()
```

**See also:** CAR, CONS, CDDR

---

### CONS

**Type:** `FUNCTION`

**Syntax:** `(cons car cdr)`

Creates a new cons cell with the given car and cdr.

**Arguments:**
- `CAR` - First element
- `CDR` - Rest (usually a list)

**Returns:** New cons cell

**Examples:**
```lisp
(CONS (QUOTE A) (QUOTE (B C)))  ; => (A B C)
(CONS (QUOTE A) (QUOTE B))  ; => (A . B)
```

**See also:** CAR, CDR, LIST

---

### LIST

**Type:** `FUNCTION`

**Syntax:** `(list item...)`

Creates a list from its arguments.

**Examples:**
```lisp
(LIST 1 2 3)  ; => (1 2 3)
(LIST)  ; => ()
```

**See also:** CONS, APPEND

---

### APPEND

**Type:** `FUNCTION`

**Syntax:** `(append list1 list2)`

Concatenates two lists.

**Examples:**
```lisp
(APPEND (QUOTE (A B)) (QUOTE (C D)))  ; => (A B C D)
```

**See also:** CONS, LIST, REVERSE

---

### REVERSE

**Type:** `FUNCTION`

**Syntax:** `(reverse list)`

Returns a list with elements in reverse order.

**Examples:**
```lisp
(REVERSE (QUOTE (A B C)))  ; => (C B A)
```

**See also:** APPEND

---

### LENGTH

**Type:** `FUNCTION`

**Syntax:** `(length list)`

Returns the number of elements in a list.

**Examples:**
```lisp
(LENGTH (QUOTE (A B C)))  ; => 3
(LENGTH ())  ; => 0
```

**See also:** NULL

---

### NTH

**Type:** `FUNCTION`

**Syntax:** `(nth n list)`

Returns the nth element of a list (0-indexed).

**Examples:**
```lisp
(NTH 0 (QUOTE (A B C)))  ; => A
(NTH 2 (QUOTE (A B C)))  ; => C
```

**See also:** NTHCDR, CAR, CADR

---

### LAST

**Type:** `FUNCTION`

**Syntax:** `(last list)`

Returns the last cons cell of a list.

**Examples:**
```lisp
(LAST (QUOTE (A B C)))  ; => (C)
```

**See also:** CAR, CDR, NTH

---

### MEMBER

**Type:** `FUNCTION`

**Syntax:** `(member item list)`

Searches for item in list using EQUAL. Returns tail starting at match.

**Examples:**
```lisp
(MEMBER (QUOTE B) (QUOTE (A B C)))  ; => (B C)
(MEMBER (QUOTE X) (QUOTE (A B C)))  ; => ()
```

**See also:** ASSOC, EQUAL

---

### ASSOC

**Type:** `FUNCTION`

**Syntax:** `(assoc key alist)`

Searches an association list for a pair with matching key.

**Examples:**
```lisp
(ASSOC (QUOTE B) (QUOTE ((A . 1) (B . 2))))  ; => (B . 2)
```

**See also:** MEMBER, PAIRLIS

---

### MAPCAR

**Type:** `FUNCTION`

**Syntax:** `(mapcar function list)`

Applies function to each element of list, returns list of results.

**Examples:**
```lisp
(MAPCAR (LAMBDA (X) (* X 2)) (QUOTE (1 2 3)))  ; => (2 4 6)
```

**See also:** MAPLIST, APPLY

---

### MAPLIST

**Type:** `FUNCTION`

**Syntax:** `(maplist function list)`

Applies function to successive tails of list.

**Examples:**
```lisp
(MAPLIST (LAMBDA (X) (LENGTH X)) (QUOTE (A B C)))  ; => (3 2 1)
```

**See also:** MAPCAR

---

### SUBST

**Type:** `FUNCTION`

**Syntax:** `(subst new old tree)`

Replaces all occurrences of old with new in tree.

**Examples:**
```lisp
(SUBST (QUOTE X) (QUOTE A) (QUOTE (A B A)))  ; => (X B X)
```

---

### NTHCDR

**Type:** `FUNCTION`

**Syntax:** `(nthcdr n list)`

Returns the list after n applications of CDR. (nthcdr 0 list) returns list unchanged; (nthcdr 1 list) is CDR. Returns NIL if n exceeds the list length.

**Examples:**
```lisp
(NTHCDR 2 (QUOTE (A B C D)))  ; => (C D)
(NTHCDR 0 (QUOTE (A B)))  ; => (A B)
```

**See also:** NTH, CDR

---

### EFFACE

**Type:** `FUNCTION`

**Syntax:** `(efface item list)`

Returns a new list with the first occurrence of item (tested by EQUAL) removed. If item does not appear, returns the list unchanged. DELETE is an alias.

**Examples:**
```lisp
(EFFACE (QUOTE B) (QUOTE (A B C B)))  ; => (A C B)
(EFFACE (QUOTE X) (QUOTE (A B C)))  ; => (A B C)
```

**See also:** DELETE, MEMBER, SUBST

---

### RPLACA

**Type:** `FUNCTION`

**Syntax:** `(rplaca cons new-car)`

Destructively replaces the CAR of a cons cell with new-car. Returns the modified cons cell. This is a mutating operation — use with care as it modifies shared structure. Classic Lisp 1.5 primitive.

**Examples:**
```lisp
(LET ((X (CONS 1 2))) (RPLACA X 99) X)  ; => (99 . 2)
```

**See also:** RPLACD, CAR, CONS

---

### RPLACD

**Type:** `FUNCTION`

**Syntax:** `(rplacd cons new-cdr)`

Destructively replaces the CDR of a cons cell with new-cdr. Returns the modified cons cell. This is a mutating operation — use with care as it can create circular structure. Classic Lisp 1.5 primitive.

**Examples:**
```lisp
(LET ((X (CONS 1 2))) (RPLACD X 99) X)  ; => (1 . 99)
```

**See also:** RPLACA, CDR, CONS

---

### SUBLIS

**Type:** `FUNCTION`

**Syntax:** `(sublis alist tree)`

Substitutes values from an association list into a tree. For each leaf in tree that matches a key in alist (by EQUAL), replaces it with the corresponding value. Returns a new tree; does not modify the original. Classic Lisp 1.5 primitive.

**Examples:**
```lisp
(SUBLIS (QUOTE ((A . 1) (B . 2))) (QUOTE (A B C)))  ; => (1 2 C)
```

**See also:** SUBST, ASSOC

---

### SORT

**Type:** `FUNCTION`

**Syntax:** `(sort list comparator)`

Returns a new list with the same elements as list, sorted according to comparator. The comparator must be a two-argument predicate that returns T (or non-NIL) when its first argument should come before its second — i.e. a strict less-than. The sort is stable. Does not modify the original list.

**Examples:**
```lisp
(SORT (QUOTE (3 1 4 1 5 9 2 6)) (QUOTE <))  ; => (1 1 2 3 4 5 6 9)
(SORT (QUOTE ("banana" "apple" "cherry")) (QUOTE STRING<))  ; => ("apple" "banana" "cherry")
```

**See also:** MAPCAR, FILTER, REVERSE

---

# PREDICATES Functions

Type and value predicates

---

### ZEROP

**Type:** `FUNCTION`

**Syntax:** `(zerop n)`

Returns T if n is zero.

**Examples:**
```lisp
(ZEROP 0)  ; => T
(ZEROP 1)  ; => ()
```

**See also:** PLUSP, MINUSP, ONEP

---

### PLUSP

**Type:** `FUNCTION`

**Syntax:** `(plusp n)`

Returns T if n is positive (greater than zero).

**Examples:**
```lisp
(PLUSP 1)  ; => T
(PLUSP 0)  ; => ()
```

**See also:** MINUSP, ZEROP

---

### MINUSP

**Type:** `FUNCTION`

**Syntax:** `(minusp n)`

Returns T if n is negative (less than zero).

**Examples:**
```lisp
(MINUSP -1)  ; => T
(MINUSP 0)  ; => ()
```

**See also:** PLUSP, ZEROP, ABS

---

### EVENP

**Type:** `FUNCTION`

**Syntax:** `(evenp n)`

Returns T if n is an even integer.

**Examples:**
```lisp
(EVENP 2)  ; => T
(EVENP 3)  ; => ()
```

**See also:** ODDP

---

### ODDP

**Type:** `FUNCTION`

**Syntax:** `(oddp n)`

Returns T if n is an odd integer.

**Examples:**
```lisp
(ODDP 3)  ; => T
(ODDP 2)  ; => ()
```

**See also:** EVENP

---

### <

**Type:** `FUNCTION`

**Syntax:** `(< a b)`

Returns T if a is less than b.

**Examples:**
```lisp
(< 1 2)  ; => T
(< 2 1)  ; => ()
```

**See also:** >, =, LESSP, GREATERP

---

### >

**Type:** `FUNCTION`

**Syntax:** `(> a b)`

Returns T if a is greater than b.

**Examples:**
```lisp
(> 2 1)  ; => T
(> 1 2)  ; => ()
```

**See also:** <, =, LESSP, GREATERP

---

### =

**Type:** `FUNCTION`

**Syntax:** `(= a b)`

Returns T if a and b are numerically equal.

**Examples:**
```lisp
(= 1 1)  ; => T
(= 1 1.0)  ; => T
(= 1 2)  ; => ()
```

**See also:** EQ, EQUAL

---

### ATOM

**Type:** `FUNCTION`

**Syntax:** `(atom x)`

Returns T if x is not a cons cell (i.e., is an atom).

**Examples:**
```lisp
(ATOM (QUOTE A))  ; => T
(ATOM 42)  ; => T
(ATOM (QUOTE (A B)))  ; => ()
```

**See also:** CONSP, LISTP, SYMBOLP

---

### SYMBOLP

**Type:** `FUNCTION`

**Syntax:** `(symbolp x)`

Returns T if x is a symbol.

**Examples:**
```lisp
(SYMBOLP (QUOTE FOO))  ; => T
(SYMBOLP ())  ; => T
(SYMBOLP 42)  ; => ()
```

**See also:** ATOM, NUMBERP, STRINGP

---

### NUMBERP

**Type:** `FUNCTION`

**Syntax:** `(numberp x)`

Returns T if x is a number (integer or float).

**Examples:**
```lisp
(NUMBERP 42)  ; => T
(NUMBERP 3.14)  ; => T
(NUMBERP (QUOTE A))  ; => ()
```

**See also:** FIXP, FLOATP

---

### FIXP

**Type:** `FUNCTION`

**Syntax:** `(fixp x)`

Returns T if x is a fixed-point (integer) number.

**Examples:**
```lisp
(FIXP 42)  ; => T
(FIXP 3.14)  ; => ()
```

**See also:** FLOATP, NUMBERP

---

### FLOATP

**Type:** `FUNCTION`

**Syntax:** `(floatp x)`

Returns T if x is a floating-point number.

**Examples:**
```lisp
(FLOATP 3.14)  ; => T
(FLOATP 42)  ; => ()
```

**See also:** FIXP, NUMBERP

---

### CHARP

**Type:** `FUNCTION`

**Syntax:** `(charp x)`

Returns T if x is a Char value (produced by a char literal like 'a'). NIL for integers, strings, and all other types. Distinct from FIXP, which is NIL for chars.

**Examples:**
```lisp
(CHARP 'a')  ; => T
(CHARP 97)  ; => ()
(CHARP "a")  ; => ()
```

**See also:** MAKE-CHAR, CHAR-CODE, CODE-CHAR, FIXP

---

### STRINGP

**Type:** `FUNCTION`

**Syntax:** `(stringp x)`

Returns T if x is a string.

**Examples:**
```lisp
(STRINGP "hello")  ; => T
(STRINGP (QUOTE HELLO))  ; => ()
```

**See also:** SYMBOLP, ATOM

---

### CONSP

**Type:** `FUNCTION`

**Syntax:** `(consp x)`

Returns T if x is a cons cell.

**Examples:**
```lisp
(CONSP (QUOTE (A B)))  ; => T
(CONSP ())  ; => ()
```

**See also:** ATOM, LISTP, NULL

---

### LISTP

**Type:** `FUNCTION`

**Syntax:** `(listp x)`

Returns T if x is a list (cons or NIL).

**Examples:**
```lisp
(LISTP (QUOTE (A B)))  ; => T
(LISTP ())  ; => T
(LISTP (QUOTE A))  ; => ()
```

**See also:** CONSP, NULL, ATOM

---

### NULL

**Type:** `FUNCTION`

**Syntax:** `(null x)`

Returns T if x is NIL.

**Examples:**
```lisp
(NULL ())  ; => T
(NULL (QUOTE ()))  ; => T
(NULL (QUOTE (A)))  ; => ()
```

**See also:** NOT, LISTP

---

### NOT

**Type:** `FUNCTION`

**Syntax:** `(not x)`

Returns T if x is NIL, NIL otherwise.

**Examples:**
```lisp
(NOT ())  ; => T
(NOT T)  ; => ()
```

**See also:** NULL, AND, OR

---

### EQ

**Type:** `FUNCTION`

**Syntax:** `(eq a b)`

Returns T if a and b are the same object (identity test).

**Examples:**
```lisp
(EQ (QUOTE A) (QUOTE A))  ; => T
(EQ (QUOTE (1)) (QUOTE (1)))  ; => ()
```

**See also:** EQUAL, =

---

### EQUAL

**Type:** `FUNCTION`

**Syntax:** `(equal a b)`

Returns T if a and b are structurally equivalent (recursive comparison).

**Examples:**
```lisp
(EQUAL (QUOTE (A B)) (QUOTE (A B)))  ; => T
(EQUAL "hi" "hi")  ; => T
```

**See also:** EQ, =

---

### FUNCTIONP

**Type:** `FUNCTION`

**Syntax:** `(functionp x)`

Returns T if x is a function (lambda, fexpr, or builtin).

**Examples:**
```lisp
(FUNCTIONP (LAMBDA (X) X))  ; => T
```

**See also:** MACROP, BOUNDP

---

### BOUNDP

**Type:** `FUNCTION`

**Syntax:** `(boundp symbol)`

Returns T if symbol has a value binding.

**Examples:**
```lisp
(BOUNDP (QUOTE CAR))  ; => T
```

**See also:** SYMBOLP

---

### MACROP

**Type:** `FUNCTION`

**Syntax:** `(macrop x)`

Returns T if x is a macro object, NIL otherwise.

**Examples:**
```lisp
(DEFMACRO M (X) X)  ; => M
(MACROP (MACRO-FUNCTION (QUOTE M)))  ; => T
```

**See also:** FUNCTIONP, SYMBOLP, DEFMACRO

---

### ARRAYP

**Type:** `FUNCTION`

**Syntax:** `(arrayp x)`

Returns T if x is an array (created with ARRAY or MAKE-ARRAY); returns NIL otherwise. DEFSTRUCT instances are also arrays internally.

**Examples:**
```lisp
(ARRAYP (ARRAY 3))  ; => T
(ARRAYP (QUOTE (1 2 3)))  ; => ()
```

**See also:** ARRAY, ARRAY-LENGTH*, EXTENSION-P

---

### EXTENSION-P

**Type:** `FUNCTION`

**Syntax:** `(extension-p x)`

Returns T if x is an opaque extension value — a host-language object that was injected into the Lisp environment from Rust via the embedder API. Extension values have no direct Lisp representation but carry a type name accessible via EXTENSION-TYPE.

**See also:** EXTENSION-TYPE, ARRAYP, FUNCTIONP

---

### ERROR-P

**Type:** `FUNCTION`

**Syntax:** `(error-p x)`

Returns T if x is an error condition value (created with MAKE-ERROR or captured by ERRORSET). Returns NIL for any other value including ordinary NIL. Useful for dispatching on values that might be errors.

**Examples:**
```lisp
(ERROR-P (MAKE-ERROR "oops"))  ; => T
(ERROR-P 42)  ; => ()
```

**See also:** MAKE-ERROR, ERROR-MESSAGE, ERROR-DATA, ERRORSET

---

# ARITHMETIC Functions

Numeric operations

---

### +

**Type:** `FUNCTION`

**Syntax:** `(+ number...)`

Returns the sum of all arguments. With no arguments, returns 0.

**Arguments:**
- `NUMBERS` - Zero or more numbers to add

**Returns:** Sum of arguments (float if any argument is float)

**Examples:**
```lisp
(+ 1 2 3)  ; => 6
(+ 1.5 2.5)  ; => 4.0
(+)  ; => 0
```

**See also:** -, *, /

---

### -

**Type:** `FUNCTION`

**Syntax:** `(- number) or (- number number...)`

With one argument, returns negation. With multiple, subtracts rest from first.

**Arguments:**
- `NUMBER` - One or more numbers

**Returns:** Difference or negation

**Examples:**
```lisp
(- 5)  ; => -5
(- 10 3)  ; => 7
(- 10 3 2)  ; => 5
```

**See also:** +, *, /

---

### *

**Type:** `FUNCTION`

**Syntax:** `(* number...)`

Returns the product of all arguments. With no arguments, returns 1.

**Arguments:**
- `NUMBERS` - Zero or more numbers to multiply

**Returns:** Product of arguments

**Examples:**
```lisp
(* 2 3 4)  ; => 24
(*)  ; => 1
```

**See also:** +, -, /, EXPT

---

### /

**Type:** `FUNCTION`

**Syntax:** `(/ dividend divisor)`

Returns the quotient of two numbers. Integer division truncates toward zero.

**Arguments:**
- `DIVIDEND` - Number to divide
- `DIVISOR` - Number to divide by (non-zero)

**Returns:** Quotient

**Examples:**
```lisp
(/ 10 2)  ; => 5
(/ 10 3)  ; => 3
(/ 10.0 3)  ; => 3.333333
```

**See also:** REMAINDER, MOD, *, -

---

### REMAINDER

**Type:** `FUNCTION`

**Syntax:** `(remainder dividend divisor)`

Returns the remainder of integer division.

**Examples:**
```lisp
(REMAINDER 10 3)  ; => 1
(REMAINDER -10 3)  ; => -1
```

**See also:** MOD, /

---

### MOD

**Type:** `FUNCTION`

**Syntax:** `(mod x y)`

Returns x modulo y. Result has same sign as divisor.

**Examples:**
```lisp
(MOD 10 3)  ; => 1
(MOD -10 3)  ; => 2
```

**See also:** REMAINDER, /

---

### EXPT

**Type:** `FUNCTION`

**Syntax:** `(expt base power)`

Returns base raised to the power.

**Examples:**
```lisp
(EXPT 2 10)  ; => 1024
(EXPT 3 3)  ; => 27
```

**See also:** *, /

---

### ADD1

**Type:** `FUNCTION`

**Syntax:** `(add1 n)`

Returns n + 1. Same as (1+ n).

**Examples:**
```lisp
(ADD1 5)  ; => 6
```

**See also:** SUB1, +, -

---

### SUB1

**Type:** `FUNCTION`

**Syntax:** `(sub1 n)`

Returns n - 1. Same as (1- n).

**Examples:**
```lisp
(SUB1 5)  ; => 4
```

**See also:** ADD1, +, -

---

### ABS

**Type:** `FUNCTION`

**Syntax:** `(abs n)`

Returns the absolute value of n.

**Examples:**
```lisp
(ABS 5)  ; => 5
(ABS -5)  ; => 5
```

**See also:** MINUSP

---

### MAX

**Type:** `FUNCTION`

**Syntax:** `(max number...)`

Returns the largest of its arguments.

**Examples:**
```lisp
(MAX 1 5 3)  ; => 5
(MAX -1 -5)  ; => -1
```

**See also:** MIN

---

### MIN

**Type:** `FUNCTION`

**Syntax:** `(min number...)`

Returns the smallest of its arguments.

**Examples:**
```lisp
(MIN 1 5 3)  ; => 1
```

**See also:** MAX

---

### RANDOM

**Type:** `FUNCTION`

**Syntax:** `(random n)`

Returns a random integer from 0 (inclusive) to n (exclusive).

**Examples:**
```lisp
(RANDOM 10)  ; => "0-9 randomly"
```

---

### PLUS

**Type:** `FUNCTION`

**Syntax:** `(plus number...)`

Classic Lisp 1.5 name for +. Returns the sum of all arguments.

**Examples:**
```lisp
(PLUS 1 2 3)  ; => 6
```

**See also:** +, -, TIMES, DIFFERENCE, QUOTIENT

---

### DIFFERENCE

**Type:** `FUNCTION`

**Syntax:** `(difference number number...)`

Classic Lisp 1.5 name for -. With one argument returns negation; with more, subtracts rest from first.

**Examples:**
```lisp
(DIFFERENCE 10 3)  ; => 7
```

**See also:** -, PLUS, TIMES, QUOTIENT

---

### TIMES

**Type:** `FUNCTION`

**Syntax:** `(times number...)`

Classic Lisp 1.5 name for *. Returns the product of all arguments.

**Examples:**
```lisp
(TIMES 2 3 4)  ; => 24
```

**See also:** *, PLUS, DIFFERENCE, QUOTIENT

---

### QUOTIENT

**Type:** `FUNCTION`

**Syntax:** `(quotient dividend divisor)`

Classic Lisp 1.5 name for /. Returns the quotient; integer division truncates toward zero.

**Examples:**
```lisp
(QUOTIENT 10 3)  ; => 3
```

**See also:** /, PLUS, DIFFERENCE, TIMES, REMAINDER

---

### LESSP

**Type:** `FUNCTION`

**Syntax:** `(lessp a b)`

Classic Lisp 1.5 name for <. Returns T if a is strictly less than b.

**Examples:**
```lisp
(LESSP 1 2)  ; => T
(LESSP 2 1)  ; => ()
```

**See also:** <, GREATERP, =, FLOAT-LESSP

---

### GREATERP

**Type:** `FUNCTION`

**Syntax:** `(greaterp a b)`

Classic Lisp 1.5 name for >. Returns T if a is strictly greater than b.

**Examples:**
```lisp
(GREATERP 2 1)  ; => T
(GREATERP 1 2)  ; => ()
```

**See also:** >, LESSP, =, FLOAT-GREATERP

---

### EQUAL-NUMBER

**Type:** `FUNCTION`

**Syntax:** `(equal-number a b)`

Alias for =. Returns T if a and b are numerically equal. Accepts both integers and floats.

**Examples:**
```lisp
(EQUAL-NUMBER 1 1)  ; => T
(EQUAL-NUMBER 1 1.0)  ; => T
```

**See also:** =, LESSP, GREATERP

---

### 1+

**Type:** `FUNCTION`

**Syntax:** `(1+ n)`

Returns n + 1. Common Lisp-style alias for ADD1.

**Examples:**
```lisp
(1+ 5)  ; => 6
(1+ -1)  ; => 0
```

**See also:** 1-, ADD1, SUB1

---

### 1-

**Type:** `FUNCTION`

**Syntax:** `(1- n)`

Returns n - 1. Common Lisp-style alias for SUB1.

**Examples:**
```lisp
(1- 5)  ; => 4
(1- 1)  ; => 0
```

**See also:** 1+, SUB1, ADD1

---

### SQRT

**Type:** `FUNCTION`

**Syntax:** `(sqrt n)`

Returns the square root of n as a float. For integer square roots use ISQRT.

**Examples:**
```lisp
(SQRT 4)  ; => 2.0
(SQRT 2)  ; => 1.4142135
```

**See also:** ISQRT, EXPT, SIN, COS

---

### SIN

**Type:** `FUNCTION`

**Syntax:** `(sin radians)`

Returns the sine of an angle given in radians, as a float.

**Examples:**
```lisp
(SIN 0)  ; => 0.0
(SIN 3.14159)  ; => 0.0
```

**See also:** COS, TAN, SQRT

---

### COS

**Type:** `FUNCTION`

**Syntax:** `(cos radians)`

Returns the cosine of an angle given in radians, as a float.

**Examples:**
```lisp
(COS 0)  ; => 1.0
```

**See also:** SIN, TAN, SQRT

---

### TAN

**Type:** `FUNCTION`

**Syntax:** `(tan radians)`

Returns the tangent of an angle given in radians, as a float.

**Examples:**
```lisp
(TAN 0)  ; => 0.0
```

**See also:** SIN, COS

---

### LOG

**Type:** `FUNCTION`

**Syntax:** `(log x) or (log x base)`

With one argument returns the natural logarithm (ln) of x. With two arguments returns the logarithm of x in the given base.

**Examples:**
```lisp
(LOG 1)  ; => 0.0
(LOG 8 2)  ; => 3.0
```

**See also:** EXP, SQRT, EXPT

---

### EXP

**Type:** `FUNCTION`

**Syntax:** `(exp n)`

Returns e (Euler's number) raised to the power n, as a float.

**Examples:**
```lisp
(EXP 1)  ; => 2.71828
(EXP 0)  ; => 1.0
```

**See also:** LOG, EXPT

---

### FLOOR

**Type:** `FUNCTION`

**Syntax:** `(floor n)`

Returns the largest integer not greater than n (rounds toward negative infinity). Returns an integer even when given a float.

**Examples:**
```lisp
(FLOOR 3.7)  ; => 3
(FLOOR -3.7)  ; => -4
```

**See also:** CEILING, ROUND, TRUNCATE

---

### CEILING

**Type:** `FUNCTION`

**Syntax:** `(ceiling n)`

Returns the smallest integer not less than n (rounds toward positive infinity). Returns an integer even when given a float.

**Examples:**
```lisp
(CEILING 3.2)  ; => 4
(CEILING -3.7)  ; => -3
```

**See also:** FLOOR, ROUND, TRUNCATE

---

### ROUND

**Type:** `FUNCTION`

**Syntax:** `(round n)`

Returns n rounded to the nearest integer. Ties round half away from zero (e.g. 0.5 rounds to 1, -0.5 rounds to -1). Returns an integer.

**Examples:**
```lisp
(ROUND 3.5)  ; => 4
(ROUND 3.4)  ; => 3
(ROUND -3.5)  ; => -4
```

**See also:** FLOOR, CEILING, TRUNCATE

---

### TRUNCATE

**Type:** `FUNCTION`

**Syntax:** `(truncate n)`

Returns n truncated toward zero (drops the fractional part). Returns an integer. Equivalent to (floor n) for positive n and (ceiling n) for negative n.

**Examples:**
```lisp
(TRUNCATE 3.7)  ; => 3
(TRUNCATE -3.7)  ; => -3
```

**See also:** FLOOR, CEILING, ROUND

---

### GCD

**Type:** `FUNCTION`

**Syntax:** `(gcd a b)`

Returns the greatest common divisor of integers a and b. Both arguments must be integers; sign is ignored.

**Examples:**
```lisp
(GCD 12 8)  ; => 4
(GCD 7 5)  ; => 1
```

**See also:** LCM, MOD, REMAINDER

---

### LCM

**Type:** `FUNCTION`

**Syntax:** `(lcm a b)`

Returns the least common multiple of integers a and b. Returns 0 if either argument is 0. Both arguments must be integers.

**Examples:**
```lisp
(LCM 4 6)  ; => 12
(LCM 7 3)  ; => 21
```

**See also:** GCD, MOD

---

### ISQRT

**Type:** `FUNCTION`

**Syntax:** `(isqrt n)`

Returns the integer square root of n (the largest integer k such that k*k <= n). Requires a non-negative integer argument. Use SQRT for floating-point results.

**Examples:**
```lisp
(ISQRT 16)  ; => 4
(ISQRT 17)  ; => 4
(ISQRT 9)  ; => 3
```

**See also:** SQRT, GCD

---

### SIGNUM

**Type:** `FUNCTION`

**Syntax:** `(signum n)`

Returns the sign of n: -1 for negative, 0 for zero, 1 for positive. Works on both integers (returns an integer) and floats (returns a float).

**Examples:**
```lisp
(SIGNUM 42)  ; => 1
(SIGNUM -7)  ; => -1
(SIGNUM 0)  ; => 0
```

**See also:** ABS, PLUSP, MINUSP, ZEROP

---

### FLOAT-EQUAL

**Type:** `FUNCTION`

**Syntax:** `(float-equal a b)`

Returns T if a and b are exactly bit-equal as floating-point values. Unlike =, this correctly distinguishes -0.0 from 0.0. Accepts both floats and integers (integers are widened to float before comparison).

**Examples:**
```lisp
(FLOAT-EQUAL 1.0 1.0)  ; => T
(FLOAT-EQUAL 0.0 -0.0)  ; => ()
```

**See also:** =, FLOAT-LESSP, FLOAT-GREATERP

---

### FLOAT-LESSP

**Type:** `FUNCTION`

**Syntax:** `(float-lessp a b)`

Returns T if a is strictly less than b in floating-point comparison. Accepts floats and integers. Use < for general numeric comparison.

**Examples:**
```lisp
(FLOAT-LESSP 1.0 2.0)  ; => T
(FLOAT-LESSP 2.0 1.0)  ; => ()
```

**See also:** <, FLOAT-GREATERP, FLOAT-EQUAL

---

### FLOAT-GREATERP

**Type:** `FUNCTION`

**Syntax:** `(float-greaterp a b)`

Returns T if a is strictly greater than b in floating-point comparison. Accepts floats and integers. Use > for general numeric comparison.

**Examples:**
```lisp
(FLOAT-GREATERP 2.0 1.0)  ; => T
(FLOAT-GREATERP 1.0 2.0)  ; => ()
```

**See also:** >, FLOAT-LESSP, FLOAT-EQUAL

---

---
*Generated by Lamedh documentation system*
()
