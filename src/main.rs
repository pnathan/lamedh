use clap::Parser;
use lamedh::{environment::Environment, eval_line, load_directory, load_file};
use rustyline::DefaultEditor;
use rustyline::error::ReadlineError;
use std::fs;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long, action = clap::ArgAction::Append)]
    i: Vec<String>,

    #[arg(short, long)]
    s: Option<String>,
}

fn main() {
    let args = Args::parse();
    // The tree-walking interpreter uses large stack frames; run it on a big
    // stack so reasonable recursion depths work, while the depth guard (#61)
    // turns runaway recursion into a recoverable error instead of an abort.
    lamedh::with_large_stack(move || run(args));
}

fn run(args: Args) {
    let env = Environment::new_with_builtins();

    // Load prologue.lisp if it exists
    if let Err(e) = load_file("prologue.lisp", &env) {
        // It's okay if it doesn't exist, but print an error if it fails for other reasons
        if !e.to_string().contains("No such file or directory") {
            eprintln!("Error loading prologue.lisp: {e:?}");
        }
    }

    // Load lib/ directory if it exists
    if let Err(e) = load_directory("lib", &env) {
        if !e.to_string().contains("Failed to read directory") {
            eprintln!("Error loading lib/: {e:?}");
        }
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
        } else if metadata.is_file() {
            if let Err(e) = load_file(&path, &env) {
                eprintln!("Error loading file {path}: {e:?}");
            }
        }
    }

    // Execute s-expression from -s flag
    if let Some(sexp) = args.s {
        let output = eval_line(&sexp, &env);
        println!("{output}");
        return;
    }

    // If no -s flag, start REPL
    let mut rl = DefaultEditor::new().unwrap();
    println!("Lamed (ל) Lisp 1.5");
    println!("Press Ctrl+C or Ctrl+D to exit");

    loop {
        let readline = rl.readline(r"(ל)> ");
        match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_str()).unwrap();
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
