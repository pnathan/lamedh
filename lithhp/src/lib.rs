pub mod reader;
pub mod printer;
pub mod environment;
pub mod evaluator;

use std::io::{BufRead, Write};
use environment::Environment;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::cell::RefCell;

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
    Eq,
    Not,
    MakeHashTable,
    Get,
    Set,
    DeleteKey,
    CurrentEnvironment,
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

#[derive(Debug, Clone)]
pub enum LispVal {
    Symbol(String),
    Number(i64),
    String(String),
    Builtin(BuiltinFunc),
    Lambda(Lambda),
    Fexpr(Fexpr),
    List(Vec<LispVal>),
    HashTable(Rc<RefCell<HashMap<LispVal, LispVal>>>),
}

impl PartialEq for LispVal {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (LispVal::Symbol(a), LispVal::Symbol(b)) => a == b,
            (LispVal::Number(a), LispVal::Number(b)) => a == b,
            (LispVal::String(a), LispVal::String(b)) => a == b,
            (LispVal::List(a), LispVal::List(b)) => a == b,
            (LispVal::HashTable(a), LispVal::HashTable(b)) => Rc::ptr_eq(a, b),
            (LispVal::Builtin(a), LispVal::Builtin(b)) => a == b,
            (LispVal::Lambda(_), LispVal::Lambda(_)) => false,
            (LispVal::Fexpr(_), LispVal::Fexpr(_)) => false,
            _ => false,
        }
    }
}

impl Eq for LispVal {}

impl Hash for LispVal {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            LispVal::Symbol(s) => s.hash(state),
            LispVal::Number(n) => n.hash(state),
            LispVal::String(s) => s.hash(state),
            LispVal::List(l) => l.hash(state),
            LispVal::HashTable(h) => {
                // Hash the pointer address. This makes each hash table unique.
                Rc::as_ptr(h).hash(state);
            }
            LispVal::Builtin(_) | LispVal::Lambda(_) | LispVal::Fexpr(_) => {
                // Functions are not hashable.
            }
        }
    }
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
