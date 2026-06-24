pub mod environment;
pub mod evaluator;
pub mod optimizer;
pub mod printer;
pub mod reader;

pub use evaluator::{DEFAULT_EVAL_DEPTH_LIMIT, eval_depth_limit, set_eval_depth_limit};

/// Stack size used by [`with_large_stack`]. The tree-walking interpreter uses
/// large stack frames, so deep recursion needs substantial headroom (the
/// recursion-depth guard fires before this is exhausted; see issue #61).
pub const INTERPRETER_STACK_SIZE: usize = 512 * 1024 * 1024;

/// Run `f` on a freshly spawned thread with a large stack ([`INTERPRETER_STACK_SIZE`]).
///
/// The default thread stack (often 2–8 MiB) is too small for the interpreter's
/// large frames to recurse meaningfully. The CLI and the test harness use this;
/// embedders running the interpreter from a small-stack thread should too (or
/// lower [`set_eval_depth_limit`]). Because `LispVal`/`Environment` are `!Send`,
/// create the environment *inside* `f`.
pub fn with_large_stack<F, T>(f: F) -> T
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    std::thread::Builder::new()
        .stack_size(INTERPRETER_STACK_SIZE)
        .spawn(f)
        .expect("failed to spawn interpreter thread")
        .join()
        .expect("interpreter thread panicked")
}

use environment::Environment;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, Write};
use std::rc::Rc;

#[derive(Debug, Clone)]
pub enum LispError {
    Generic(String),
    Return(Box<LispVal>),
    Go(String),
}

impl PartialEq for LispError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (LispError::Generic(a), LispError::Generic(b)) => a == b,
            (LispError::Go(a), LispError::Go(b)) => a == b,
            (LispError::Return(v1), LispError::Return(v2)) => v1 == v2,
            _ => false,
        }
    }
}

impl fmt::Display for LispError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            LispError::Generic(s) => write!(f, "Error: {s}"),
            LispError::Return(_) => write!(f, "Internal LispError: RETURN used outside of PROG."),
            LispError::Go(_) => write!(f, "Internal LispError: GO used outside of PROG."),
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
    Print,
    GetP,
    PutP,
    Stringp,
    Apply,
    LoadFile,
    Lessp,
    Greaterp,
    Zerop,
    Remainder,
    Expt,
    Numberp,
    // I/O functions
    Read,
    Prin1,
    Princ,
    Terpri,
    // Error handling
    Error,
    Errorset,
    // List processing
    Subst,
    Sublis,
    Assoc,
    Maplist,
    Mapcar,
    Rplaca,
    Rplacd,
    // Bitwise operations
    Logor,
    Logand,
    Logxor,
    Leftshift,
    // Property list functions
    Remprop,
    Deflist,
    // Type predicates
    Fixp,
    Floatp,
    Symbolp,
    Boundp,
    Functionp,
    Macrop,
    // List functions
    List,
    Last,
    Nth,
    Nthcdr,
    Efface,
    // Numeric functions
    Mod,
    Plusp,
    Evenp,
    Oddp,
    Add1,
    Sub1,
    Random,
    // Bitwise
    Ash,
    Lognot,
    Rot,
    // Function operations
    Funcall,
    Macroexpand,
    // String/Symbol functions
    Explode,
    Implode,
    Maknam,
    Gensym,
    Intern,
    Plist,
    // Float comparisons
    FloatEqual,
    FloatLessp,
    FloatGreaterp,
    // Condition flags
    SetFlag,
    ClearFlag,
    FlagSetP,
    ClearAllFlags,
    // Capabilities / features
    EnableFeature,
    DisableFeature,
    FeatureEnabledP,
    Features,
    // Shell (gated behind the SHELL feature)
    Shell,
    // First-class environments
    MakeEnvironment,
    TheEnvironment,
    // Source optimizer
    Optimize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Symbol {
    pub name: String,
    pub plist: HashMap<String, LispVal>,
}

#[derive(Debug, Clone)]
pub struct Lambda {
    pub params: Vec<String>,
    pub rest_param: Option<String>,
    pub body: Box<LispVal>,
    pub env: Rc<Environment>,
}

impl PartialEq for Lambda {
    fn eq(&self, other: &Self) -> bool {
        self.params == other.params
            && self.rest_param == other.rest_param
            && self.body == other.body
            && Rc::ptr_eq(&self.env, &other.env)
    }
}

#[derive(Debug, Clone)]
pub struct Fexpr {
    pub params: Vec<String>,
    pub body: Box<LispVal>,
    pub env: Rc<Environment>,
}

impl PartialEq for Fexpr {
    fn eq(&self, other: &Self) -> bool {
        self.params == other.params && self.body == other.body && Rc::ptr_eq(&self.env, &other.env)
    }
}

#[derive(Debug, Clone)]
pub struct Macro {
    pub params: Vec<String>,
    pub rest_param: Option<String>,
    pub body: Box<LispVal>,
    pub env: Rc<Environment>,
}

impl PartialEq for Macro {
    fn eq(&self, other: &Self) -> bool {
        self.params == other.params
            && self.rest_param == other.rest_param
            && self.body == other.body
            && Rc::ptr_eq(&self.env, &other.env)
    }
}

/// The function signature for host-registered (native) Lisp callables.
pub type NativeFn = dyn Fn(&[LispVal], &Rc<Environment>) -> Result<LispVal, LispError>;

/// A Kernel-style vau operative closure.
///
/// When called, `operands_param` is bound to the **unevaluated** operand list
/// and `env_param` is bound to the **caller's lexical environment** as a
/// `LispVal::Environment`.  The body is then evaluated in the extended closure
/// env, which gives full access to both.
#[derive(Debug, Clone)]
pub struct Vau {
    pub operands_param: String,
    pub env_param: String,
    pub body: Box<LispVal>,
    pub env: Rc<Environment>,
}

impl PartialEq for Vau {
    fn eq(&self, other: &Self) -> bool {
        self.operands_param == other.operands_param
            && self.env_param == other.env_param
            && self.body == other.body
            && Rc::ptr_eq(&self.env, &other.env)
    }
}

pub enum LispVal {
    Symbol(Rc<RefCell<Symbol>>),
    Number(i64),
    Float(f64),
    String(String),
    Builtin(BuiltinFunc),
    Lambda(Lambda),
    Fexpr(Fexpr),
    Macro(Macro),
    Vau(Vau),
    Cons {
        car: Box<LispVal>,
        cdr: Box<LispVal>,
    },
    Nil,
    HashTable(Rc<RefCell<HashMap<LispVal, LispVal>>>),
    /// A host-registered Rust closure callable from Lisp.
    Native(Rc<NativeFn>),
    /// A first-class environment value.
    Environment(Rc<Environment>),
}

impl fmt::Debug for LispVal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LispVal::Symbol(s) => write!(f, "Symbol({:?})", s.borrow().name),
            LispVal::Number(n) => write!(f, "Number({n})"),
            LispVal::Float(v) => write!(f, "Float({v})"),
            LispVal::String(s) => write!(f, "String({s:?})"),
            LispVal::Builtin(b) => write!(f, "Builtin({b:?})"),
            LispVal::Lambda(_) => write!(f, "Lambda(...)"),
            LispVal::Fexpr(_) => write!(f, "Fexpr(...)"),
            LispVal::Macro(_) => write!(f, "Macro(...)"),
            LispVal::Vau(_) => write!(f, "Vau(...)"),
            LispVal::Cons { car, cdr } => write!(f, "Cons({car:?}, {cdr:?})"),
            LispVal::Nil => write!(f, "Nil"),
            LispVal::HashTable(_) => write!(f, "HashTable(...)"),
            LispVal::Native(_) => write!(f, "Native(...)"),
            LispVal::Environment(_) => write!(f, "Environment(...)"),
        }
    }
}

impl Clone for LispVal {
    fn clone(&self) -> Self {
        match self {
            LispVal::Symbol(s) => LispVal::Symbol(Rc::clone(s)),
            LispVal::Number(n) => LispVal::Number(*n),
            LispVal::Float(f) => LispVal::Float(*f),
            LispVal::String(s) => LispVal::String(s.clone()),
            LispVal::Builtin(b) => LispVal::Builtin(b.clone()),
            LispVal::Lambda(l) => LispVal::Lambda(l.clone()),
            LispVal::Fexpr(x) => LispVal::Fexpr(x.clone()),
            LispVal::Macro(m) => LispVal::Macro(m.clone()),
            LispVal::Vau(v) => LispVal::Vau(v.clone()),
            LispVal::Cons { car, cdr } => LispVal::Cons {
                car: car.clone(),
                cdr: cdr.clone(),
            },
            LispVal::Nil => LispVal::Nil,
            LispVal::HashTable(h) => LispVal::HashTable(Rc::clone(h)),
            LispVal::Native(f) => LispVal::Native(Rc::clone(f)),
            LispVal::Environment(e) => LispVal::Environment(Rc::clone(e)),
        }
    }
}

impl PartialEq for LispVal {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (LispVal::Symbol(a), LispVal::Symbol(b)) => Rc::ptr_eq(a, b),
            (LispVal::Number(a), LispVal::Number(b)) => a == b,
            (LispVal::Float(a), LispVal::Float(b)) => a == b,
            (LispVal::String(a), LispVal::String(b)) => a == b,
            (
                LispVal::Cons {
                    car: car1,
                    cdr: cdr1,
                },
                LispVal::Cons {
                    car: car2,
                    cdr: cdr2,
                },
            ) => car1 == car2 && cdr1 == cdr2,
            (LispVal::Nil, LispVal::Nil) => true,
            (LispVal::HashTable(a), LispVal::HashTable(b)) => Rc::ptr_eq(a, b),
            (LispVal::Builtin(a), LispVal::Builtin(b)) => a == b,
            (LispVal::Lambda(a), LispVal::Lambda(b)) => a == b,
            (LispVal::Fexpr(a), LispVal::Fexpr(b)) => a == b,
            (LispVal::Macro(a), LispVal::Macro(b)) => a == b,
            (LispVal::Vau(a), LispVal::Vau(b)) => a == b,
            (LispVal::Native(a), LispVal::Native(b)) => Rc::ptr_eq(a, b),
            (LispVal::Environment(a), LispVal::Environment(b)) => Rc::ptr_eq(a, b),
            _ => false,
        }
    }
}

impl Eq for LispVal {}

impl Hash for LispVal {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            LispVal::Symbol(s) => Rc::as_ptr(s).hash(state),
            LispVal::Number(n) => n.hash(state),
            LispVal::Float(f) => f.to_bits().hash(state),
            LispVal::String(s) => s.hash(state),
            LispVal::Cons { car, cdr } => {
                car.hash(state);
                cdr.hash(state);
            }
            LispVal::Nil => 0.hash(state),
            LispVal::HashTable(h) => {
                Rc::as_ptr(h).hash(state);
            }
            LispVal::Native(f) => {
                Rc::as_ptr(f).hash(state);
            }
            LispVal::Environment(e) => {
                Rc::as_ptr(e).hash(state);
            }
            LispVal::Builtin(_)
            | LispVal::Lambda(_)
            | LispVal::Fexpr(_)
            | LispVal::Macro(_)
            | LispVal::Vau(_) => {
                // Functions are not hashable by value.
            }
        }
    }
}

// ---------------------------------------------------------------------------
// From<T> for LispVal — infallible conversions from Rust primitives
// ---------------------------------------------------------------------------

impl From<i64> for LispVal {
    fn from(n: i64) -> Self {
        LispVal::Number(n)
    }
}

impl From<f64> for LispVal {
    fn from(f: f64) -> Self {
        LispVal::Float(f)
    }
}

impl From<bool> for LispVal {
    fn from(b: bool) -> Self {
        if b {
            LispVal::Symbol(Rc::new(RefCell::new(Symbol {
                name: "T".to_string(),
                plist: HashMap::new(),
            })))
        } else {
            LispVal::Nil
        }
    }
}

impl From<String> for LispVal {
    fn from(s: String) -> Self {
        LispVal::String(s)
    }
}

impl From<&str> for LispVal {
    fn from(s: &str) -> Self {
        LispVal::String(s.to_string())
    }
}

impl From<Vec<LispVal>> for LispVal {
    fn from(v: Vec<LispVal>) -> Self {
        v.into_iter()
            .rev()
            .fold(LispVal::Nil, |cdr, car| LispVal::Cons {
                car: Box::new(car),
                cdr: Box::new(cdr),
            })
    }
}

// ---------------------------------------------------------------------------
// TryFrom<LispVal> for Rust primitives — fallible extractions
// ---------------------------------------------------------------------------

impl TryFrom<LispVal> for i64 {
    type Error = LispError;
    fn try_from(val: LispVal) -> Result<Self, Self::Error> {
        match val {
            LispVal::Number(n) => Ok(n),
            other => Err(LispError::Generic(format!(
                "expected integer, got {other:?}"
            ))),
        }
    }
}

impl TryFrom<LispVal> for f64 {
    type Error = LispError;
    fn try_from(val: LispVal) -> Result<Self, Self::Error> {
        match val {
            LispVal::Float(f) => Ok(f),
            LispVal::Number(n) => Ok(n as f64),
            other => Err(LispError::Generic(format!(
                "expected number, got {other:?}"
            ))),
        }
    }
}

impl TryFrom<LispVal> for bool {
    type Error = LispError;
    fn try_from(val: LispVal) -> Result<Self, Self::Error> {
        Ok(val.is_truthy())
    }
}

impl TryFrom<LispVal> for String {
    type Error = LispError;
    fn try_from(val: LispVal) -> Result<Self, Self::Error> {
        match val {
            LispVal::String(s) => Ok(s),
            other => Err(LispError::Generic(format!(
                "expected string, got {other:?}"
            ))),
        }
    }
}

impl TryFrom<LispVal> for Vec<LispVal> {
    type Error = LispError;
    fn try_from(val: LispVal) -> Result<Self, Self::Error> {
        val.as_list_vec()
    }
}

// ---------------------------------------------------------------------------
// LispVal helpers
// ---------------------------------------------------------------------------

impl LispVal {
    /// Build a Lisp list from any iterator of items convertible to LispVal.
    pub fn list<I, T>(iter: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<LispVal>,
    {
        let items: Vec<LispVal> = iter.into_iter().map(Into::into).collect();
        LispVal::from(items)
    }

    /// Extract the integer value, or return an error.
    pub fn as_number(&self) -> Result<i64, LispError> {
        match self {
            LispVal::Number(n) => Ok(*n),
            other => Err(LispError::Generic(format!(
                "expected integer, got {other:?}"
            ))),
        }
    }

    /// Extract the float value (also accepts integers, coercing to f64).
    pub fn as_float(&self) -> Result<f64, LispError> {
        match self {
            LispVal::Float(f) => Ok(*f),
            LispVal::Number(n) => Ok(*n as f64),
            other => Err(LispError::Generic(format!(
                "expected number, got {other:?}"
            ))),
        }
    }

    /// Extract the string value, or return an error.
    pub fn as_str_val(&self) -> Result<&str, LispError> {
        match self {
            LispVal::String(s) => Ok(s.as_str()),
            other => Err(LispError::Generic(format!(
                "expected string, got {other:?}"
            ))),
        }
    }

    /// Collect a proper Lisp list into a Vec, or return an error if not a list.
    pub fn as_list_vec(&self) -> Result<Vec<LispVal>, LispError> {
        let mut result = Vec::new();
        let mut current = self;
        loop {
            match current {
                LispVal::Nil => break,
                LispVal::Cons { car, cdr } => {
                    result.push(*car.clone());
                    current = cdr;
                }
                _ => {
                    return Err(LispError::Generic(format!(
                        "not a proper list: dotted pair ending in {current:?}"
                    )));
                }
            }
        }
        Ok(result)
    }

    /// Lisp truthiness: everything except Nil is truthy.
    pub fn is_truthy(&self) -> bool {
        !matches!(self, LispVal::Nil)
    }
}

// ---------------------------------------------------------------------------
// Embedded standard library
// ---------------------------------------------------------------------------

/// The standard library sources embedded at compile time, in load order.
///
/// Each entry is `(filename, source)`. The filename is used only for error messages.
const STDLIB_SOURCES: &[(&str, &str)] = &[
    ("00-core.lisp", include_str!("../lib/00-core.lisp")),
    ("01-list.lisp", include_str!("../lib/01-list.lisp")),
    ("02-cxr.lisp", include_str!("../lib/02-cxr.lisp")),
    ("03-meta.lisp", include_str!("../lib/03-meta.lisp")),
    (
        "04-predicates.lisp",
        include_str!("../lib/04-predicates.lisp"),
    ),
    ("05-math.lisp", include_str!("../lib/05-math.lisp")),
    (
        "06-builtin-docs.lisp",
        include_str!("../lib/06-builtin-docs.lisp"),
    ),
    ("07-shell.lisp", include_str!("../lib/07-shell.lisp")),
    ("08-vau.lisp", include_str!("../lib/08-vau.lisp")),
    ("09-lisp15.lisp", include_str!("../lib/09-lisp15.lisp")),
    ("10-testing.lisp", include_str!("../lib/10-testing.lisp")),
    (
        "97-doc-renderer.lisp",
        include_str!("../lib/97-doc-renderer.lisp"),
    ),
    (
        "98-help-system.lisp",
        include_str!("../lib/98-help-system.lisp"),
    ),
    (
        "99-help-data.lisp",
        include_str!("../lib/99-help-data.lisp"),
    ),
];

/// Evaluate the embedded standard library into `env`.
///
/// This is called by [`Environment::with_stdlib`]; call it directly if you want
/// to load the stdlib into an environment you already have.
pub fn load_stdlib(env: &Rc<Environment>) -> Result<(), LispError> {
    for (filename, src) in STDLIB_SOURCES {
        let exprs = reader::read_all(src, env)
            .map_err(|e| LispError::Generic(format!("stdlib parse error in {filename}: {e}")))?;
        for expr in exprs {
            evaluator::eval(&expr, env)
                .map_err(|e| LispError::Generic(format!("stdlib eval error in {filename}: {e}")))?;
        }
    }
    Ok(())
}

/// Evaluate a single s-expression string, returning the typed value.
///
/// The input must contain exactly one form; use [`eval_all`] for programs with
/// multiple top-level forms.
pub fn eval_str(src: &str, env: &Rc<Environment>) -> Result<LispVal, LispError> {
    let form =
        reader::read(src, env).map_err(|e| LispError::Generic(format!("Parse error: {e}")))?;
    evaluator::eval(&form, env)
}

/// Evaluate all s-expressions in `src` and return the results.
///
/// Expressions are evaluated in order; if any expression fails the error is
/// returned immediately and subsequent expressions are not evaluated.
pub fn eval_all(src: &str, env: &Rc<Environment>) -> Result<Vec<LispVal>, LispError> {
    let forms =
        reader::read_all(src, env).map_err(|e| LispError::Generic(format!("Parse error: {e}")))?;
    forms
        .iter()
        .map(|form| evaluator::eval(form, env))
        .collect()
}

/// Evaluate a single line for REPL display, returning a printable string.
///
/// This is a thin wrapper over [`eval_str`] for use in the REPL; host code
/// should prefer [`eval_str`] or [`eval_all`] to get typed results.
pub fn eval_line(line: &str, env: &Rc<Environment>) -> String {
    match eval_str(line, env) {
        Ok(result) => printer::print(&result),
        Err(e) => format!("{e}"),
    }
}

pub fn load_file(path: &str, env: &Rc<Environment>) -> Result<(), LispError> {
    let content = fs::read_to_string(path)
        .map_err(|e| LispError::Generic(format!("Failed to read file {path}: {e}")))?;
    let expressions = reader::read_all(&content, env)
        .map_err(|e| LispError::Generic(format!("Failed to parse file {path}: {e}")))?;

    for expr in expressions {
        evaluator::eval(&expr, env)?;
    }
    Ok(())
}

pub fn load_directory(path: &str, env: &Rc<Environment>) -> Result<(), LispError> {
    let entries = std::fs::read_dir(path)
        .map_err(|e| LispError::Generic(format!("Failed to read directory {path}: {e}")))?;

    let mut files: Vec<_> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            let path = entry.path();
            path.is_file() && path.extension().map_or(false, |ext| ext == "lisp")
        })
        .collect();

    files.sort_by_key(|entry| entry.file_name());

    for entry in files {
        load_file(&entry.path().to_string_lossy(), env)?;
    }
    Ok(())
}

pub fn repl_loop<R: BufRead, W: Write>(
    in_stream: &mut R,
    out_stream: &mut W,
) -> std::io::Result<()> {
    let env = Environment::new_with_builtins();
    loop {
        let mut line = String::new();
        let bytes_read = in_stream.read_line(&mut line)?;

        if bytes_read == 0 {
            // End of stream
            break;
        }

        let line = line.trim_end();
        if line.is_empty() {
            continue;
        }

        let output = eval_line(line, &env);
        writeln!(out_stream, "{output}")?;
        out_stream.flush()?;
    }
    Ok(())
}
