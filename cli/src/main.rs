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
//!    line-editing.  History persists across sessions in `~/.lamedh_history`
//!    (loaded on startup, and saved after each accepted entry so it survives
//!    `(exit)` — which exits the process from inside the evaluator — as well
//!    as Ctrl-D), and Tab completes on the names of every symbol currently
//!    interned in the environment (builtins, stdlib functions, and user
//!    definitions).  Incomplete input (unclosed parens/strings) prompts for
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
//! dirs      = "6"
//!
//! [features]
//! default = ["jit"]
//! jit = ["lamedh/jit"]
//! ```

use clap::Parser;
use lamedh::check;
use lamedh::{
    Shared, environment::Environment, eval_all, formatter, load_directory, load_file, printer,
    test_runner,
};
use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Context, Helper};
use std::fs;

/// A character that ends a Lisp token: whitespace or a reader special char.
/// Used both to find the token under the cursor and to reject symbol names
/// that could not be re-typed as a single token.
fn is_token_delimiter(c: char) -> bool {
    c.is_whitespace() || "()'\"`,;".contains(c)
}

/// Byte offset where the token under `pos` begins: scan back from `pos` for a
/// delimiter and start just after it (or at 0). `pos` is a rustyline cursor
/// byte offset, always on a char boundary, so the slicing here never panics.
fn token_start(line: &str, pos: usize) -> usize {
    line[..pos]
        .rfind(is_token_delimiter)
        .map(|i| i + 1)
        .unwrap_or(0)
}

/// Completion candidates (lowercased, ready to insert) for `prefix` against the
/// interned symbol `names`. Symbols are interned uppercase and the reader
/// upcases on read, so we match case-insensitively and display lowercase.
///
/// Names that embed a delimiter — e.g. a symbol made via `(intern "a b")` — are
/// dropped: inserting one would split the edit line into several tokens instead
/// of completing a single identifier.
fn symbol_completions(names: Vec<String>, prefix: &str) -> Vec<String> {
    let upper_prefix = prefix.to_uppercase();
    let mut out: Vec<String> = names
        .into_iter()
        .filter(|name| name.starts_with(&upper_prefix))
        .filter(|name| !name.chars().any(is_token_delimiter))
        .map(|name| name.to_lowercase())
        .collect();
    out.sort();
    out.dedup();
    out
}

/// `rustyline` line-editor helper providing tab-completion over every symbol
/// currently interned in the interpreter's global symbol table (builtins,
/// stdlib functions, and anything the user has defined at the REPL).
///
/// Hinting/highlighting/validation are left at their default (no-op)
/// implementations; only completion is customized.
struct LispHelper {
    env: Shared<Environment>,
}

impl Helper for LispHelper {}
impl Hinter for LispHelper {
    type Hint = String;
}
impl Highlighter for LispHelper {}
impl Validator for LispHelper {}

impl Completer for LispHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let start = token_start(line, pos);
        let prefix = &line[start..pos];
        if prefix.is_empty() {
            return Ok((pos, vec![]));
        }
        let matches: Vec<Pair> = symbol_completions(self.env.all_symbol_names(), prefix)
            .into_iter()
            .map(|display| Pair {
                display: display.clone(),
                replacement: display,
            })
            .collect();
        Ok((start, matches))
    }
}

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
    /// Repeatable: each -s string is evaluated in order, in one shared
    /// environment.
    #[arg(short, long, action = clap::ArgAction::Append)]
    s: Vec<String>,

    /// Grant a sandbox capability to the interpreter.  May be specified
    /// multiple times.  Known capabilities: READ-FS, CREATE-FS, TEMP-FS,
    /// SHELL, IO, NET-DNS, NET-CONNECT, NET-LISTEN, OS-ENV, OS-ENV-WRITE,
    /// OS-PROCESS, OS-SIGNAL.  Capability names are case-insensitive.
    ///
    /// By default all capabilities are enabled.  Use `--sandbox` to start
    /// with none, then grant individual ones with `-c`.
    ///
    /// Example: `lamedh --sandbox --capability READ-FS --capability SHELL`
    #[arg(long = "capability", short = 'c', action = clap::ArgAction::Append)]
    capabilities: Vec<String>,

    /// Start with all capabilities disabled (sandboxed).  Individual
    /// capabilities can still be granted with `--capability`.
    #[arg(long)]
    sandbox: bool,

    /// Statically check the given file(s) without executing them: report
    /// parse errors, unbound function calls (with did-you-mean / CL-ism
    /// guidance), and provable arity mismatches.  Exit 0 if clean, 1 if there
    /// are warnings, 2 if a file failed to parse or read.  The files to check
    /// are the positional arguments; `--check` with no files is an error.
    #[arg(long)]
    check: bool,

    /// Output format for `--check`/`--test` diagnostics: `human` (default)
    /// writes plain text; `sexpr` writes one readable s-expression per
    /// finding/failing test (see docs/check.md and docs/testing-cli.md for
    /// the schemas).
    #[arg(long = "error-format", value_name = "FORMAT", default_value = "human")]
    error_format: String,

    /// Rewrite the given file(s) in place to canonical form and print one
    /// line per file that changed. Exit 2 if a file cannot be read or
    /// parsed. The files to format are the positional arguments.
    #[arg(long)]
    fmt: bool,

    /// Like `--fmt` but writes nothing: prints one line per file that would
    /// change and exits 1 if any would, 0 if every file is already
    /// canonical. Exit 2 if a file cannot be read or parsed.
    #[arg(long = "fmt-check")]
    fmt_check: bool,

    /// Run every test registered (via `deftest`) by loading the given
    /// file(s)/directories, then report pass/fail. Directories load sorted
    /// `*.lisp` files, same rule as `-i`. Prints one line per failing test,
    /// then a `test result: N passed; M failed` summary line. Exit 0 if
    /// every test passed, 1 if any failed, 2 on a load/parse failure. The
    /// files to test are the positional arguments; `--test` with no files
    /// is an error.
    #[arg(long)]
    test: bool,

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

/// The capability names granted by default (everything). Shared by every
/// mode that builds a "developer tool, not a sandbox" environment: plain
/// batch/REPL mode and `--test` (script-mode semantics, since a test file is
/// just another script). `--sandbox` reverts to none of these; `-c` then
/// grants individual ones back on top.
const DEFAULT_CAPABILITIES: &[&str] = &[
    "READ-FS",
    "CREATE-FS",
    "TEMP-FS",
    "SHELL",
    "IO",
    "NET-DNS",
    "NET-CONNECT",
    "NET-LISTEN",
    "OS-ENV",
    "OS-ENV-WRITE",
    "OS-PROCESS",
    "OS-SIGNAL",
];

/// Grant `args`' capabilities to `env`: every [`DEFAULT_CAPABILITIES`] unless
/// `--sandbox` was passed, then any explicit `-c/--capability` flags on top.
fn grant_capabilities(args: &Args, env: &Shared<Environment>) {
    if !args.sandbox {
        for cap in DEFAULT_CAPABILITIES {
            env.enable_feature(cap);
        }
    }
    for cap in &args.capabilities {
        env.enable_feature(&cap.to_uppercase());
    }
}

/// Quote a Rust string as a Lisp string literal (escaping `"` and `\`).
/// Mirrors `check::sexpr_string` for `--test --error-format=sexpr` output.
fn sexpr_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

/// `--fmt` / `--fmt-check`: rewrite (or report on) each file's canonical
/// form. Never returns.
fn run_fmt(args: &Args, check_only: bool) -> ! {
    let flag = if check_only { "--fmt-check" } else { "--fmt" };
    if args.script.is_empty() {
        eprintln!("error: {flag} requires one or more files");
        std::process::exit(2);
    }
    // A minimal kernel is enough: formatting only needs the reader (to
    // confirm the file parses, and to intern symbols along the way), never
    // evaluation.
    let env = Environment::new_with_builtins();
    let mut changed = false;
    let mut hard_error = false;
    for path in &args.script {
        let src = match fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("{path}: cannot read file: {e}");
                hard_error = true;
                continue;
            }
        };
        if let Err(e) = lamedh::reader::read_all(&src, &env) {
            eprintln!("{path}: {e}");
            hard_error = true;
            continue;
        }
        let formatted = formatter::format_source(&src);
        if formatted == src {
            continue;
        }
        changed = true;
        if !check_only && let Err(e) = fs::write(path, &formatted) {
            eprintln!("{path}: cannot write file: {e}");
            hard_error = true;
            continue;
        }
        println!("{path}");
    }
    if hard_error {
        std::process::exit(2);
    }
    std::process::exit(if check_only && changed { 1 } else { 0 });
}

/// `--test`: load the given files/directories, run every registered test,
/// and report pass/fail. Never returns.
fn run_test(args: &Args) -> ! {
    let sexpr = match args.error_format.as_str() {
        "human" => false,
        "sexpr" => true,
        other => {
            eprintln!("error: unknown --error-format '{other}' (expected 'human' or 'sexpr')");
            std::process::exit(2);
        }
    };
    if args.script.is_empty() {
        eprintln!("error: --test requires one or more files or directories");
        std::process::exit(2);
    }

    // Script-mode semantics: a full stdlib world with every capability on
    // (a test file is just another script), same as batch script/-s mode.
    let env = Environment::with_stdlib_fresh();
    grant_capabilities(args, &env);

    for path in &args.script {
        if !load_path(path, &env) {
            std::process::exit(2);
        }
    }

    let outcomes = match test_runner::run_registered_tests(&env) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{}", lamedh::format_error_with_backtrace(&e, &env));
            std::process::exit(2);
        }
    };

    let failed = outcomes.iter().filter(|o| !o.passed).count();
    for o in outcomes.iter().filter(|o| !o.passed) {
        let message = o.message.as_deref().unwrap_or("");
        if sexpr {
            println!(
                "((test . {}) (status . fail) (message . {}))",
                sexpr_string(&o.name),
                sexpr_string(message),
            );
        } else {
            println!("FAIL {}: {}", o.name, message);
        }
    }
    let passed = outcomes.len() - failed;
    println!("test result: {passed} passed; {failed} failed");
    std::process::exit(if failed == 0 { 0 } else { 1 });
}

/// Static-check mode (`--check`): verify files without executing them, print
/// diagnostics in the requested format, and exit with the check exit code.
/// Never returns.
fn run_check(args: &Args) -> ! {
    let sexpr = match args.error_format.as_str() {
        "human" => false,
        "sexpr" => true,
        other => {
            eprintln!("error: unknown --error-format '{other}' (expected 'human' or 'sexpr')");
            std::process::exit(2);
        }
    };
    if args.script.is_empty() {
        eprintln!("error: --check requires one or more files to check");
        std::process::exit(2);
    }
    let findings = check::check_paths(&args.script);
    for f in &findings {
        if sexpr {
            println!("{}", f.to_sexpr());
        } else {
            println!("{}", f.to_human());
        }
    }
    std::process::exit(check::exit_code(&findings));
}

fn run(args: Args) {
    // Static-check, format, and test modes short-circuit everything else:
    // none of them load -i files or start a REPL, and each builds its own
    // environment sized to what it actually needs.
    if args.check {
        run_check(&args);
    }
    if args.fmt || args.fmt_check {
        run_fmt(&args, args.fmt_check);
    }
    if args.test {
        run_test(&args);
    }

    // Use the embedded stdlib so the interpreter is self-contained. A lib/
    // directory on disk can still override or extend it via -i.
    //
    // The `_fresh` variant: the CLI builds exactly one environment per
    // process, so `with_stdlib()`'s per-thread prototype cache would only
    // add a fork pass and retain a second copy of the world for the life of
    // the process. The result is identical either way.
    let env = Environment::with_stdlib_fresh();

    // By default every capability is on — the CLI is a developer tool, not
    // a sandbox.  `--sandbox` reverts to all-off, and `-c` can then grant
    // individual ones back.
    grant_capabilities(&args, &env);

    // Split the positional args into the script path and its arguments, and
    // expose the latter as *ARGV* (a list of strings; () when absent).
    let script = args.script.first().cloned();
    let argv = lamedh::LispVal::list(args.script.iter().skip(1).map(String::as_str));
    env.set("*ARGV*".to_string(), argv);

    // Batch mode (a script path or -s): a failed -i load is fatal (issue
    // #239) so CI and agent pipelines can trust the exit code.  In REPL mode
    // a failed load is reported but the session still starts.
    let batch = script.is_some() || !args.s.is_empty();

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
            eprintln!("{}", lamedh::format_error_with_backtrace(&e, &env));
            std::process::exit(1);
        }
        warn_on_overflow(&env, had_overflow);
        return;
    }

    // Execute s-expression(s) from -s flags (repeatable, shared env).
    if !args.s.is_empty() {
        for sexp in &args.s {
            let had_overflow = env.flag_set("OVERFLOW");
            match eval_all(sexp, &env) {
                Ok(results) => {
                    for r in results {
                        println!("{}", printer::print(&r));
                    }
                    warn_on_overflow(&env, had_overflow);
                }
                Err(e) => {
                    eprintln!("{}", lamedh::format_error_with_backtrace(&e, &env));
                    std::process::exit(1);
                }
            }
        }
        return;
    }

    // If no script and no -s flag, start REPL. Configure history (persisted
    // across sessions to ~/.lamedh_history) and tab-completion over every
    // symbol currently interned in the environment.
    let config = match rustyline::Config::builder().max_history_size(1000) {
        Ok(builder) => builder.build(),
        Err(e) => {
            eprintln!("Failed to configure line editor: {e}");
            std::process::exit(1);
        }
    };
    let mut rl = match rustyline::Editor::with_config(config) {
        Ok(editor) => editor,
        Err(e) => {
            eprintln!("Failed to initialize line editor: {e}");
            std::process::exit(1);
        }
    };
    rl.set_helper(Some(LispHelper { env: env.clone() }));

    let history_path = dirs::home_dir().map(|p| p.join(".lamedh_history"));
    if let Some(ref path) = history_path {
        let _ = rl.load_history(path);
    }

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
                // Persist eagerly, before evaluating: `(exit)` calls
                // `std::process::exit` from inside the evaluator and never
                // returns to the post-loop save, so a deferred-only save would
                // silently drop the whole session's history. Saving here also
                // survives a crash or non-local exit mid-eval.
                if let Some(ref path) = history_path {
                    let _ = rl.save_history(path);
                }
                let had_overflow = env.flag_set("OVERFLOW");
                match eval_all(&input, &env) {
                    Ok(results) => {
                        for r in results {
                            println!("{}", printer::print(&r));
                        }
                    }
                    Err(e) => println!("{}", lamedh::format_error_with_backtrace(&e, &env)),
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

    if let Some(ref path) = history_path {
        let _ = rl.save_history(path);
    }
}

#[cfg(test)]
mod tests {
    use super::{symbol_completions, token_start};

    #[test]
    fn token_start_finds_identifier_boundary() {
        assert_eq!(token_start("de", 2), 0);
        assert_eq!(token_start("(de", 3), 1);
        assert_eq!(token_start("(foo de", 7), 5);
        assert_eq!(token_start("(foo 'de", 8), 6); // quote is a delimiter
        assert_eq!(token_start("", 0), 0);
        // A multibyte char before an ASCII-boundary cursor must not panic.
        assert_eq!(token_start("(déf", "(déf".len()), 1);
    }

    #[test]
    fn completions_match_prefix_case_insensitively_and_lowercase() {
        let names = vec![
            "DEFUN".to_string(),
            "DEFMACRO".to_string(),
            "CAR".to_string(),
        ];
        let got = symbol_completions(names, "de");
        assert_eq!(got, vec!["defmacro".to_string(), "defun".to_string()]);
    }

    #[test]
    fn completions_skip_names_with_embedded_delimiters() {
        // A symbol made via (intern "foo bar") must never be offered — inserting
        // it would split the edit line into two tokens.
        let names = vec![
            "FOO BAR".to_string(), // embedded space
            "FOO".to_string(),
            "FOO(X".to_string(),  // embedded delimiter
            "FOO\"Q".to_string(), // embedded quote
        ];
        let got = symbol_completions(names, "foo");
        assert_eq!(got, vec!["foo".to_string()]);
    }

    #[test]
    fn completions_empty_when_no_match() {
        let names = vec!["CAR".to_string(), "CDR".to_string()];
        assert!(symbol_completions(names, "zzz").is_empty());
    }
}
