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
//! # Run a script (shebang lines are supported); extra args land in *ARGV*
//! lamedh script.lisp arg1 arg2
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
//! 2. `*ARGV*` is bound to any arguments following the script path (or `()`).
//! 3. Each `-i path` argument is processed in order.  Directories are expanded
//!    to all `*.lisp` files sorted by name; files are loaded directly.
//! 4. If a positional script path was supplied, it is loaded and the process
//!    exits.  If `-s expr` was supplied, `expr` is evaluated and printed, then
//!    the process exits.  In these batch modes any load error exits with
//!    status 1 (issue #239).
//! 5. Otherwise the REPL loop starts, using rustyline for history and
//!    line-editing.  Incomplete input (unclosed parens/strings) prompts for
//!    continuation lines; Ctrl-C cancels the current input; exit with Ctrl-D
//!    or `(exit)`.
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
//! lamedh    = { path = "..", default-features = false }
//! rustyline = "14.0.0"
//! clap      = { version = "4.5.4", features = ["derive"] }
//!
//! [features]
//! default = ["jit"]
//! jit = ["lamedh/jit"]
//! ```

use clap::Parser;
use lamedh::{Shared, environment::Environment, eval_all, load_directory, load_file, printer};
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

    /// Script file to run, followed by arguments for the script.  The
    /// arguments are exposed to Lisp as the list *ARGV* (strings).  The
    /// process exits after the script finishes; use (exit n) to set the
    /// exit code.  A leading `#!` line in the script is ignored.
    #[arg(
        value_name = "SCRIPT [ARGS]...",
        trailing_var_arg = true,
        allow_hyphen_values = false
    )]
    script: Vec<String>,
}

fn main() {
    let args = Args::parse();
    // The tree-walking interpreter uses large stack frames; run it on a big
    // stack so reasonable recursion depths work, while the depth guard (#61)
    // turns runaway recursion into a recoverable error instead of an abort.
    lamedh::with_large_stack(move || run(args));
}

/// Load one `-i` path (file or directory).  Returns `false` on any error.
fn load_path(path: &str, env: &Shared<Environment>) -> bool {
    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("Error getting metadata for {path}: {e}");
            return false;
        }
    };
    // Load errors already name the file (and line/column), so print them
    // without an extra prefix.
    if metadata.is_dir() {
        if let Err(e) = load_directory(path, env) {
            eprintln!("{e}");
            return false;
        }
    } else if metadata.is_file()
        && let Err(e) = load_file(path, env)
    {
        eprintln!("{e}");
        return false;
    }
    true
}

/// Warn when evaluation newly set the OVERFLOW condition flag (issue #244):
/// wrapping integer arithmetic is silent at the value level, so surface the
/// transition once at the top level where a human (or agent) can see it.
fn warn_on_overflow(env: &Environment, had_overflow: bool) {
    if !had_overflow && env.flag_set("OVERFLOW") {
        eprintln!(
            "warning: integer overflow — a result wrapped around \
             (check (flag-set-p 'overflow); reset with (clear-flag 'overflow))"
        );
    }
}

fn run(args: Args) {
    // Use the embedded stdlib so the interpreter is self-contained. A lib/
    // directory on disk can still override or extend it via -i.
    let env = Environment::with_stdlib();

    // Grant capabilities requested on the command line.
    for cap in &args.capabilities {
        env.enable_feature(&cap.to_uppercase());
    }

    // Split the positional args into the script path and its arguments, and
    // expose the latter as *ARGV* (a list of strings; () when absent).
    let script = args.script.first().cloned();
    let argv = lamedh::LispVal::list(args.script.iter().skip(1).map(String::as_str));
    env.set("*ARGV*".to_string(), argv);

    // Batch mode (a script path or -s): a failed -i load is fatal (issue
    // #239) so CI and agent pipelines can trust the exit code.  In REPL mode
    // a failed load is reported but the session still starts.
    let batch = script.is_some() || args.s.is_some();

    // Load files from -i flag
    for path in &args.i {
        if !load_path(path, &env) && batch {
            std::process::exit(1);
        }
    }

    // Run a positional script file, then exit.
    if let Some(script_path) = script {
        let had_overflow = env.flag_set("OVERFLOW");
        if let Err(e) = load_file(&script_path, &env) {
            eprintln!("{e}");
            std::process::exit(1);
        }
        warn_on_overflow(&env, had_overflow);
        return;
    }

    // Execute s-expression(s) from -s flag
    if let Some(sexp) = args.s {
        let had_overflow = env.flag_set("OVERFLOW");
        match eval_all(&sexp, &env) {
            Ok(results) => {
                for r in results {
                    println!("{}", printer::print(&r));
                }
                warn_on_overflow(&env, had_overflow);
            }
            Err(e) => {
                eprintln!("{e}");
                std::process::exit(1);
            }
        }
        return;
    }

    // If no script and no -s flag, start REPL
    let mut rl = match DefaultEditor::new() {
        Ok(editor) => editor,
        Err(e) => {
            eprintln!("Failed to initialize line editor: {e}");
            std::process::exit(1);
        }
    };
    println!("Lamed (ל) Lisp 1.5");
    println!("Press Ctrl+D or type (exit) to quit; Ctrl+C cancels the current input");

    // Multi-line input: lines accumulate in `buffer` until the form is
    // complete (balanced parens / closed strings), then the whole buffer is
    // evaluated (issue #240).  eval_all, not eval_str, so several forms on
    // one line all run — matching -s.
    let mut buffer = String::new();
    loop {
        let prompt = if buffer.is_empty() {
            "(ל)> ".to_string()
        } else {
            " ...> ".to_string()
        };
        match rl.readline(&prompt) {
            Ok(line) => {
                if buffer.is_empty() && line.trim().is_empty() {
                    continue;
                }
                if !buffer.is_empty() {
                    buffer.push('\n');
                }
                buffer.push_str(&line);
                if lamedh::reader::is_incomplete(&buffer) {
                    continue;
                }
                let input = std::mem::take(&mut buffer);
                let _ = rl.add_history_entry(input.as_str());
                let had_overflow = env.flag_set("OVERFLOW");
                match eval_all(&input, &env) {
                    Ok(results) => {
                        for r in results {
                            println!("{}", printer::print(&r));
                        }
                    }
                    Err(e) => println!("{e}"),
                }
                warn_on_overflow(&env, had_overflow);
            }
            Err(ReadlineError::Interrupted) => {
                if buffer.is_empty() {
                    println!("Ctrl-C (press Ctrl-D or type (exit) to quit)");
                } else {
                    buffer.clear();
                    println!("(input cancelled)");
                }
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
