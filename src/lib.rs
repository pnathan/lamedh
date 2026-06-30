//! # Lamedh — an embeddable Lisp 1.5 interpreter
//!
//! Lamedh (Hebrew: ל, "Lamed") is a complete, embeddable Lisp 1.5 interpreter
//! written in Rust.  It provides:
//!
//! - A tree-walking evaluator with trampolining tail-call optimisation (TCO)
//! - Lexical closures, macros, fexprs, and Kernel-style vau operatives
//! - Both lexical and dynamic (special) variable scoping
//! - An extensible type system via [`LispValExtension`]
//! - A capability-gated sandbox (filesystem, shell, stdin — all off by default)
//! - A full standard library embedded in the binary (no `.lisp` files needed)
//!
//! ## Quick start
//!
//! Add to `Cargo.toml`:
//!
//! ```toml
//! [dependencies]
//! lamedh = "0.2"
//! ```
//!
//! Evaluate Lisp expressions from Rust:
//!
//! ```rust,ignore
//! use lamedh::{eval_str, LispVal};
//! use lamedh::environment::Environment;
//!
//! lamedh::with_large_stack(|| {
//!     let env = Environment::with_stdlib();
//!
//!     // Evaluate a simple expression
//!     let val = eval_str("(+ 1 2 3)", &env).unwrap();
//!     assert_eq!(val, LispVal::Number(6));
//!
//!     // Define a function and call it
//!     eval_str("(defun factorial (n) (if (zerop n) 1 (* n (factorial (sub1 n)))))", &env).unwrap();
//!     let result = eval_str("(factorial 10)", &env).unwrap();
//!     assert_eq!(result, LispVal::Number(3628800));
//! });
//! ```
//!
//! ## Stack size
//!
//! The tree-walking evaluator recurses in Rust for non-tail calls.  The default
//! system thread stack (2–8 MiB) overflows before the recursion-depth guard
//! fires.  Always wrap the interpreter entry point in [`with_large_stack`], which
//! spawns a 512 MiB thread.  Because [`LispVal`] and [`environment::Environment`]
//! are `!Send`, create the environment *inside* the closure.
//!
//! ## Embedding pattern
//!
//! ```rust,ignore
//! use lamedh::{eval_str, LispError, LispVal};
//! use lamedh::environment::Environment;
//!
//! fn run_script(src: &str) -> Result<LispVal, LispError> {
//!     lamedh::with_large_stack(move || {
//!         let env = Environment::with_stdlib();
//!
//!         // Register a host function
//!         env.register_fn("greet", |args, _env| {
//!             let name = args[0].as_str_val()?;
//!             Ok(LispVal::from(format!("Hello, {name}!")))
//!         });
//!
//!         eval_str(src, &env)
//!     })
//! }
//! ```
//!
//! ## Sandboxing
//!
//! All potentially dangerous capabilities are disabled by default.  Use
//! [`environment::Environment::enable_feature`] to opt in:
//!
//! ```rust,ignore
//! // Allow scripts to call (shell "ls")
//! env.enable_feature("SHELL");
//!
//! // Allow read-only filesystem access: read-file, file-exists-p, directory-files, …
//! env.enable_feature("READ-FS");
//!
//! // Allow filesystem mutations: write-file, chmod, create-directory, delete-file, rename-file
//! env.enable_feature("CREATE-FS");
//!
//! // Allow temp-file creation: make-temp-file, make-temp-directory
//! env.enable_feature("TEMP-FS");
//!
//! // Allow (read) to consume stdin
//! env.enable_feature("IO");
//! ```
//!
//! ## Module structure
//!
//! | Module | Role |
//! |--------|------|
//! | [`environment`] | Variable binding, symbol interning, dynamic scoping |
//! | [`reader`] | nom-based s-expression parser |
//! | [`evaluator`] | Core eval loop with TCO, special forms, 100+ builtins |
//! | [`printer`] | Format [`LispVal`] back to readable text |
//! | [`optimizer`] | Constant-folding source-level optimizer |
//!
//! ## Language overview
//!
//! Lamedh is a Lisp 1.5 dialect with modern extensions:
//!
//! | Feature | Lisp form |
//! |---------|-----------|
//! | Variable binding | `(def x 42)`, `(setq x 99)` |
//! | Function definition | `(defun square (n) (* n n))` |
//! | Conditionals | `(if p a b)`, `(cond (p1 e1) (p2 e2))` |
//! | Local scope | `(let ((x 1) (y 2)) (+ x y))` |
//! | Closures | `(lambda (x) (* x x))` |
//! | Tail-recursive loops | `(defun loop (n) (if (zerop n) t (loop (sub1 n))))` |
//! | Macros | `(defmacro when (p &rest b) \`(if ,p (progn ,@b) nil))` |
//! | Fexprs | `(defexpr my-quote (args) (car args))` |
//! | Kernel vau | `(vau (ops e) (eval (car ops) e))` |
//! | Hash tables | `(let ((h (make-hash-table))) (set-bang h 'x 1) (gethash h 'x))` |
//! | Arrays | `(let ((a (array 5))) (store a 0 99) (fetch a 0))` |
//! | Property lists | `(putp 'foo "doc" "A foo.") (getp 'foo "doc")` |
//! | Structs | `(defstruct point x y) (make-point :x 1 :y 2)` |
//!
//! ## Cargo manifest
//!
//! ```toml
//! [package]
//! name        = "lamedh"
//! version     = "0.2.0"
//! edition     = "2024"
//! description = "An embeddable Lisp 1.5 interpreter written in Rust"
//! license     = "AGPL-3.0"
//!
//! [lib]
//! name = "lamedh"
//! path = "src/lib.rs"
//!
//! [dependencies]
//! nom = "=7.1.3"   # pinned reader dependency
//!
//! # Default typed-JIT backend. Disable default features for the
//! # dependency-light typed checker / closure interpreter path.
//! cranelift-jit      = { version = "0.133", optional = true }
//! cranelift-module   = { version = "0.133", optional = true }
//! cranelift-codegen  = { version = "0.133", optional = true }
//! cranelift-frontend = { version = "0.133", optional = true }
//!
//! [features]
//! default = ["jit"]
//! jit = [
//!     "dep:cranelift-jit",
//!     "dep:cranelift-module",
//!     "dep:cranelift-codegen",
//!     "dep:cranelift-frontend",
//! ]
//!
//! [workspace]
//! members         = ["cli"]
//! default-members = [".", "cli"]
//! # Benchmark comparison crates (benchmarks/*/rust) are excluded from the
//! # workspace and built directly by benchmarks/run_benchmarks.sh.
//! ```
//!
//! The dependency-light library build uses `nom` (parser combinators for the
//! reader). The default 0.2.x build also enables the typed JIT's Cranelift
//! backend; use `--no-default-features` to omit it. All I/O is gated behind
//! capability flags so embedders get a sandboxed interpreter by default.
//!
//! The companion binary crate `lamedh-cli` (in `cli/`) adds `rustyline` (REPL
//! line-editing) and `clap` (argument parsing) but does not affect the library.
//!
//! ## Standard library
//!
//! [`environment::Environment::with_stdlib`] loads the following modules in order,
//! all embedded in the binary at compile time (no `.lisp` files are needed at
//! runtime):
//!
//! | Module | Key functions |
//! |--------|--------------|
//! | `00-core.lisp` | `DEFUN`, `PROG2`, `CSET`, `CSETQ` |
//! | `01-list.lisp` | `APPEND`, `MEMBER`, `LENGTH`, `REVERSE`, `PAIRLIS`, `NULL`, `NCONC`, `COPY`, `SASSOC`, `MAPC`, `MAPCON` |
//! | `02-cxr.lisp` | All 30 CAR/CDR compositions `CAAR`…`CDDDDR` |
//! | `03-meta.lisp` | `DOCUMENTATION` |
//! | `04-predicates.lisp` | `EQUAL` (structural equality) |
//! | `05-math.lisp` | `ONEP`, `MINUSP`, `ADD1`, `SUB1`, `MAX`, `MIN`, `ABS` |
//! | `07-shell.lisp` | `SH`, `SHELL-STDOUT`, `SHELL-STDERR`, `SHELL-EXIT-CODE`, `SHELL-OK-P` |
//! | `08-vau.lisp` | Kernel-style derived forms: `$IF`, `$AND`, `$OR`, `$SEQUENCE` |
//! | `09-lisp15.lisp` | Lisp 1.5 appendix A: `PAIR`, `ATTRIB`, `PROP`, `FLAG`, `REMFLAG`, `MAP`, `SEARCH`, `RECIP`, `SELECT`, `TRACE` |
//! | `10-testing.lisp` | xUnit framework: `DEFTEST`, `ASSERT-EQUAL`, `ASSERT-TRUE`, `ASSERT-FALSE`, `ASSERT-NIL`, `RUN-TESTS`, `CLEAR-TESTS` |
//! | `11-optimizer-vau.lisp` | Source optimizer: `OPTIMIZE-FORM`, `$OPT` |
//! | `97-doc-renderer.lisp` | REPL documentation renderer |
//! | `98-help-system.lisp` | `(HELP)`, `(HELP 'fn)`, `(HELP 'categories)` |
//! | `99-help-data.lisp` | Structured documentation database for all built-ins |
//!
//! Shell helpers (`07-shell.lisp`) require the `SHELL` capability to be granted
//! by the host (`env.enable_feature("SHELL")`) or CLI (`--capability SHELL`).
//! The testing framework (`10-testing.lisp`) is automatically available when
//! you call [`environment::Environment::with_stdlib`].

pub mod environment;
pub mod evaluator;
pub mod jit;
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
use std::path::{Path, PathBuf};
use std::rc::Rc;

/// Trait for host-defined value types that can participate in the Lisp system.
///
/// Implement this trait on your Rust type and wrap it with
/// `LispVal::ext(value)` to make it available to Lisp code.
///
/// ```rust,ignore
/// use lamedh::{LispVal, LispValExtension, LispError};
///
/// struct MyPoint { x: f64, y: f64 }
///
/// impl LispValExtension for MyPoint {
///     fn type_name(&self) -> &str { "point" }
///     fn display(&self) -> String { format!("#<point {},{}>", self.x, self.y) }
///     fn eq_ext(&self, other: &dyn LispValExtension) -> bool {
///         other.as_any().downcast_ref::<MyPoint>()
///             .map_or(false, |p| p.x == self.x && p.y == self.y)
///     }
///     fn hash_ext(&self, state: &mut dyn std::hash::Hasher) {
///         self.x.to_bits().hash(state);
///         self.y.to_bits().hash(state);
///     }
///     fn as_any(&self) -> &dyn std::any::Any { self }
/// }
///
/// let pt = LispVal::ext(MyPoint { x: 1.0, y: 2.0 });
/// ```
pub trait LispValExtension: fmt::Debug {
    /// Short type tag shown in error messages and the printer.
    fn type_name(&self) -> &str;
    /// Human-readable representation (used by PRINT/PRIN1).
    fn display(&self) -> String;
    /// Value equality with another extension value.
    fn eq_ext(&self, other: &dyn LispValExtension) -> bool;
    /// Hash for use as a hash-table key.
    fn hash_ext(&self, state: &mut dyn Hasher);
    /// Downcast support — return `self` as `Any`.
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Runtime error type for the Lisp interpreter.
///
/// Most user-visible errors are [`LispError::Generic`].  The other variants are
/// internal control-flow signals used by `PROG`/`RETURN`/`GO` and should never
/// escape a top-level [`eval_str`] call under normal circumstances; if they do,
/// treat them as bugs.
#[derive(Debug, Clone)]
pub enum LispError {
    /// A runtime error with a human-readable message.
    Generic(String),
    /// Carries a value out of a `PROG` form via `(RETURN val)`.
    /// Not a true error; caught by the `PROG` evaluator.
    Return(Box<LispVal>),
    /// Carries a label name out of a `PROG` form via `(GO label)`.
    /// Not a true error; caught by the `PROG` evaluator.
    Go(String),
    /// Carries a value out of a `CATCH` form via `(THROW tag value)`.
    /// Not a true error; caught by the matching `CATCH` (compared by tag value).
    Throw {
        tag: Box<LispVal>,
        value: Box<LispVal>,
    },
    /// Carries a value out of a `BLOCK` via `(RETURN-FROM name value)`.
    /// Not a true error; caught by the matching named `BLOCK`.
    ReturnFrom { name: String, value: Box<LispVal> },
    /// A signalled first-class condition value (typically a [`LispVal::Error`]).
    /// Raised by `(error ...)`; trapped by `ERRORSET` (→ NIL) and bound by the
    /// handler variable in `HANDLER-CASE`.
    Signaled(Box<LispVal>),
}

impl PartialEq for LispError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (LispError::Generic(a), LispError::Generic(b)) => a == b,
            (LispError::Go(a), LispError::Go(b)) => a == b,
            (LispError::Return(v1), LispError::Return(v2)) => v1 == v2,
            (LispError::Signaled(a), LispError::Signaled(b)) => a == b,
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
            LispError::Throw { .. } => {
                write!(f, "Internal LispError: THROW with no matching CATCH.")
            }
            LispError::ReturnFrom { .. } => {
                write!(f, "Internal LispError: RETURN-FROM with no matching BLOCK.")
            }
            LispError::Signaled(v) => match v.as_ref() {
                LispVal::Error(e) => write!(f, "Error: {}", e.message),
                other => write!(f, "Error: {}", crate::printer::print(other)),
            },
        }
    }
}

/// Discriminants for the built-in primitive functions.
///
/// Each variant corresponds to a Lisp function registered in
/// [`environment::Environment::new_with_builtins`].  The mapping from Lisp name
/// to variant is defined there; the actual implementation lives in
/// `evaluator::apply_builtin`.
///
/// You do not normally construct `BuiltinFunc` values directly — they are
/// created by the environment initialiser and stored as [`LispVal::Builtin`].
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
    Charp,
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
    // Capabilities / features (read-only from Lisp; grant/revoke only from the host)
    FeatureEnabledP,
    Features,
    // Shell (gated behind the SHELL feature)
    Shell,
    // First-class environments
    MakeEnvironment,
    TheEnvironment,
    // Source optimizer
    Optimize,
    // Lisp 1.5 EVLIS
    Evlis,
    // Arrays (Lisp 1.5 Appendix A)
    MakeArray,
    ArrayFetch,
    ArrayStore,
    ArrayLength,
    Length,
    ListToArray,
    ArrayToList,
    Arrayp,
    Extensionp,
    ExtensionTypeName,
    // Lisp 1.5 EVCON
    Evcon,
    // I/O
    Spaces,
    // File I/O (gated behind READ-FS / CREATE-FS capability)
    ReadFile,
    ReadFileByte,
    ReadFileSection,
    WriteFile,
    // File metadata predicates (gated behind READ-FS capability)
    FileExistsP,
    DirectoryP,
    FileP,
    FileReadableP,
    FileWritableP,
    FileExecutableP,
    FileSize,
    DirectoryFiles,
    FileNewerP,
    // File mutation (gated behind CREATE-FS capability)
    Chmod,
    CreateDirectory,
    DeleteFile,
    RenameFile,
    // Temp filesystem (gated behind TEMP-FS capability)
    MakeTempFile,
    MakeTempDirectory,
    // Sorting (stable, non-destructive; takes a comparator predicate)
    Sort,
    // Math library (f64 transcendentals, float->int rounding, i64 integer math)
    Sqrt,
    Sin,
    Cos,
    Tan,
    Log,
    Exp,
    Floor,
    Ceiling,
    Round,
    Truncate,
    Gcd,
    Lcm,
    Isqrt,
    Signum,
    // String operations (kernel primitives not expressible in pure Lisp)
    StringLength,
    Substring,
    CharCode,
    CodeChar,
    MakeChar,
    StringToNumber,
    NumberToString,
    // Value -> string rendering (backs FORMAT and friends)
    Prin1ToString,
    PrincToString,
    // First-class error/condition values
    MakeError,
    ErrorP,
    ErrorMessage,
    ErrorData,
    // Introspection
    Describe,
    SeeSource,
    Disassemble,
    // Concurrency primitives (gated behind the `concurrency` feature)
    #[cfg(feature = "concurrency")]
    MakeChannel,
    #[cfg(feature = "concurrency")]
    ChannelSend,
    #[cfg(feature = "concurrency")]
    ChannelRecv,
    #[cfg(feature = "concurrency")]
    ChannelRecvTimeout,
    #[cfg(feature = "concurrency")]
    CloneInterpreter,
}

/// An interned Lisp symbol.
///
/// Symbols are the atomic identifiers of Lisp programs — names like `FOO`,
/// `+`, `CAR`, or `*MY-VAR*`.  Every distinct name is stored exactly once in
/// the [`environment::SymbolTable`]; two occurrences of the same name share the
/// same `Rc<RefCell<Symbol>>` allocation, making [`LispVal`] `EQ` comparison
/// a pointer-equality test.
///
/// Each symbol carries a **property list** (`plist`) — a `HashMap` of
/// indicator→value pairs.  The standard library uses `"docstring"` to store
/// documentation accessible via `(documentation 'sym)`.  You can store
/// arbitrary metadata from Rust via [`environment::Environment::intern_symbol`]
/// and then mutate `sym.borrow_mut().plist`.
#[derive(Debug, Clone, PartialEq)]
pub struct Symbol {
    /// Uppercased canonical name.
    pub name: String,
    /// Arbitrary key/value metadata attached to this symbol.
    pub plist: HashMap<String, LispVal>,
    /// Global value cell (the canonical-namespace binding for this symbol).
    ///
    /// Root-level (global) bindings live here rather than in a `HashMap` on the
    /// root environment frame, so a global/function reference resolves by reading
    /// the cell on the symbol the AST already holds — O(1), no hash, no chain
    /// walk. Local bindings (params, `let`, `prog`, loop vars) still live in
    /// their frame's map. Because each interpreter chain has its own
    /// `SymbolTable`, these cells are naturally scoped per global namespace, so
    /// independent `make-environment` namespaces stay isolated.
    pub value: Option<LispVal>,
}

/// A lexical closure created by `(lambda (params…) body…)` or `defun`.
///
/// When called, a new child environment is created whose lexical parent is
/// `env` (the definition site) and whose dynamic parent is the caller's
/// environment (so dynamic/special variables propagate correctly).  Parameters
/// are bound in this child env before `body` is evaluated.
///
/// If `rest_param` is `Some(name)`, any arguments beyond the fixed `params`
/// are collected into a list and bound to that name (the `&REST` convention).
#[derive(Debug, Clone)]
pub struct Lambda {
    /// Fixed parameter names, in order.
    pub params: Vec<String>,
    /// Optional variadic parameter name (`&REST rest`).
    pub rest_param: Option<String>,
    /// Body expression (typically `(PROGN ...)` for multi-form bodies).
    pub body: Box<LispVal>,
    /// Captured lexical environment (definition site).
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

/// A fexpr ("function expression") created by `(defexpr name (args-sym) body…)`.
///
/// Unlike a lambda, a fexpr receives its arguments **unevaluated** — the
/// entire argument list is passed as a single `LispVal` list bound to the
/// name in `params[0]`.  The fexpr body can then selectively call `(eval …)`
/// on whichever arguments it chooses.
///
/// Fexprs are the classic Lisp mechanism for user-defined special forms.
/// They differ from macros in that they compute a *value* directly rather
/// than returning code to be evaluated.
#[derive(Debug, Clone)]
pub struct Fexpr {
    /// Single-element vec containing the name for the unevaluated arg list.
    pub params: Vec<String>,
    /// Body expression.
    pub body: Box<LispVal>,
    /// Captured lexical environment.
    pub env: Rc<Environment>,
}

impl PartialEq for Fexpr {
    fn eq(&self, other: &Self) -> bool {
        self.params == other.params && self.body == other.body && Rc::ptr_eq(&self.env, &other.env)
    }
}

/// A macro created by `(defmacro name (params…) body…)`.
///
/// Macros receive their arguments **unevaluated** and return a Lisp form
/// (the *expansion*) which is then evaluated in the caller's environment.
/// This two-phase design (expand-then-eval) distinguishes macros from fexprs.
///
/// Supports `&REST` for variadic parameter lists, enabling patterns like
/// `(defmacro when (test &rest body) \`(if ,test (progn ,@body) nil))`.
#[derive(Debug, Clone)]
pub struct Macro {
    /// Fixed parameter names.
    pub params: Vec<String>,
    /// Optional `&REST` parameter for remaining arguments.
    pub rest_param: Option<String>,
    /// Body expression; evaluating it must produce a valid Lisp form.
    pub body: Box<LispVal>,
    /// Captured lexical environment (definition site).
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

// NOTE: the crate-level doc block is at the very top of this file (//! lines).
// Additional sections are appended here so that rustdoc renders them in order.

/// The universal value type of the Lamedh Lisp interpreter.
///
/// Every Lisp object — numbers, strings, lists, functions, hash tables — is
/// represented as a `LispVal`.  The variants map directly to the types defined
/// in the Lisp 1.5 manual, extended with modern additions.
///
/// ## Memory model
///
/// - **Cons cells** use `Rc<LispVal>` (not `Box`) so sharing a list head is
///   O(1) refcount bump.  Cons cells are immutable; `RPLACA`/`RPLACD` return
///   new cells rather than mutating in place.
/// - **Symbols** are interned: two occurrences of `FOO` share one
///   `Rc<RefCell<Symbol>>`.  [`LispVal::eq`] for symbols uses pointer equality,
///   matching the Lisp `EQ` predicate.
/// - **Hash tables**, **arrays**, **environments**, and **natives** all use `Rc`
///   for shared ownership.
///
/// ## Truthiness
///
/// [`LispVal::Nil`] is the only falsy value.  Everything else — including `0`,
/// `""`, and empty string — is truthy.  Use [`LispVal::is_truthy`] in Rust code
/// and `(not x)` in Lisp code.
///
/// ## Conversion
///
/// Use the `From<T>` impls for Rust→Lisp conversion and `TryFrom<LispVal>` for
/// Lisp→Rust:
///
/// ```rust,ignore
/// let n: LispVal = LispVal::from(42i64);         // Number(42)
/// let s: LispVal = LispVal::from("hello");        // String("hello")
/// let lst = LispVal::list([1i64, 2, 3]);          // (1 2 3)
///
/// let x: i64 = i64::try_from(n)?;                // 42
/// let v: Vec<LispVal> = Vec::try_from(lst)?;     // [Number(1), Number(2), Number(3)]
/// ```
pub enum LispVal {
    /// An interned symbol such as `FOO`, `+`, or `*MY-VAR*`.
    Symbol(Rc<RefCell<Symbol>>),
    /// A 64-bit signed integer.
    Number(i64),
    /// A single byte (character, 0–255). Produced by char literals (`'a'` →
    /// `Char(97)`). Coerces to `Number` in arithmetic, like C integer
    /// promotion. Distinct from `Number`: `charp` tests for this type;
    /// `fixp` does not match it.
    Char(u8),
    /// A 64-bit IEEE 754 floating-point number.
    Float(f64),
    /// A UTF-8 string literal.
    String(String),
    /// A built-in primitive function.  See [`BuiltinFunc`].
    Builtin(BuiltinFunc),
    /// A lexical closure.  See [`Lambda`].
    ///
    /// Boxed: closures are far larger than the other variants, so storing them
    /// inline would make every `LispVal` (even a `Number`) as wide as a
    /// `Lambda`. Boxing keeps `LispVal` small, which is a large win because the
    /// evaluator clones and moves `LispVal`s constantly (see the perf work in
    /// the boxing pass). The same reasoning applies to `Fexpr`/`Macro`/`Vau`.
    Lambda(Box<Lambda>),
    /// A fexpr (unevaluated-argument function).  See [`Fexpr`].
    Fexpr(Box<Fexpr>),
    /// A macro (code-returning function).  See [`Macro`].
    Macro(Box<Macro>),
    /// A Kernel-style vau operative.  See [`Vau`].
    Vau(Box<Vau>),
    /// A cons cell.  Children are `Rc` (not `Box`) so cloning a list is an
    /// O(1) refcount bump instead of a deep copy — cons cells are immutable in
    /// this implementation (`rplaca`/`rplacd` return new cells), so structural
    /// sharing is sound.
    Cons { car: Rc<LispVal>, cdr: Rc<LispVal> },
    /// The empty list / boolean false.  `()` and `NIL` are the same object.
    Nil,
    /// A mutable hash table, used as `(make-hash-table)`.  Keys and values are
    /// arbitrary `LispVal`s.
    HashTable(Rc<RefCell<HashMap<LispVal, LispVal>>>),
    /// A host-registered Rust closure callable from Lisp.  See
    /// [`environment::Environment::register_fn`].
    Native(Rc<NativeFn>),
    /// A first-class environment.  Obtained via `(the-environment)` or
    /// `(make-environment)`.  Can be passed to `(eval expr env)`.
    Environment(Rc<Environment>),
    /// A 0-indexed mutable vector (Lisp 1.5 `array`).  Created by `(array n)`;
    /// accessed with `(fetch a i)` / `(store a i v)`.
    Array(Rc<RefCell<Vec<LispVal>>>),
    /// A typed, nominal struct value crossing the typed membrane.
    Struct(Rc<StructObj>),
    /// A host-defined extension value.  Use [`LispVal::ext`] to construct.
    /// See [`LispValExtension`].
    Extension(Rc<dyn LispValExtension>),
    /// A first-class error/condition value: a message plus an optional data
    /// payload (a cons list of "irritants", or `Nil`).  Constructed with
    /// `(make-error msg data)`, signalled by `(error ...)`, and bound by the
    /// handler variable in `(handler-case ...)`.  Boxed behind an `Rc` so the
    /// `LispVal` stays small.
    Error(Rc<ErrorObj>),
    /// A message-passing channel (only present under the `concurrency` feature).
    ///
    /// Created with `(make-channel)`.  Both ends are bundled together so that
    /// one side can be cloned off and used from another evaluation context.
    /// Values cross the channel boundary as printer-serialised strings and are
    /// re-read by the receiver.
    #[cfg(feature = "concurrency")]
    Channel(std::sync::Arc<ChannelObj>),
}

/// The payload of a [`LispVal::Error`].
///
/// Conceptually a small map of `message` (a `String`) and `data` (a cons cell
/// or `Nil`).  Modelled as a struct with those two fields rather than a literal
/// `HashMap` so the common accessors are direct field reads.
#[derive(Debug, Clone)]
pub struct ErrorObj {
    /// Human-readable error message.
    pub message: String,
    /// Structured payload: a cons list of irritants, or `Nil`.
    pub data: LispVal,
}

/// The payload of a [`LispVal::Channel`] (only present under the `concurrency` feature).
///
/// A channel allows single-producer / single-consumer message passing between
/// two Lisp evaluation contexts.  Values are serialised via the printer on the
/// send side and deserialised via the reader on the receive side, avoiding the
/// requirement for [`LispVal`] to be `Send`.
///
/// The receiver is wrapped in a [`std::sync::Mutex`] so that the whole
/// `ChannelObj` can be placed behind an [`std::sync::Arc`] and cloned cheaply
/// (the Mutex guards exclusive access to `recv`).
#[cfg(feature = "concurrency")]
#[derive(Debug)]
pub struct ChannelObj {
    /// The sending half of the underlying `mpsc` channel.
    /// `Sender<String>` is `Clone + Send`, so this can be duplicated freely.
    pub sender: std::sync::mpsc::Sender<String>,
    /// The receiving half.  `mpsc::Receiver` is neither `Clone` nor `Sync`,
    /// so we wrap it in a `Mutex` to allow the `ChannelObj` to live behind `Arc`.
    pub receiver: std::sync::Mutex<std::sync::mpsc::Receiver<String>>,
}

/// The payload of a [`LispVal::Struct`].
#[derive(Debug, Clone)]
pub struct StructObj {
    /// Uppercased typed struct name, e.g. `POINT`.
    pub type_name: String,
    /// Field values in declaration order.
    pub fields: Vec<LispVal>,
}

impl LispVal {
    /// Wrap a host value implementing [`LispValExtension`] into a `LispVal`.
    pub fn ext<T: LispValExtension + 'static>(v: T) -> Self {
        LispVal::Extension(Rc::new(v))
    }
}

impl fmt::Debug for LispVal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LispVal::Symbol(s) => write!(f, "Symbol({:?})", s.borrow().name),
            LispVal::Number(n) => write!(f, "Number({n})"),
            LispVal::Char(b) => write!(f, "Char({b})"),
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
            LispVal::Array(a) => write!(f, "Array(len={})", a.borrow().len()),
            LispVal::Struct(s) => write!(f, "Struct(type={}, fields={:?})", s.type_name, s.fields),
            LispVal::Extension(e) => write!(f, "Extension({})", e.type_name()),
            LispVal::Error(e) => write!(f, "Error({:?}, {:?})", e.message, e.data),
            #[cfg(feature = "concurrency")]
            LispVal::Channel(_) => write!(f, "Channel(...)"),
        }
    }
}

impl Clone for LispVal {
    fn clone(&self) -> Self {
        match self {
            LispVal::Symbol(s) => LispVal::Symbol(Rc::clone(s)),
            LispVal::Number(n) => LispVal::Number(*n),
            LispVal::Char(b) => LispVal::Char(*b),
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
            LispVal::Array(a) => LispVal::Array(Rc::clone(a)),
            LispVal::Struct(s) => LispVal::Struct(Rc::clone(s)),
            LispVal::Extension(e) => LispVal::Extension(Rc::clone(e)),
            LispVal::Error(e) => LispVal::Error(Rc::clone(e)),
            #[cfg(feature = "concurrency")]
            LispVal::Channel(c) => LispVal::Channel(std::sync::Arc::clone(c)),
        }
    }
}

fn lisp_float_eq(a: f64, b: f64) -> bool {
    a == b || (a.is_nan() && b.is_nan())
}

fn lisp_float_hash_bits(f: f64) -> u64 {
    if f.is_nan() {
        f64::NAN.to_bits()
    } else if f == 0.0 {
        0.0f64.to_bits()
    } else {
        f.to_bits()
    }
}

impl PartialEq for LispVal {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (LispVal::Symbol(a), LispVal::Symbol(b)) => Rc::ptr_eq(a, b),
            (LispVal::Number(a), LispVal::Number(b)) => a == b,
            (LispVal::Char(a), LispVal::Char(b)) => a == b,
            (LispVal::Float(a), LispVal::Float(b)) => lisp_float_eq(*a, *b),
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
            (LispVal::Array(a), LispVal::Array(b)) => Rc::ptr_eq(a, b),
            (LispVal::Struct(a), LispVal::Struct(b)) => {
                a.type_name == b.type_name && a.fields == b.fields
            }
            (LispVal::Extension(a), LispVal::Extension(b)) => a.eq_ext(b.as_ref()),
            (LispVal::Error(a), LispVal::Error(b)) => a.message == b.message && a.data == b.data,
            #[cfg(feature = "concurrency")]
            (LispVal::Channel(a), LispVal::Channel(b)) => std::sync::Arc::ptr_eq(a, b),
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
            LispVal::Char(b) => b.hash(state),
            LispVal::Float(f) => lisp_float_hash_bits(*f).hash(state),
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
            LispVal::Array(a) => {
                Rc::as_ptr(a).hash(state);
            }
            LispVal::Struct(s) => {
                s.type_name.hash(state);
                s.fields.hash(state);
            }
            LispVal::Extension(e) => {
                e.hash_ext(state);
            }
            LispVal::Error(e) => {
                e.message.hash(state);
                e.data.hash(state);
            }
            LispVal::Builtin(_)
            | LispVal::Lambda(_)
            | LispVal::Fexpr(_)
            | LispVal::Macro(_)
            | LispVal::Vau(_) => {
                // Functions are not hashable by value.
            }
            #[cfg(feature = "concurrency")]
            LispVal::Channel(c) => std::sync::Arc::as_ptr(c).hash(state),
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
                value: None,
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
                car: Rc::new(car),
                cdr: Rc::new(cdr),
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
                    result.push(car.as_ref().clone());
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
    ("07-shell.lisp", include_str!("../lib/07-shell.lisp")),
    ("08-vau.lisp", include_str!("../lib/08-vau.lisp")),
    ("09-lisp15.lisp", include_str!("../lib/09-lisp15.lisp")),
    ("10-testing.lisp", include_str!("../lib/10-testing.lisp")),
    (
        "11-optimizer-vau.lisp",
        include_str!("../lib/11-optimizer-vau.lisp"),
    ),
    ("12-control.lisp", include_str!("../lib/12-control.lisp")),
    (
        "13-functional.lisp",
        include_str!("../lib/13-functional.lisp"),
    ),
    ("14-strings.lisp", include_str!("../lib/14-strings.lisp")),
    (
        "15-sets-hash.lisp",
        include_str!("../lib/15-sets-hash.lisp"),
    ),
    (
        "16-conditions.lisp",
        include_str!("../lib/16-conditions.lisp"),
    ),
    ("17-arrays.lisp", include_str!("../lib/17-arrays.lisp")),
    ("18-format.lisp", include_str!("../lib/18-format.lisp")),
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

fn proper_list_to_vec(list: &LispVal, context: &str) -> Result<Vec<LispVal>, LispError> {
    let mut out = Vec::new();
    let mut current = list;
    while let LispVal::Cons { car, cdr } = current {
        out.push(car.as_ref().clone());
        current = cdr;
    }
    if *current != LispVal::Nil {
        return Err(LispError::Generic(format!(
            "{context}: arguments must be a proper list"
        )));
    }
    Ok(out)
}

fn include_target(form: &LispVal) -> Result<Option<String>, LispError> {
    let LispVal::Cons { car, cdr } = form else {
        return Ok(None);
    };
    let LispVal::Symbol(sym) = car.as_ref() else {
        return Ok(None);
    };
    if sym.borrow().name != "INCLUDE" {
        return Ok(None);
    }

    let args = proper_list_to_vec(cdr, "include")?;
    if args.len() != 1 {
        return Err(LispError::Generic(
            "include requires exactly one string path".to_string(),
        ));
    }
    match &args[0] {
        LispVal::String(path) => Ok(Some(path.clone())),
        _ => Err(LispError::Generic(
            "include requires a string path".to_string(),
        )),
    }
}

fn resolve_include_path(parent_file: &Path, include: &str) -> PathBuf {
    let include_path = Path::new(include);
    if include_path.is_absolute() {
        return include_path.to_path_buf();
    }
    parent_file
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join(include_path)
}

fn load_file_inner(
    path: &Path,
    env: &Rc<Environment>,
    include_stack: &mut Vec<PathBuf>,
) -> Result<(), LispError> {
    let cycle_key = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if include_stack.iter().any(|p| p == &cycle_key) {
        let mut chain = include_stack
            .iter()
            .map(|p| p.display().to_string())
            .collect::<Vec<_>>();
        chain.push(cycle_key.display().to_string());
        return Err(LispError::Generic(format!(
            "include cycle detected: {}",
            chain.join(" -> ")
        )));
    }

    let display_path = path.display().to_string();
    let content = fs::read_to_string(path)
        .map_err(|e| LispError::Generic(format!("Failed to read file {display_path}: {e}")))?;
    let expressions = reader::read_all(&content, env)
        .map_err(|e| LispError::Generic(format!("Failed to parse file {display_path}: {e}")))?;

    include_stack.push(cycle_key);
    for expr in expressions {
        if let Some(include) = include_target(&expr)? {
            let include_path = resolve_include_path(path, &include);
            load_file_inner(&include_path, env, include_stack)?;
        } else {
            evaluator::eval(&expr, env)?;
        }
    }
    include_stack.pop();
    Ok(())
}

/// Load and evaluate a single Lisp source file.
///
/// Reads the file at `path`, parses all top-level expressions, and evaluates
/// them in `env` in order.  The `READ-FS` capability must be enabled in `env`
/// for the Lisp builtin `(LOAD-FILE path)` to call this; Rust host code can
/// call it directly regardless of feature flags.
///
/// # Errors
///
/// Returns the first parse or evaluation error encountered.  Subsequent
/// expressions in the file are **not** evaluated after a failure.
pub fn load_file(path: &str, env: &Rc<Environment>) -> Result<(), LispError> {
    load_file_inner(Path::new(path), env, &mut Vec::new())
}

/// Load and evaluate all `*.lisp` files in a directory, sorted by name.
///
/// Files are sorted by filename, so numeric prefixes (`00-core.lisp`,
/// `01-list.lisp`, …) control load order.  Subdirectories are ignored.
///
/// # Errors
///
/// Returns the first error from any file; subsequent files are not loaded.
pub fn load_directory(path: &str, env: &Rc<Environment>) -> Result<(), LispError> {
    let entries = std::fs::read_dir(path)
        .map_err(|e| LispError::Generic(format!("Failed to read directory {path}: {e}")))?;

    let mut files: Vec<_> = entries
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            let path = entry.path();
            path.is_file() && path.extension().is_some_and(|ext| ext == "lisp")
        })
        .collect();

    files.sort_by_key(|entry| entry.file_name());

    for entry in files {
        load_file(&entry.path().to_string_lossy(), env)?;
    }
    Ok(())
}

/// Run a minimal read-eval-print loop over `in_stream`/`out_stream`.
///
/// Reads one line at a time, evaluates it, and writes the result.  Used by
/// tests and simple embeddings that don't need rustyline's history and
/// completion.  For the interactive REPL, see the `lamedh-cli` crate.
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
