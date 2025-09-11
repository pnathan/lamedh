# Lithhp Lisp Interpreter

Welcome to the documentation for Lithhp, a Lisp interpreter written in Rust.

## Introduction

Lithhp is a lightweight Lisp interpreter that supports a subset of the Lisp language. It is designed to be a simple, embeddable interpreter that can be used to run Lisp code.

For a detailed description of the language, see the [Litthp Lisp Language Reference](language_reference.md).

## Building and Running

To build the project, run the following command from the `lithhp` directory:

```bash
cargo build
```

To run the REPL, use the following command:

```bash
cargo run
```

You can also execute a file containing Lisp code:

```bash
cargo run -- -i my-file.lisp
```

Or execute a single s-expression:

```bash
cargo run -- -s "(+ 1 2)"
```
