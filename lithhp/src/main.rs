use clap::Parser;
use lithhp::{environment::Environment, eval_line, load_file};
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

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

    let mut env = Environment::new_with_builtins();

    // Load prologue.lisp if it exists
    if let Err(e) = load_file("prologue.lisp", &mut env) {
        // It's okay if it doesn't exist, but print an error if it fails for other reasons
        if !e.to_string().contains("No such file or directory") {
            eprintln!("Error loading prologue.lisp: {:?}", e);
        }
    }

    // Load files from -i flag
    for file in args.i {
        if let Err(e) = load_file(&file, &mut env) {
            eprintln!("Error loading file {}: {:?}", file, e);
            return;
        }
    }

    // Execute s-expression from -s flag
    if let Some(sexp) = args.s {
        let output = eval_line(&sexp, &mut env);
        println!("{output}");
        return;
    }

    // If no -s flag, start REPL
    let mut rl = DefaultEditor::new().unwrap();
    println!("Lithhp Lisp 1.5");
    println!("Press Ctrl+C or Ctrl+D to exit");

    loop {
        let readline = rl.readline("lithhp> ");
        match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_str()).unwrap();
                let output = eval_line(&line, &mut env);
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
