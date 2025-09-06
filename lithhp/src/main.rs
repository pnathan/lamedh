use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use lithhp::{eval_line, environment::Environment};

fn main() {
    let mut env = Environment::new_with_builtins();
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
