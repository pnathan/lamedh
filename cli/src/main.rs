//! Command-line interface and interactive REPL for the Lamedh Lisp interpreter.
//!
//! # Crate: `lamedh-cli`  (binary name: `lamedh`)
//!
//! This is the thin driver crate that wires the [`lamedh`] library to a
//! terminal.  It is intentionally minimal: all interpreter logic lives in the
//! library crate; this crate only adds argument parsing ([`clap`]) and
//! line-editing ([`rustyline`]).
//!
//! ## Usage
//!
//! ```text
//! # Start the interactive REPL
//! lamedh
//!
//! # Load one or more files before the REPL (or before -s)
//! lamedh -i prelude.lisp -i src/
//!
//! # Evaluate a single s-expression and print the result, then exit
//! lamedh -s "(+ 1 2 3)"
//! ```
//!
//! ## Startup sequence
//!
//! 1. The embedded standard library is loaded via
//!    [`lamedh::environment::Environment::with_stdlib`].
//! 2. Each `-i path` argument is processed in order.  Directories are expanded
//!    to all `*.lisp` files sorted by name; files are loaded directly.
//! 3. If `-s expr` was supplied, `expr` is evaluated and printed, then the
//!    process exits.
//! 4. Otherwise the REPL loop starts, using rustyline for history and
//!    line-editing.  Exit with Ctrl-D (EOF) or Ctrl-C.
//!
//! ## Cargo manifest
//!
//! ```toml
//! [package]
//! name    = "lamedh-cli"
//! version = "0.1.2"
//! edition = "2024"
//!
//! [[bin]]
//! name = "lamedh"
//! path = "src/main.rs"
//!
//! [dependencies]
//! lamedh    = { path = ".." }
//! rustyline = "14.0.0"
//! clap      = { version = "4.5.4", features = ["derive"] }
//! ```

use clap::Parser;
use lamedh::{environment::Environment, eval_all, eval_line, load_directory, load_file, printer};
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use std::fs;

/// Command-line arguments for the `lamedh` binary.
///
/// Parsed by [`clap`] via the `#[derive(Parser)]` macro.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Load a file or directory of `.lisp` files before entering the REPL (or
    /// before `-s`).  May be specified multiple times; each path is processed
    /// in order.  Directories are expanded to all `*.lisp` files sorted by
    /// filename — numeric prefixes (`00-`, `01-`, …) control load order.
    #[arg(short, long, action = clap::ArgAction::Append)]
    i: Vec<String>,

    /// Evaluate one or more s-expressions and print the results, then exit.
    /// Mutually exclusive with the interactive REPL: if `-s` is present the
    /// REPL is not started.
    #[arg(short, long)]
    s: Option<String>,

    /// Grant a sandbox capability to the interpreter.  May be specified
    /// multiple times.  Known capabilities: READ-FS, CREATE-FS, TEMP-FS,
    /// SHELL, IO.  Capability names are case-insensitive.
    ///
    /// Example: `lamedh --capability READ-FS --capability SHELL`
    #[arg(long = "capability", short = 'c', action = clap::ArgAction::Append)]
    capabilities: Vec<String>,
}

fn main() {
    let args = Args::parse();
    // The tree-walking interpreter uses large stack frames; run it on a big
    // stack so reasonable recursion depths work, while the depth guard (#61)
    // turns runaway recursion into a recoverable error instead of an abort.
    lamedh::with_large_stack(move || run(args));
}

fn run(args: Args) {
    // Use the embedded stdlib so the interpreter is self-contained. A lib/
    // directory on disk can still override or extend it via -i.
    let env = Environment::with_stdlib();

    // Grant capabilities requested on the command line.
    for cap in &args.capabilities {
        env.enable_feature(&cap.to_uppercase());
    }

    // Load files from -i flag
    for path in args.i {
        let metadata = match fs::metadata(&path) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("Error getting metadata for {path}: {e}");
                continue;
            }
        };

        if metadata.is_dir() {
            if let Err(e) = load_directory(&path, &env) {
                eprintln!("Error loading directory {path}: {e:?}");
            }
        } else if metadata.is_file()
            && let Err(e) = load_file(&path, &env)
        {
            eprintln!("Error loading file {path}: {e:?}");
        }
    }

    // Execute s-expression(s) from -s flag
    if let Some(sexp) = args.s {
        match eval_all(&sexp, &env) {
            Ok(results) => {
                for r in results {
                    println!("{}", printer::print(&r));
                }
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }

    // If no -s flag, start REPL
    let mut rl = match DefaultEditor::new() {
        Ok(editor) => editor,
        Err(e) => {
            eprintln!("Failed to initialize line editor: {e}");
            std::process::exit(1);
        }
    };
    println!("Lamed (ל) Lisp 1.5");
    println!("Press Ctrl+C or Ctrl+D to exit");

    loop {
        let readline = rl.readline(r"(ל)> ");
        match readline {
            Ok(line) => {
                let _ = rl.add_history_entry(line.as_str());
                let output = eval_line(&line, &env);
                println!("{output}");
            }
            Err(ReadlineError::Interrupted) => {
                println!("Ctrl-C");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("Ctrl-D");
                break;
            }
            Err(err) => {
                println!("Error: {err:?}");
                break;
            }
        }
    }
}
