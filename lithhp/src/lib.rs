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
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum LispError {
    Generic(String),
}

impl fmt::Display for LispError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LispError::Generic(s) => write!(f, "{}", s),
        }
    }
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
    NumericEquals,
    MakeHashTable,
    Get,
    Set,
    DeleteKey,
    CurrentEnvironment,
    Keys,
    Atom,
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
pub struct Macro {
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
    Macro(Macro),
    Cons { car: Box<LispVal>, cdr: Box<LispVal> },
    Nil,
    HashTable(Rc<RefCell<HashMap<LispVal, LispVal>>>),
}

impl PartialEq for LispVal {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (LispVal::Symbol(a), LispVal::Symbol(b)) => a == b,
            (LispVal::Number(a), LispVal::Number(b)) => a == b,
            (LispVal::String(a), LispVal::String(b)) => a == b,
            (LispVal::Cons{car: car1, cdr: cdr1}, LispVal::Cons{car: car2, cdr: cdr2}) => car1 == car2 && cdr1 == cdr2,
            (LispVal::Nil, LispVal::Nil) => true,
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
            LispVal::Cons{car, cdr} => {
                car.hash(state);
                cdr.hash(state);
            }
            LispVal::Nil => 0.hash(state),
            LispVal::HashTable(h) => {
                // Hash the pointer address. This makes each hash table unique.
                Rc::as_ptr(h).hash(state);
            }
            LispVal::Builtin(_) | LispVal::Lambda(_) | LispVal::Fexpr(_) | LispVal::Macro(_) => {
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

use std::fs;

pub fn load_file(path: &str, env: &mut Environment) -> Result<(), LispError> {
    let content = fs::read_to_string(path)
        .map_err(|e| LispError::Generic(format!("Failed to read file {}: {}", path, e)))?;
    let expressions = reader::read_all(&content)
        .map_err(|e| LispError::Generic(format!("Failed to parse file {}: {}", path, e)))?;

    for expr in expressions {
        evaluator::eval(&expr, env)?;
    }
    Ok(())
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
