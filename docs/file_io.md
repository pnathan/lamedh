# File I/O and Loading System

This document describes how the Lisp interpreter interacts with the file system, including file loading mechanisms, I/O functions, and system architecture.

## Overview

The interpreter provides a **read-only file loading system** designed for:
- Loading Lisp source files and evaluating them
- Bootstrapping standard libraries at startup
- Loading user code and configurations
- Batch processing of Lisp files

**Key Limitation**: This is a **read-only** system. There are no built-in functions for writing files, creating directories, or manipulating the file system.

## File Loading Mechanisms

### 1. Command-Line File Loading

#### Loading Individual Files

Use the `-i` flag to load files at startup:

```bash
cargo run -- -i myfile.lisp
```

Multiple files can be loaded in sequence:

```bash
cargo run -- -i file1.lisp -i file2.lisp -i file3.lisp
```

Files are loaded in the order specified. All loaded code shares the same global environment.

#### Loading Directories

Directories can be loaded using the same `-i` flag:

```bash
cargo run -- -i mylib/
```

When loading a directory:
- Only files with `.lisp` extension are loaded
- Files are loaded in **alphabetical order** by filename
- Subdirectories are **not** recursively loaded
- Non-existent directories are silently ignored

### 2. Programmatic File Loading

#### LOAD-FILE Function

**Syntax**: `(load-file filename)`

Loads and evaluates a Lisp file at runtime.

**Parameters**:
- `filename`: A string containing the path to the file (relative or absolute)

**Returns**: `T` on success

**Example**:
```lisp
(load-file "lib/helpers.lisp")  ; Returns T
(load-file "/absolute/path/to/file.lisp")
```

**Error Handling**:
- Throws error if file doesn't exist
- Throws error if file cannot be read (permissions, I/O error)
- Throws error if file contains syntax errors
- Error messages include the problematic file path

#### File Path Resolution

Paths are resolved as follows:
- **Relative paths**: Resolved relative to the current working directory
- **Absolute paths**: Used as-is
- **No path normalization**: Paths are passed directly to the OS

### 3. Automatic Startup Loading

The interpreter automatically loads standard libraries at startup in this order:

#### Step 1: Prologue Loading

File: `prologue.lisp` (project root directory)

- Loaded before user files
- Silently ignored if missing
- Errors reported only if file exists but fails to read/parse

Current prologue content:
```lisp
;; see lib/
(def lisp 'lamedh)
```

#### Step 2: Standard Library Loading

Directory: `lib/` (project root directory)

All `.lisp` files loaded in alphabetical order:

1. **00-core.lisp** - Core macros (`defun` with docstring support)
2. **01-list.lisp** - List processing functions (`pairlis`, `null`, `append`, `member`, `length`, `reverse`, `consp`, `listp`)
3. **02-cxr.lisp** - CAR/CDR compositions (`caar`, `cadr`, `caddr`, etc.)
4. **03-meta.lisp** - Metaprogramming utilities (`documentation`)
5. **04-predicates.lisp** - Type predicates (`equal`)
6. **05-math.lisp** - Mathematical functions

The numbering scheme (`00-`, `01-`, etc.) ensures correct load order.

#### Step 3: User Files

Files specified via `-i` flags are loaded after the standard library.

## File Loading Architecture

### Internal Implementation

File loading follows this pipeline:

```
File Path
   ↓
fs::read_to_string()  ← Read entire file into memory
   ↓
reader::read_all()    ← Parse all s-expressions
   ↓
For each expression:
   evaluator::eval()  ← Evaluate in shared environment
   ↓
Success (T) or Error
```

**Implementation locations**:
- `src/lib.rs:240-248` - `load_file()` core implementation
- `src/lib.rs:250-265` - `load_directory()` implementation
- `src/evaluator.rs:570-587` - `LoadFile` builtin function
- `src/main.rs:22-55` - CLI file loading orchestration
- `src/reader.rs:209-222` - `read_all()` parser

### Error Handling

#### File Reading Errors

```lisp
(load-file "nonexistent.lisp")
; Error: Failed to read file nonexistent.lisp: No such file or directory
```

#### Parsing Errors

```lisp
(load-file "broken.lisp")
; Error: Failed to parse file broken.lisp: [parse error details]
```

#### Evaluation Errors

If an error occurs during evaluation of expressions in the file, the standard error handling applies (see Error Handling in the Language Reference).

## Standard I/O Functions

While not file I/O per se, these functions handle input/output streams:

### READ

**Syntax**: `(read)`

Reads one line from standard input, parses it as a Lisp expression, and returns the parsed value.

**Example**:
```lisp
(read)  ; Waits for user input
; User types: (+ 1 2)
; Returns: (+ 1 2)  (the list, not evaluated)
```

### PRIN1

**Syntax**: `(prin1 object)`

Prints `object` in readable Lisp syntax (strings with quotes, escape characters visible).

**Returns**: The object that was printed

**Example**:
```lisp
(prin1 "hello")  ; Prints: "hello"
(prin1 '(a b c)) ; Prints: (A B C)
```

### PRINC

**Syntax**: `(princ object)`

Prints `object` in character form (strings without quotes).

**Returns**: The object that was printed

**Example**:
```lisp
(princ "hello")  ; Prints: hello (no quotes)
(princ 42)       ; Prints: 42
```

### TERPRI

**Syntax**: `(terpri)`

Prints a newline character.

**Returns**: `NIL`

**Example**:
```lisp
(princ "Line 1")
(terpri)
(princ "Line 2")
; Output:
; Line 1
; Line 2
```

## File Loading Best Practices

### 1. Library Organization

Organize library files with numeric prefixes to control load order:

```
lib/
  00-core.lisp       (fundamental macros)
  01-utilities.lisp  (helper functions)
  02-advanced.lisp   (depends on utilities)
```

### 2. Idempotent Loading

Design files to be safely loaded multiple times:

```lisp
;; Use DEF for constants (can be redefined)
(def *version* "1.0")

;; Use DEFUN for functions (can be redefined)
(defun helper (x) (* x 2))
```

### 3. Avoid Side Effects

Minimize side effects during file loading:

```lisp
;; GOOD: Define functions and constants
(defun process-data (data) ...)
(def *default-timeout* 30)

;; AVOID: Immediate execution
(print "Loading module...")  ; Prints every time file loads
```

### 4. Error Recovery

When dynamically loading files, use `ERRORSET` to handle failures gracefully:

```lisp
(defun safe-load (filename)
  "Load a file, returning T on success, NIL on failure."
  (if (errorset (list 'load-file filename))
      T
      (progn
        (princ "Failed to load: ")
        (princ filename)
        (terpri)
        NIL)))
```

## Limitations and Missing Features

### Not Available

The following file operations are **not supported**:

- **Writing files**: No `WRITE-FILE`, `APPEND-FILE`, or similar
- **File handles**: No `OPEN`, `CLOSE`, `WITH-OPEN-FILE`
- **File system operations**: No `DELETE-FILE`, `RENAME-FILE`, `COPY-FILE`
- **File metadata**: No `FILE-EXISTS-P`, `FILE-SIZE`, `FILE-MODIFIED-TIME`
- **Directory operations**: No `CREATE-DIRECTORY`, `LIST-DIRECTORY`, `DELETE-DIRECTORY`
- **Path manipulation**: No `PATH-JOIN`, `BASENAME`, `DIRNAME`
- **Binary I/O**: Text-based only, no binary file reading
- **Streaming I/O**: Entire file loaded into memory
- **File watching**: No filesystem event monitoring
- **Recursive directory loading**: Only single-level directory loading

### Current Capabilities Summary

| Operation | Available | Function/Method |
|-----------|-----------|-----------------|
| Load Lisp file | ✓ | `LOAD-FILE`, `-i` flag |
| Load directory | ✓ | `-i` flag, automatic `lib/` |
| Read from stdin | ✓ | `READ` |
| Write to stdout | ✓ | `PRIN1`, `PRINC`, `PRINT` |
| Write to files | ✗ | Not supported |
| File metadata | ✗ | Not supported |
| File manipulation | ✗ | Not supported |
| Binary I/O | ✗ | Not supported |

## Examples

### Example 1: Load Configuration

```lisp
;; config.lisp
(def *app-name* "My Application")
(def *version* "1.0.0")
(def *debug* T)
```

```bash
cargo run -- -i config.lisp
```

```lisp
; In REPL:
*app-name*  ; Returns "My Application"
*version*   ; Returns "1.0.0"
```

### Example 2: Load Utility Library

```lisp
;; utils.lisp
(defun square (x)
  "Return the square of x."
  (* x x))

(defun cube (x)
  "Return the cube of x."
  (* x x x))
```

```lisp
; In REPL:
(load-file "utils.lisp")  ; Returns T
(square 5)                ; Returns 25
(cube 3)                  ; Returns 27
```

### Example 3: Load Module with Dependencies

```lisp
;; math-lib.lisp
(defun factorial (n)
  (if (= n 0)
      1
      (* n (factorial (- n 1)))))

;; main.lisp
(load-file "math-lib.lisp")
(def result (factorial 5))
(prin1 result)
(terpri)
```

```bash
cargo run -- -i main.lisp
; Output: 120
```

### Example 4: Library Directory Structure

```
myproject/
  main.lisp
  lib/
    00-base.lisp      (basic utilities)
    01-math.lisp      (math functions, uses base)
    02-strings.lisp   (string functions, uses base)
    03-io.lisp        (I/O helpers, uses strings)
```

```bash
cargo run -- -i lib/ -i main.lisp
```

Files load in order: `00-base.lisp`, `01-math.lisp`, `02-strings.lisp`, `03-io.lisp`, `main.lisp`

### Example 5: Conditional Loading

```lisp
;; Load optional modules based on availability
(defun try-load (filename)
  "Try to load a file, returning T if successful, NIL otherwise."
  (if (errorset (list 'load-file filename))
      (progn
        (princ "Loaded: ")
        (princ filename)
        (terpri)
        T)
      (progn
        (princ "Skipped: ")
        (princ filename)
        (terpri)
        NIL)))

(try-load "optional-feature.lisp")
```

## Testing File Loading

Test files demonstrating file loading can be found in:
- `tests/test_load_file.rs` - Integration test for `LOAD-FILE` builtin
- `tests/load_file_test_sample.lisp` - Sample Lisp file for testing
- `tests/prog_test.lisp` - Example of loadable test file
- `tests/docstring_test.lisp` - Example with documentation strings

## Technical Details

### File Reading

Files are read entirely into memory as UTF-8 strings via Rust's `std::fs::read_to_string()`.

### Parsing

The `reader::read_all()` function uses nom parser combinators to parse multiple s-expressions from the file contents. Comments (`;` to end of line) are automatically stripped.

### Evaluation

Each parsed expression is evaluated sequentially in the shared global environment. If any expression fails, the error is propagated and subsequent expressions are not evaluated.

### Environment Sharing

All loaded files share the same global environment:
- Definitions in one file are visible in later files
- The order of file loading matters for dependencies
- Later definitions can override earlier ones

### Symbol Interning

All symbols are interned in a global symbol table, ensuring:
- Symbol identity is preserved across files
- Fast symbol comparison (pointer equality)
- Minimal memory overhead for repeated symbols
