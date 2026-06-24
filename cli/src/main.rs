use clap::Parser;
use lamedh::{environment::Environment, eval_all, eval_line, load_directory, load_file, printer};
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
    // Use the embedded stdlib so the interpreter is self-contained. A lib/
    // directory on disk can still override or extend it via -i.
    let env = Environment::with_stdlib();

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
