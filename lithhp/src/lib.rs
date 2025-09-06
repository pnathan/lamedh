pub mod reader;
pub mod printer;
pub mod environment;
pub mod evaluator;

use std::io::{BufRead, Write};
use environment::Environment;

#[derive(Debug, Clone, PartialEq)]
pub enum LispError {
    Generic(String),
}

#[derive(Debug, Clone, PartialEq)]
pub enum BuiltinFunc {
    Plus,
    Minus,
    Multiply,
    Divide,
    Car,
    Cdr,
    Cons,
    Concat,
    Index,
    Eval,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Lambda {
    pub params: Vec<String>,
    pub body: Box<LispVal>,
    pub env: Environment,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Fexpr {
    pub params: Vec<String>,
    pub body: Box<LispVal>,
    pub env: Environment,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LispVal {
    Symbol(String),
    Number(i64),
    String(String),
    Builtin(BuiltinFunc),
    Lambda(Lambda),
    Fexpr(Fexpr),
    List(Vec<LispVal>),
}

pub fn eval_line(line: &str, env: &mut Environment) -> String {
    match reader::read(line) {
        Ok(lisp_val) => match evaluator::eval(&lisp_val, env) {
            Ok(result) => printer::print(&result),
            Err(e) => format!("Error: {:?}", e),
        },
        Err(e) => format!("Error: {e}"),
    }
}

pub fn repl_loop<R: BufRead, W: Write>(in_stream: &mut R, out_stream: &mut W) -> std::io::Result<()> {
    let mut env = Environment::new_with_builtins();
    loop {
        let mut line = String::new();
        let bytes_read = in_stream.read_line(&mut line)?;

        if bytes_read == 0 { // End of stream
            break;
        }

        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }

        let output = eval_line(line, &mut env);
        writeln!(out_stream, "{output}")?;
        out_stream.flush()?;
    }
    Ok(())
}
