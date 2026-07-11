# The Lamedh Manual

Lamedh (ל, "Lamed") is an embeddable Lisp 1.5 interpreter written in Rust:
a small kernel, a mostly-Lisp standard library, gradual types with a
row-polymorphic checker and a native compilation tier, and a sandbox that
is closed by default. This manual covers the language as of version 0.3.0.

| Chapter | Covers |
|---|---|
| [1. Getting Started](01-getting-started.md) | Building, the REPL, scripts, command-line flags |
| [2. Language Basics](02-language-basics.md) | Syntax, definitions, control flow, lists, math, tail calls |
| [3. Data Structures](03-data-structures.md) | Arrays, hash tables, strings, property lists, `setf` |
| [4. Records and Types](04-records-and-types.md) | `defrecord`, row polymorphism, the checker, interfaces |
| [5. Functions, Macros, and Evaluation](05-functions-and-evaluation.md) | Closures, macros, fexprs, `vau`, dynamic variables |
| [6. Conditions and Restarts](06-conditions-and-restarts.md) | Errors, handlers, the restart protocol |
| [7. Sandboxing, Guards, and Concurrency](07-sandbox-guards-concurrency.md) | Capabilities, fuel, fences, `spawn`, channels |
| [8. Patterns and Metaprogramming](08-patterns-and-metaprogramming.md) | `match`, `sgrep`, `rewrite`, the rulebook, the change plane |
| [9. The Typed JIT and Embedding](09-jit-and-embedding.md) | Execution tiers, compilation, hosting Lamedh from Rust |
| [10. Modules](10-modules.md) | `defmodule`, `with-module`, `import`, module introspection, capability provision, `require`/`provide` load-once libraries |
| [11. Ports and Binary I/O](11-ports-and-io.md) | The `PORTS` module: files, in-memory byte ports, standard streams, `with-open-port`, structured I/O errors, host-wrapped ports |
| [12. Codecs](12-codecs.md) | `JSON`, `URL`, `BASE64`, `HEX`, `MIME`: parse/stringify, percent-encoding, Base64/Hex byte codecs, headers and Content-Type — capability-free pure-data transforms |

Conventions used throughout: REPL results are shown as `; => result`
comments; commands that need sandbox capabilities show the full command
line including `--capability` flags; symbols are written lowercase in
source and print uppercase, because the reader case-folds.

Every example in this manual was executed against the interpreter it
documents. If an example does not behave as printed, that is a bug — file
it.
