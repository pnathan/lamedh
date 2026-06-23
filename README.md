# Lamedh

A Lisp 1.5 implementation in Rust.

## Features

- **Read-Eval-Print Loop (REPL)** with rustyline
- **S-expression syntax** with reader macros: `'` (quote), `` ` `` (quasiquote), `,` (unquote)
- **Lexically scoped environments** with symbol interning
- **Property lists** for symbol metadata
- **Hash tables** for key-value storage
- **Standard library** with common Lisp functions

## Quick Start

```bash
# Build
cargo build

# Run REPL
cargo run

# Load a file
cargo run -- -i myfile.lisp

# Execute expression
cargo run -- -s "(+ 1 2)"
```

## Documentation

See the **[Reference Manual](docs/index.md)** for complete documentation:

- [Introduction](docs/introduction.md)
- [Getting Started](docs/getting_started.md)
- [Data Types](docs/data_types.md)
- [Special Forms](docs/special_forms.md)
- [Function Reference](docs/appendix_function_index.md)
- [Embedding Guide](docs/embedding.md)

## Example

```lisp
;; Define a function
(defun factorial (n)
  "Compute factorial of N."
  (if (= n 0)
      1
      (* n (factorial (- n 1)))))

;; Use it
(factorial 10)  ; => 3628800

;; Higher-order functions
(mapcar '(1 2 3 4 5)
        (lambda (x) (* x x)))
; => (1 4 9 16 25)
```

## License

See LICENSE file.
