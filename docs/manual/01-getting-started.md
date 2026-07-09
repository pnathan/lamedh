# 1. Getting Started

## 1.1 What Lamedh Is

Lamedh (ל) is an embeddable Lisp 1.5 interpreter written in Rust. It ships
as a library crate you can link into a Rust host, plus a command-line
`lamedh` binary that gives you a REPL, a script runner, and a `-s`
one-shot evaluator. On top of the Lisp 1.5 core it adds lexical closures,
macros, fexprs, Kernel-style `vau` operatives, dynamic variables, arrays,
hash tables, structs, conditions, gradual Hindley-Milner typing, and an
optional native JIT backend. Host-facing side effects — reading files,
writing files, running shell commands, reading stdin — are disabled by
default. A Lamedh program can only touch the filesystem, spawn processes,
or block on input if the host or the CLI explicitly grants that
capability.

## 1.2 Building

Lamedh is a Cargo workspace with two crates: `lamedh` (the interpreter
library, at the repo root) and `lamedh-cli` (the `lamedh` binary, under
`cli/`). Build both from the repo root:

```bash
cargo build
```

This is the default build, and it enables the `jit` feature: the typed
compiler gets a native Cranelift backend, so `deffun-typed` functions that
qualify get compiled to machine code instead of running through the typed
closure interpreter.

If you don't want the Cranelift dependency, build without default
features:

```bash
cargo build --no-default-features
```

You still get the full typed checker and a working (non-native) typed
closure backend — only native code generation is missing.

Run the test suite with:

```bash
cargo test
```

`cargo test` and `cargo build` from the repo root cover both crates,
because the workspace manifest sets `default-members = [".", "cli"]`.

## 1.3 The REPL

With no arguments, `lamedh` starts an interactive REPL:

```bash
cargo run
```

or, once built, run the binary directly:

```bash
target/debug/lamedh
```

You'll see:

```
Lamed (ל) Lisp 1.5
Press Ctrl+D or type (exit) to quit; Ctrl+C cancels the current input
(ל)>
```

Type an expression and press Enter to evaluate it. If your parentheses or
quotes aren't balanced yet, the prompt switches to a continuation prompt,
` ...>`, and keeps accumulating lines until the form is complete:

```
(ל)> (defun sq (x)
 ...>   (* x x))
SQ
(ל)> (sq 6)
36
```

Ctrl-C cancels whatever you've typed so far (on an empty line it just
reminds you how to quit); Ctrl-D exits the REPL, and so does typing
`(exit)`.

REPL input history persists across sessions in `~/.lamedh_history` (up to
1000 entries), so pressing the up arrow in a new session recalls commands
from previous ones. Tab completes symbol names — it matches against every
symbol currently interned in the environment, which includes builtins,
every stdlib function, and anything you've defined so far in the session.

## 1.4 Running a Single Expression: `-s`

For one-shot evaluation without entering the REPL, use `-s`:

```bash
lamedh -s "(+ 1 2 3)"
```

```
6
```

`-s` takes exactly one string, but that string can hold more than one
top-level form; each form is evaluated in order and each result is
printed on its own line:

```bash
lamedh -s "(defun square (x) (* x x)) (square 5)"
```

```
SQUARE
25
```

`-s` can only be given once on the command line — it is not a repeatable
flag. If you need several independent expressions, put them all in one
`-s` string, or use `-i` (below) to load a file first.

## 1.5 Scripts

Pass a file path (instead of `-s`) to run it as a script:

```bash
lamedh path/to/script.lisp arg1 arg2
```

Anything after the script path becomes `*ARGV*`, a list of strings,
inside the script. A leading shebang line (`#!...`) is tolerated and
ignored, so scripts can be made directly executable:

```lisp
#!/usr/bin/env lamedh
;; hello.lisp
(princ "Hello, ")
(princ (car *ARGV*))
(princ "!")
(terpri)
(exit 0)
```

```bash
$ lamedh hello.lisp World
Hello, World!
```

`(exit n)` sets the process exit code and terminates immediately,
including from inside a script:

```lisp
(exit 3)
```

```bash
$ lamedh exitcode.lisp; echo $?
3
```

That makes `(exit (if (run-tests) 0 1))` a normal way to end a
test-runner script for use in CI.

## 1.6 Loading Files or Directories: `-i`

`-i` loads a file or a directory of `.lisp` files before the REPL starts
(or before `-s`/a script runs). It's repeatable — give it as many times
as you need, and each path loads in the order given:

```bash
lamedh -i lib.lisp -i main.lisp
```

If a `-i` path is a directory, every `*.lisp` file in it loads, sorted by
filename — this is why the standard library itself uses numeric
filename prefixes (`00-core.lisp`, `01-list.lisp`, ...) to control load
order:

```bash
$ lamedh -i mylib/ -s "(list *loaded-first* (greet-lib))"
(T "loaded-from-dir")
```

In batch modes (a script path or `-s`), a failed `-i` load is fatal — the
process exits with status 1, so CI and agent pipelines can trust the exit
code. In the REPL, a failed `-i` load is reported to stderr but the
session still starts.

## 1.7 Capabilities and the Sandbox

Lamedh treats filesystem access, shell execution, and blocking stdin
reads as capabilities that must be granted explicitly — a fresh
interpreter can't touch any of them. Calling a gated builtin without the
right capability fails with an error instead of doing anything:

```bash
$ lamedh -s '(read-file "data.txt")'
Error: READ-FS capability is not enabled (grant it via --capability READ-FS or the host API)
```

Grant capabilities on the command line with `--capability` (or the short
form `-c`), repeatable, case-insensitive:

```bash
$ lamedh --capability READ-FS -s '(read-file "data.txt")'
"hello file contents\n"
```

The known capability names are `READ-FS`, `CREATE-FS`, `TEMP-FS`,
`SHELL`, and `IO`. Chapter 7 covers the full capability model, including
how to grant capabilities from embedding Rust code and how attenuated
capability sets propagate to spawned interpreter threads.

## 1.8 Case: Symbols Are Uppercase

The reader upcases symbols as it interns them, matching Lisp 1.5
convention. `foo`, `Foo`, and `FOO` all name the same symbol:

```bash
$ lamedh -s "(defun Square (x) (* x x)) (square 4) (SQUARE 4)"
SQUARE
16
16
```

`defun` prints the symbol it just defined — uppercase, because that's
how it's stored — even though you can keep typing it in whatever case is
comfortable.

One related gotcha: if a function body is a single string with nothing
after it, that string is read as the docstring, not the return value:

```bash
$ lamedh -s '(defun greet () "hi") (greet)'
GREET
()
```

Give the function a real body (or add a docstring *and* a body) to get
the string back:

```bash
$ lamedh -s '(defun greet () "hi" (concat "h" "i")) (greet)'
GREET
"hi"
```

## 1.9 A First Session

Putting it together, a REPL session defining and using a function:

```
(ל)> (+ 1 2 3)
6
(ל)> (cons 'a '(b c))
(A B C)
(ל)> (defun square (x)
 ...>   "Square X."
 ...>   (* x x))
SQUARE
(ל)> (square 5)
25
(ל)> (mapcar (lambda (x) (square x)) '(1 2 3 4))
(1 4 9 16)
(ל)>
```

Ctrl-D to leave:

```
(ל)> (exit)
```

or just close the terminal — `(exit)` and Ctrl-D both flush REPL history
to `~/.lamedh_history` before the process ends.

From here, Chapter 2 covers the syntax and data types in full: numbers,
strings, symbols, lists, dotted pairs, and the reader's other literal
forms.
