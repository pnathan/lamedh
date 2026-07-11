# Lamedh Reference Manual

Auto-generated from Lisp documentation database.

---

## Categories

- INTROSPECTION - Inspecting registered definitions and compiled code
- FLAGS - Global condition/signal flags
- ENVIRONMENTS - First-class environment objects
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

Reads up to len bytes starting at byte offset from the file at path. Returns the bytes as a string (lossily decoded from UTF-8; non-UTF-8 bytes become replacement characters). Returns a shorter string if fewer than len bytes are available. Requires the READ-FS capability.

**See also:** READ-FILE, READ-FILE-BYTE, WRITE-FILE, FEATURE-ENABLED-P

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

**See also:** DEFUN, DEFUN-TYPED, DEFUN-TYPED-OPT, CHECK-TYPE, LAMBDA

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
(STRING-CAPITALIZE "hELLO world")  ; => "Hello world"
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
